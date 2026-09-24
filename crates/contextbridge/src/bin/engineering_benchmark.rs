use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use chrono::Utc;
use contextbridge::local_app;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    env,
    error::Error,
    fs::{self, File},
    io::{BufWriter, Write},
    process::Command,
    sync::OnceLock,
    time::Instant,
};
use tower::ServiceExt;

type E = Box<dyn Error + Send + Sync>;
const A: &str = "agent-a-token";
const B: &str = "agent-b-token";
const ALPHA: &str = "project-alpha";
const BETA: &str = "project-beta";
const SEED: u64 = 17;
const BUDGET: usize = 100;

#[derive(Clone, Copy, Debug)]
enum Arm {
    Cold,
    Naive,
    ContextBridge,
}
impl Arm {
    fn all() -> [Self; 3] {
        [Self::Cold, Self::Naive, Self::ContextBridge]
    }
    fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Naive => "naive",
            Self::ContextBridge => "contextbridge",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Supersession,
    Relevance,
    Isolation,
    Duplication,
    Pressure,
}
impl Scenario {
    fn all() -> [Self; 5] {
        [
            Self::Supersession,
            Self::Relevance,
            Self::Isolation,
            Self::Duplication,
            Self::Pressure,
        ]
    }
    fn name(self) -> &'static str {
        match self {
            Self::Supersession => "supersession",
            Self::Relevance => "relevance",
            Self::Isolation => "workspace-isolation",
            Self::Duplication => "duplication",
            Self::Pressure => "token-pressure",
        }
    }
    fn core(self) -> usize {
        match self {
            Self::Supersession | Self::Isolation => 2,
            _ => 1,
        }
    }
    fn query(self) -> &'static str {
        match self {
            Self::Pressure => "Implement persistence under a tight context budget",
            _ => "Implement persistence",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct Item {
    memory_id: String,
    workspace_id: String,
    kind: String,
    subject: Option<String>,
    content: String,
    status: String,
    source_agent: Option<String>,
    raw_handle: Option<String>,
    identity: String,
}

#[derive(Debug, Serialize)]
struct Metrics {
    task_success: bool,
    required_evidence_recall: f64,
    context_precision: f64,
    stale_evidence_count: usize,
    stale_evidence_rate: f64,
    irrelevant_evidence_count: usize,
    cross_workspace_leakage: usize,
    duplicate_evidence_count: usize,
    supersession_correct: bool,
    raw_evidence_recoverable: bool,
    model_visible_tokens: usize,
    packet_size_bytes: usize,
    retrieval_latency_ms: Option<u128>,
    ingestion_latency_ms: u128,
    promotion_latency_ms: u128,
    latency_ms: u128,
}

#[derive(Debug, Serialize)]
struct Row {
    schema_version: u8,
    run_id: String,
    timestamp: String,
    git_commit: String,
    scenario: String,
    arm: String,
    dataset_size: usize,
    seed: u64,
    rustc: String,
    required_evidence: Vec<String>,
    candidate_evidence_count: usize,
    provided_evidence: Vec<Item>,
    metrics: Metrics,
}

struct Seed {
    ingestion: u128,
    promotion: u128,
}
struct Selection {
    items: Vec<Item>,
    retrieval: Option<u128>,
    bytes: usize,
    tokens: usize,
}

fn args() -> Result<(String, Vec<usize>), E> {
    let mut output = "benchmarks/results/engineering-baseline.jsonl".into();
    let mut sizes = vec![10, 100, 1_000];
    let argv = env::args().skip(1).collect::<Vec<_>>();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--output" => {
                output = argv.get(i + 1).ok_or("missing --output value")?.clone();
                i += 2;
            }
            "--sizes" => {
                sizes = argv
                    .get(i + 1)
                    .ok_or("missing --sizes value")?
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<Vec<usize>, _>>()?;
                if sizes.is_empty() || sizes.contains(&0) {
                    return Err("sizes must be positive".into());
                }
                i += 2;
            }
            "--help" | "-h" => {
                println!("usage: engineering-benchmark [--output FILE] [--sizes 10,100,1000]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    Ok((output, sizes))
}

#[tokio::main]
async fn main() -> Result<(), E> {
    let (output, sizes) = args()?;
    if let Some(parent) = std::path::Path::new(&output).parent() {
        fs::create_dir_all(parent)?;
    }
    let timestamp = Utc::now().to_rfc3339();
    let commit = git("git", &["rev-parse", "HEAD"]);
    let rustc = git("rustc", &["--version"]);
    let mut writer = BufWriter::new(File::create(&output)?);
    for scenario in Scenario::all() {
        for size in &sizes {
            for arm in Arm::all() {
                let row = run(scenario, arm, *size, &timestamp, &commit, &rustc).await?;
                serde_json::to_writer(&mut writer, &row)?;
                writer.write_all(b"\n")?;
            }
        }
    }
    writer.flush()?;
    println!("{output}");
    Ok(())
}

async fn run(
    scenario: Scenario,
    arm: Arm,
    size: usize,
    timestamp: &str,
    commit: &str,
    rustc: &str,
) -> Result<Row, E> {
    let mut auth = HashMap::new();
    auth.insert(A.into(), contextbridge::AgentId("agent-a".into()));
    auth.insert(B.into(), contextbridge::AgentId("agent-b".into()));
    let app = local_app("sqlite::memory:", auth).await?;
    let seeded = seed(&app, scenario, size).await?;
    let selected = select(&app, arm, scenario).await?;
    let candidates = if matches!(arm, Arm::Cold) {
        0
    } else {
        candidates(&app, scenario).await?
    };
    let recoverable = recover(&app, &selected.items).await?;
    let required = required(scenario);
    let metrics = measure(
        scenario,
        &selected.items,
        &required,
        recoverable,
        &selected,
        seeded,
    );
    Ok(Row {
        schema_version: 1,
        run_id: format!("{timestamp}-{}-{}-{size}", scenario.name(), arm.name()),
        timestamp: timestamp.into(),
        git_commit: commit.into(),
        scenario: scenario.name().into(),
        arm: arm.name().into(),
        dataset_size: size,
        seed: SEED,
        rustc: rustc.into(),
        required_evidence: required,
        candidate_evidence_count: candidates,
        provided_evidence: selected.items,
        metrics,
    })
}
async fn seed(app: &Router, scenario: Scenario, size: usize) -> Result<Seed, E> {
    let started = Instant::now();
    let promotion_started = Instant::now();
    let core = match scenario {
        Scenario::Supersession => vec![
            ("redis", "persistence", "Persistence uses Redis."),
            ("dynamodb", "persistence", "Persistence uses DynamoDB."),
        ],
        Scenario::Relevance | Scenario::Duplication | Scenario::Pressure => {
            vec![("dynamodb", "persistence", "Persistence uses DynamoDB.")]
        }
        Scenario::Isolation => vec![
            ("alpha", "persistence", "Persistence uses DynamoDB."),
            ("beta", "persistence", "Persistence uses PostgreSQL."),
        ],
    };
    for (id, subject, content) in &core {
        let workspace = if *id == "beta" { BETA } else { ALPHA };
        event(
            app,
            workspace,
            &format!("{}-{id}", scenario.name()),
            subject,
            content,
        )
        .await?;
    }
    let promotion = promotion_started.elapsed().as_millis();
    match scenario {
        Scenario::Duplication => {
            for i in 0..size.saturating_sub(1) {
                memory(
                    app,
                    ALPHA,
                    &format!("duplicate-{i}"),
                    "Persistence uses DynamoDB.",
                )
                .await?;
            }
        }
        _ => {
            let filler = size.saturating_sub(scenario.core());
            for chunk in (0..filler).collect::<Vec<_>>().chunks(20) {
                let values = chunk.iter().map(|i| json!({
                    "event_id": format!("{}-filler-{i}", scenario.name()),
                    "kind": "finding",
                    "subject": format!("unrelated-{i}"),
                    "content": format!("Unrelated project fact {i} from deterministic seed {seed}.", seed = SEED),
                    "raw_evidence": format!("Raw unrelated evidence {i}."),
                    "promote": true
                })).collect();
                events(app, ALPHA, values).await?;
            }
        }
    }
    Ok(Seed {
        ingestion: started.elapsed().as_millis(),
        promotion,
    })
}

async fn event(
    app: &Router,
    workspace: &str,
    id: &str,
    subject: &str,
    content: &str,
) -> Result<(), E> {
    events(
        app,
        workspace,
        vec![json!({
            "event_id": id, "kind": "decision", "subject": subject, "content": content,
            "raw_evidence": format!("Agent A raw evidence: {content}"), "promote": true
        })],
    )
    .await
}

async fn events(app: &Router, workspace: &str, values: Vec<Value>) -> Result<(), E> {
    json_request(
        app,
        A,
        "POST",
        "/v1/events",
        json!({"workspace_id": workspace, "events": values}),
    )
    .await
    .map(|_| ())
}

async fn memory(app: &Router, workspace: &str, id: &str, content: &str) -> Result<(), E> {
    json_request(
        app,
        A,
        "POST",
        "/v1/memories",
        json!({
            "workspace_id": workspace, "kind": "decision", "subject": "persistence",
            "content": content, "raw_evidence": format!("Duplicate raw evidence {id}.")
        }),
    )
    .await
    .map(|_| ())
}
async fn select(app: &Router, arm: Arm, scenario: Scenario) -> Result<Selection, E> {
    match arm {
        Arm::Cold => Ok(Selection {
            items: vec![],
            retrieval: None,
            bytes: 0,
            tokens: 0,
        }),
        Arm::Naive => {
            let started = Instant::now();
            let mut values = list(app, ALPHA).await?;
            if matches!(scenario, Scenario::Isolation) {
                values.extend(list(app, BETA).await?);
            }
            let items = values
                .iter()
                .map(|v| item(v, ALPHA))
                .collect::<Result<Vec<_>, _>>()?;
            let bytes = serde_json::to_vec(&items)?.len();
            let tokens = items.iter().map(|x| token_count(&x.content)).sum();
            Ok(Selection {
                items,
                retrieval: Some(started.elapsed().as_millis()),
                bytes,
                tokens,
            })
        }
        Arm::ContextBridge => {
            let started = Instant::now();
            let packet = json_request(
                app,
                B,
                "POST",
                "/v1/context",
                json!({
                    "workspace_id": ALPHA, "query": scenario.query(), "max_tokens": BUDGET
                }),
            )
            .await?;
            let bytes = serde_json::to_vec(&packet)?.len();
            let tokens = packet["token_usage"]["selected"]
                .as_u64()
                .ok_or("missing selected token count")? as usize;
            let items = packet["evidence"]
                .as_array()
                .ok_or("missing context evidence")?
                .iter()
                .map(context_item)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Selection {
                items,
                retrieval: Some(started.elapsed().as_millis()),
                bytes,
                tokens,
            })
        }
    }
}

async fn list(app: &Router, workspace: &str) -> Result<Vec<Value>, E> {
    json_request(
        app,
        B,
        "GET",
        &format!("/v1/memories?workspace_id={workspace}&limit=200"),
        json!({}),
    )
    .await?
    .as_array()
    .cloned()
    .ok_or_else(|| "memories response not array".into())
}

async fn candidates(app: &Router, scenario: Scenario) -> Result<usize, E> {
    let mut count = list(app, ALPHA).await?.len();
    if matches!(scenario, Scenario::Isolation) {
        count += list(app, BETA).await?.len();
    }
    Ok(count)
}

fn item(v: &Value, default_workspace: &str) -> Result<Item, E> {
    let content: String = v["content"]
        .as_str()
        .ok_or("missing memory content")?
        .into();
    Ok(Item {
        memory_id: v["id"].as_str().ok_or("missing memory id")?.into(),
        workspace_id: v["workspace_id"]
            .as_str()
            .unwrap_or(default_workspace)
            .into(),
        kind: v["kind"].as_str().ok_or("missing memory kind")?.into(),
        subject: v["subject"].as_str().map(Into::into),
        content: content.clone(),
        status: v["status"].as_str().ok_or("missing memory status")?.into(),
        source_agent: v["provenance"]["source_agent"].as_str().map(Into::into),
        raw_handle: v["evidence"]["handle"].as_str().map(Into::into),
        identity: identity(
            v["kind"].as_str().unwrap_or_default(),
            v["subject"].as_str(),
            &content,
        ),
    })
}

fn context_item(v: &Value) -> Result<Item, E> {
    let content: String = v["content"]
        .as_str()
        .ok_or("missing context content")?
        .into();
    Ok(Item {
        memory_id: v["memory_id"]
            .as_str()
            .ok_or("missing context memory id")?
            .into(),
        workspace_id: ALPHA.into(),
        kind: v["kind"].as_str().ok_or("missing context kind")?.into(),
        subject: v["subject"].as_str().map(Into::into),
        content: content.clone(),
        status: v["status"].as_str().ok_or("missing context status")?.into(),
        source_agent: v["provenance"]["source_agent"].as_str().map(Into::into),
        raw_handle: v["raw_handle"].as_str().map(Into::into),
        identity: identity(
            v["kind"].as_str().unwrap_or_default(),
            v["subject"].as_str(),
            &content,
        ),
    })
}

async fn recover(app: &Router, items: &[Item]) -> Result<bool, E> {
    if items.is_empty() {
        return Ok(false);
    }
    for item in items {
        let handle = item.raw_handle.as_deref().ok_or("missing raw handle")?;
        let hash = handle.rsplit('/').next().ok_or("invalid raw handle")?;
        let bytes = request_bytes(
            app,
            B,
            &format!("/v1/evidence/{hash}?workspace_id={}", item.workspace_id),
        )
        .await?;
        if bytes.is_empty() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn required(scenario: Scenario) -> Vec<String> {
    let mut values = vec!["DynamoDB current".into()];
    if matches!(scenario, Scenario::Supersession) {
        values.push("Agent A provenance".into());
    }
    values
}

fn measure(
    scenario: Scenario,
    selected: &[Item],
    required: &[String],
    recoverable: bool,
    selection: &Selection,
    seed: Seed,
) -> Metrics {
    let found = required
        .iter()
        .filter(|r| selected.iter().any(|i| satisfies(i, r)))
        .count();
    let recall = ratio(found, required.len());
    let relevant = selected
        .iter()
        .filter(|i| i.workspace_id == ALPHA && i.subject.as_deref() == Some("persistence"))
        .count();
    let precision = ratio(relevant, selected.len());
    let stale = selected
        .iter()
        .filter(|i| {
            i.status == "superseded"
                || (matches!(scenario, Scenario::Supersession) && i.content.contains("Redis"))
        })
        .count();
    let leakage = selected.iter().filter(|i| i.workspace_id != ALPHA).count();
    let mut seen = HashMap::new();
    for i in selected {
        *seen.entry(&i.identity).or_insert(0usize) += 1;
    }
    let duplicates = seen.values().map(|n| n.saturating_sub(1)).sum();
    let supersession = if matches!(scenario, Scenario::Supersession) {
        selected
            .iter()
            .any(|i| i.status == "current" && i.content.contains("DynamoDB"))
            && stale == 0
    } else {
        stale == 0
    };
    let success = recall == 1.0
        && stale == 0
        && leakage == 0
        && duplicates == 0
        && supersession
        && recoverable
        && selection.tokens <= BUDGET;
    Metrics {
        task_success: success,
        required_evidence_recall: recall,
        context_precision: precision,
        stale_evidence_count: stale,
        stale_evidence_rate: ratio(stale, selected.len()),
        irrelevant_evidence_count: selected.len().saturating_sub(relevant),
        cross_workspace_leakage: leakage,
        duplicate_evidence_count: duplicates,
        supersession_correct: supersession,
        raw_evidence_recoverable: recoverable,
        model_visible_tokens: selection.tokens,
        packet_size_bytes: selection.bytes,
        retrieval_latency_ms: selection.retrieval,
        ingestion_latency_ms: seed.ingestion,
        promotion_latency_ms: seed.promotion,
        latency_ms: seed.ingestion + selection.retrieval.unwrap_or(0),
    }
}

fn satisfies(item: &Item, requirement: &str) -> bool {
    match requirement {
        "DynamoDB current" => item.status == "current" && item.content.contains("DynamoDB"),
        "Agent A provenance" => {
            item.source_agent.as_deref() == Some("agent-a") && item.content.contains("DynamoDB")
        }
        _ => false,
    }
}

fn identity(kind: &str, subject: Option<&str>, content: &str) -> String {
    format!(
        "{}|{}|{}",
        kind.to_lowercase(),
        normalize(subject.unwrap_or_default()),
        normalize(content)
    )
}
fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn ratio(n: usize, d: usize) -> f64 {
    if d == 0 { 0.0 } else { n as f64 / d as f64 }
}

async fn json_request(
    app: &Router,
    token: &str,
    method: &str,
    uri: &str,
    payload: Value,
) -> Result<Value, E> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))?,
        )
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 5_000_000).await?;
    let value = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&bytes)?
    };
    if status != StatusCode::OK {
        return Err(format!("{method} {uri} failed: {status} {value}").into());
    }
    Ok(value)
}

async fn request_bytes(app: &Router, token: &str, uri: &str) -> Result<Vec<u8>, E> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 5_000_000).await?.to_vec();
    if status != StatusCode::OK {
        return Err(format!("GET {uri} failed: {status}").into());
    }
    Ok(bytes)
}

fn token_count(value: &str) -> usize {
    static BPE: OnceLock<Result<tiktoken_rs::CoreBPE, String>> = OnceLock::new();
    BPE.get_or_init(|| tiktoken_rs::o200k_base().map_err(|e| e.to_string()))
        .as_ref()
        .map(|bpe| bpe.encode_ordinary(value).len())
        .unwrap_or_else(|e| panic!("tokenizer unavailable: {e}"))
}
fn git(command: &str, args: &[&str]) -> String {
    let output = Command::new(command).args(args).output().ok();
    output
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().into())
        .filter(|s: &String| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextbridge::AgentId;

    #[test]
    fn evaluator_detects_leakage_and_duplicates() {
        let item = |workspace: &str, id: &str| Item {
            memory_id: id.into(),
            workspace_id: workspace.into(),
            kind: "decision".into(),
            subject: Some("persistence".into()),
            content: "Persistence uses DynamoDB.".into(),
            status: "current".into(),
            source_agent: Some("agent-a".into()),
            raw_handle: Some(format!("artifact://{workspace}/{id}")),
            identity: "same".into(),
        };
        let selected = vec![item(ALPHA, "one"), item(BETA, "two")];
        let m = measure(
            Scenario::Isolation,
            &selected,
            &required(Scenario::Isolation),
            true,
            &Selection {
                items: selected.clone(),
                retrieval: Some(1),
                bytes: 1,
                tokens: 1,
            },
            Seed {
                ingestion: 1,
                promotion: 1,
            },
        );
        assert_eq!(m.required_evidence_recall, 1.0);
        assert_eq!(m.cross_workspace_leakage, 1);
        assert_eq!(m.duplicate_evidence_count, 1);
        assert!(!m.task_success);
    }

    #[tokio::test]
    async fn arms_have_distinct_context_boundaries() {
        let mut auth = HashMap::new();
        auth.insert(A.into(), AgentId("agent-a".into()));
        auth.insert(B.into(), AgentId("agent-b".into()));
        let app = local_app("sqlite::memory:", auth).await.unwrap();
        seed(&app, Scenario::Supersession, 10).await.unwrap();
        assert!(
            select(&app, Arm::Cold, Scenario::Supersession)
                .await
                .unwrap()
                .items
                .is_empty()
        );
        assert!(
            select(&app, Arm::Naive, Scenario::Supersession)
                .await
                .unwrap()
                .items
                .iter()
                .any(|i| i.content.contains("Redis"))
        );
        assert!(
            select(&app, Arm::ContextBridge, Scenario::Supersession)
                .await
                .unwrap()
                .items
                .iter()
                .all(|i| !i.content.contains("Redis"))
        );
    }

    #[tokio::test]
    async fn same_seed_produces_equivalent_scenario_data() {
        let mut auth = HashMap::new();
        auth.insert(A.into(), AgentId("agent-a".into()));
        auth.insert(B.into(), AgentId("agent-b".into()));
        let first = local_app("sqlite::memory:", auth.clone()).await.unwrap();
        let second = local_app("sqlite::memory:", auth).await.unwrap();
        seed(&first, Scenario::Relevance, 10).await.unwrap();
        seed(&second, Scenario::Relevance, 10).await.unwrap();
        let snapshot = |values: Vec<Value>| {
            values
                .into_iter()
                .map(|value| {
                    (
                        value["kind"].clone(),
                        value["subject"].clone(),
                        value["content"].clone(),
                        value["status"].clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            snapshot(list(&first, ALPHA).await.unwrap()),
            snapshot(list(&second, ALPHA).await.unwrap())
        );
    }
}
