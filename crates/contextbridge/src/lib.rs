use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};
use thiserror::Error;
use uuid::Uuid;

pub mod aws;
pub mod promotion;
pub mod test_support;

pub const DEFAULT_MAX_TOKENS: usize = 1_000;
pub const MAX_MAX_TOKENS: usize = 8_000;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
        #[serde(transparent)]
        pub struct $name(pub String);
    };
}
id_type!(WorkspaceId);
id_type!(AgentId);
id_type!(EventId);
id_type!(MemoryId);
id_type!(EvidenceId);

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    Decision,
    Finding,
    Constraint,
    Task,
    State,
}
impl MemoryKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Finding => "finding",
            Self::Constraint => "constraint",
            Self::Task => "task",
            Self::State => "state",
        }
    }
}
impl TryFrom<&str> for MemoryKind {
    type Error = DomainError;
    fn try_from(v: &str) -> Result<Self, Self::Error> {
        match v {
            "decision" => Ok(Self::Decision),
            "finding" => Ok(Self::Finding),
            "constraint" => Ok(Self::Constraint),
            "task" => Ok(Self::Task),
            "state" => Ok(Self::State),
            _ => Err(DomainError::InvalidInput(format!(
                "unsupported memory kind: {v}"
            ))),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryStatus {
    Current,
    Superseded,
    Stale,
    Conflicted,
}
impl MemoryStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Superseded => "superseded",
            Self::Stale => "stale",
            Self::Conflicted => "conflicted",
        }
    }
}
impl TryFrom<&str> for MemoryStatus {
    type Error = DomainError;
    fn try_from(v: &str) -> Result<Self, Self::Error> {
        match v {
            "current" => Ok(Self::Current),
            "superseded" => Ok(Self::Superseded),
            "stale" => Ok(Self::Stale),
            "conflicted" => Ok(Self::Conflicted),
            _ => Err(DomainError::InvalidInput(format!(
                "unsupported memory status: {v}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Provenance {
    pub source_agent: AgentId,
    pub event_id: Option<EventId>,
    pub observed_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RawEvidenceHandle {
    pub handle: String,
    pub evidence_id: EvidenceId,
}
#[derive(Clone, Debug, Serialize)]
pub struct Memory {
    pub id: MemoryId,
    pub workspace_id: WorkspaceId,
    pub kind: MemoryKind,
    pub subject: Option<String>,
    pub content: String,
    pub status: MemoryStatus,
    pub provenance: Provenance,
    pub evidence: RawEvidenceHandle,
    pub superseded_by: Option<MemoryId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    pub event_id: Option<EventId>,
    pub kind: MemoryKind,
    pub subject: Option<String>,
    pub content: String,
    pub observed_at: Option<DateTime<Utc>>,
    pub raw_evidence: Option<String>,
    #[serde(default)]
    pub promote: bool,
    pub supersedes_memory_id: Option<MemoryId>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventsRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: Option<String>,
    pub events: Vec<EventInput>,
}
#[derive(Clone, Debug, Serialize)]
pub struct EventRecord {
    pub event_id: EventId,
    pub workspace_id: WorkspaceId,
    pub agent_id: AgentId,
    pub kind: MemoryKind,
    pub content: String,
    pub promoted_memory_id: Option<MemoryId>,
    pub received_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize)]
pub struct EventOutcome {
    pub outcome: String,
    pub event_id: EventId,
    pub promoted_memory_id: Option<MemoryId>,
    pub raw_handle: Option<RawEvidenceHandle>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMemoryRequest {
    pub workspace_id: WorkspaceId,
    pub kind: MemoryKind,
    pub subject: Option<String>,
    pub content: String,
    pub raw_evidence: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct MemoryListQuery {
    pub workspace_id: WorkspaceId,
    pub status: Option<MemoryStatus>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRequest {
    pub workspace_id: WorkspaceId,
    pub query: String,
    pub max_tokens: Option<usize>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupersedeRequest {
    pub workspace_id: WorkspaceId,
    pub replacement_memory_id: MemoryId,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ContextEvidence {
    pub memory_id: MemoryId,
    pub evidence_id: EvidenceId,
    pub subject: Option<String>,
    pub kind: MemoryKind,
    pub content: String,
    pub provenance: Provenance,
    pub reasons: Vec<String>,
    pub estimated_tokens: usize,
    pub raw_handle: String,
    pub status: MemoryStatus,
}
#[derive(Clone, Debug, Serialize)]
pub struct CurrentStateItem {
    pub key: String,
    pub value: String,
    pub memory_id: MemoryId,
}
#[derive(Clone, Debug, Serialize)]
pub struct TokenUsage {
    pub budget: usize,
    pub selected: usize,
    pub tokenizer: &'static str,
    pub is_proxy: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct ContextPacket {
    pub request_id: String,
    pub workspace_id: WorkspaceId,
    pub query: String,
    pub summary: String,
    pub current_state: Vec<CurrentStateItem>,
    pub evidence: Vec<ContextEvidence>,
    pub token_usage: TokenUsage,
}
#[derive(Clone, Debug, Serialize)]
pub struct ActivityRecord {
    pub event: EventRecord,
    pub outcome: Option<EventOutcome>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SupersedeResponse {
    pub superseded: Memory,
    pub current: Memory,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("workspace boundary violation")]
    WorkspaceBoundary,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("storage failure: {0}")]
    Storage(String),
    #[error("tokenizer unavailable: {0}")]
    Tokenizer(String),
}

#[async_trait]
pub trait EvidenceStore: Send + Sync {
    async fn put_raw(
        &self,
        workspace: &WorkspaceId,
        evidence_id: &EvidenceId,
        content: &[u8],
    ) -> Result<String, DomainError>;
    async fn recover_raw(&self, handle: &str) -> Result<Vec<u8>, DomainError>;
}

#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait Storage: EvidenceStore + Send + Sync {
    async fn append_event(
        &self,
        w: &WorkspaceId,
        a: &AgentId,
        e: &EventInput,
    ) -> Result<EventRecord, DomainError>;
    async fn mark_event_promoted(
        &self,
        event_id: &EventId,
        memory_id: &MemoryId,
    ) -> Result<(), DomainError>;
    async fn store_memory(
        &self,
        w: &WorkspaceId,
        a: &AgentId,
        event: Option<&EventId>,
        kind: &MemoryKind,
        subject: Option<&str>,
        content: &str,
        raw: Option<&str>,
    ) -> Result<Memory, DomainError>;
    async fn get_memory(&self, w: &WorkspaceId, id: &MemoryId) -> Result<Memory, DomainError>;
    async fn list_memories(
        &self,
        w: &WorkspaceId,
        status: Option<&MemoryStatus>,
        limit: u32,
    ) -> Result<Vec<Memory>, DomainError>;
    async fn supersede(
        &self,
        w: &WorkspaceId,
        old: &MemoryId,
        new: &MemoryId,
        reason: &str,
    ) -> Result<SupersedeResponse, DomainError>;
    async fn list_activity(
        &self,
        w: &WorkspaceId,
        limit: u32,
    ) -> Result<Vec<ActivityRecord>, DomainError>;
}

#[derive(Clone)]
pub struct LocalSqliteStorage {
    pool: SqlitePool,
}
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS events(event_id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL,agent_id TEXT NOT NULL,kind TEXT NOT NULL,subject TEXT,content TEXT NOT NULL,observed_at TEXT NOT NULL,received_at TEXT NOT NULL,promoted_memory_id TEXT);
CREATE TABLE IF NOT EXISTS memories(memory_id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL,kind TEXT NOT NULL,subject TEXT,content TEXT NOT NULL,status TEXT NOT NULL,source_agent TEXT NOT NULL,source_event_id TEXT,observed_at TEXT NOT NULL,evidence_id TEXT NOT NULL UNIQUE,raw_handle TEXT NOT NULL UNIQUE,superseded_by TEXT);
CREATE TABLE IF NOT EXISTS raw_evidence(evidence_id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL,raw_handle TEXT NOT NULL UNIQUE,content BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS supersessions(old_memory_id TEXT PRIMARY KEY,new_memory_id TEXT NOT NULL,workspace_id TEXT NOT NULL,reason TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS schema_meta(version INTEGER PRIMARY KEY);
CREATE INDEX IF NOT EXISTS memories_scope ON memories(workspace_id,status);
"#;
impl LocalSqliteStorage {
    pub async fn connect(url: &str) -> Result<Self, DomainError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(url)
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?;
        for s in SCHEMA.split(';').filter(|s| !s.trim().is_empty()) {
            sqlx::query(s)
                .execute(&pool)
                .await
                .map_err(|e| DomainError::Storage(e.to_string()))?;
        }
        let version: Option<i64> = sqlx::query_scalar("SELECT max(version) FROM schema_meta")
            .fetch_one(&pool)
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?;
        if version.unwrap_or(0) < 2 {
            let mut tx = pool
                .begin()
                .await
                .map_err(|e| DomainError::Storage(e.to_string()))?;
            sqlx::query("CREATE TABLE events_new(event_id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL,agent_id TEXT NOT NULL,kind TEXT NOT NULL,subject TEXT,content TEXT NOT NULL,observed_at TEXT NOT NULL,received_at TEXT NOT NULL,promoted_memory_id TEXT)").execute(&mut *tx).await.map_err(|e| DomainError::Storage(e.to_string()))?;
            sqlx::query("INSERT INTO events_new SELECT event_id,workspace_id,agent_id,kind,subject,content,observed_at,received_at,promoted_memory_id FROM events").execute(&mut *tx).await.map_err(|e| DomainError::Storage(e.to_string()))?;
            sqlx::query("DROP TABLE events")
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Storage(e.to_string()))?;
            sqlx::query("ALTER TABLE events_new RENAME TO events")
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Storage(e.to_string()))?;
            sqlx::query("INSERT INTO schema_meta(version) VALUES (2)")
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Storage(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| DomainError::Storage(e.to_string()))?;
        }
        Ok(Self { pool })
    }
}

const MEMORY_BY_ID: &str = "SELECT memory_id,workspace_id,kind,subject,content,status,source_agent,source_event_id,observed_at,evidence_id,raw_handle,superseded_by FROM memories WHERE workspace_id = ? AND memory_id = ?";
fn time(v: String) -> Result<DateTime<Utc>, DomainError> {
    DateTime::parse_from_rfc3339(&v)
        .map(|x| x.with_timezone(&Utc))
        .map_err(|e| DomainError::Storage(e.to_string()))
}
fn memory(row: sqlx::sqlite::SqliteRow) -> Result<Memory, DomainError> {
    Ok(Memory {
        id: MemoryId(row.get("memory_id")),
        workspace_id: WorkspaceId(row.get("workspace_id")),
        kind: MemoryKind::try_from(row.get::<String, _>("kind").as_str())?,
        subject: row.get("subject"),
        content: row.get("content"),
        status: MemoryStatus::try_from(row.get::<String, _>("status").as_str())?,
        provenance: Provenance {
            source_agent: AgentId(row.get("source_agent")),
            event_id: row.get::<Option<String>, _>("source_event_id").map(EventId),
            observed_at: time(row.get("observed_at"))?,
        },
        evidence: RawEvidenceHandle {
            handle: row.get("raw_handle"),
            evidence_id: EvidenceId(row.get("evidence_id")),
        },
        superseded_by: row.get::<Option<String>, _>("superseded_by").map(MemoryId),
    })
}

#[async_trait]
impl EvidenceStore for LocalSqliteStorage {
    async fn put_raw(
        &self,
        workspace: &WorkspaceId,
        evidence_id: &EvidenceId,
        content: &[u8],
    ) -> Result<String, DomainError> {
        let digest = blake3::hash(content).to_hex().to_string();
        let handle = format!("artifact://{}/{}", workspace.0, digest);
        sqlx::query("INSERT INTO raw_evidence(evidence_id,workspace_id,raw_handle,content) VALUES(?,?,?,?) ON CONFLICT(raw_handle) DO NOTHING")
            .bind(&evidence_id.0).bind(&workspace.0).bind(&handle).bind(content)
            .execute(&self.pool).await.map_err(|e| DomainError::Storage(e.to_string()))?;
        Ok(handle)
    }
    async fn recover_raw(&self, handle: &str) -> Result<Vec<u8>, DomainError> {
        sqlx::query("SELECT content FROM raw_evidence WHERE raw_handle=?")
            .bind(handle)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?
            .map(|r| r.get("content"))
            .ok_or_else(|| DomainError::NotFound(format!("raw evidence {handle}")))
    }
}

#[async_trait]
impl Storage for LocalSqliteStorage {
    async fn append_event(
        &self,
        w: &WorkspaceId,
        a: &AgentId,
        e: &EventInput,
    ) -> Result<EventRecord, DomainError> {
        if e.content.trim().is_empty() {
            return Err(DomainError::InvalidInput(
                "event content cannot be empty".into(),
            ));
        }
        let id = e
            .event_id
            .clone()
            .unwrap_or_else(|| EventId(Uuid::new_v4().to_string()));
        let observed = e.observed_at.unwrap_or_else(Utc::now);
        let received = Utc::now();
        let r=sqlx::query("INSERT INTO events(event_id,workspace_id,agent_id,kind,subject,content,observed_at,received_at) VALUES(?,?,?,?,?,?,?,?)").bind(&id.0).bind(&w.0).bind(&a.0).bind(e.kind.as_str()).bind(&e.subject).bind(&e.content).bind(observed.to_rfc3339()).bind(received.to_rfc3339()).execute(&self.pool).await;
        if let Err(x) = r {
            if x.to_string().contains("UNIQUE") {
                return Err(DomainError::Conflict(format!(
                    "event already exists: {}",
                    id.0
                )));
            }
            return Err(DomainError::Storage(x.to_string()));
        }
        Ok(EventRecord {
            event_id: id,
            workspace_id: w.clone(),
            agent_id: a.clone(),
            kind: e.kind.clone(),
            content: e.content.clone(),
            promoted_memory_id: None,
            received_at: received,
        })
    }
    async fn mark_event_promoted(
        &self,
        event_id: &EventId,
        memory_id: &MemoryId,
    ) -> Result<(), DomainError> {
        sqlx::query("UPDATE events SET promoted_memory_id = ? WHERE event_id = ?")
            .bind(&memory_id.0)
            .bind(&event_id.0)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?;
        Ok(())
    }
    async fn store_memory(
        &self,
        w: &WorkspaceId,
        a: &AgentId,
        event: Option<&EventId>,
        kind: &MemoryKind,
        subject: Option<&str>,
        content: &str,
        raw: Option<&str>,
    ) -> Result<Memory, DomainError> {
        if content.trim().is_empty() || content.len() > 100_000 {
            return Err(DomainError::InvalidInput(
                "content must be non-empty and <= 100000 bytes".into(),
            ));
        }
        let id = MemoryId(Uuid::new_v4().to_string());
        let evidence_id = EvidenceId(Uuid::new_v4().to_string());
        let handle = self
            .put_raw(w, &evidence_id, raw.unwrap_or(content).as_bytes())
            .await?;
        let observed = Utc::now();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?;
        sqlx::query("INSERT INTO memories(memory_id,workspace_id,kind,subject,content,status,source_agent,source_event_id,observed_at,evidence_id,raw_handle) VALUES(?,?,?,?,?,'current',?,?,?,?,?)").bind(&id.0).bind(&w.0).bind(kind.as_str()).bind(subject).bind(content).bind(&a.0).bind(event.map(|x|x.0.clone())).bind(observed.to_rfc3339()).bind(&evidence_id.0).bind(&handle).execute(&mut *tx).await.map_err(|e|DomainError::Storage(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?;
        Ok(Memory {
            id,
            workspace_id: w.clone(),
            kind: kind.clone(),
            subject: subject.map(str::to_string),
            content: content.into(),
            status: MemoryStatus::Current,
            provenance: Provenance {
                source_agent: a.clone(),
                event_id: event.cloned(),
                observed_at: observed,
            },
            evidence: RawEvidenceHandle {
                handle,
                evidence_id,
            },
            superseded_by: None,
        })
    }
    async fn get_memory(&self, w: &WorkspaceId, id: &MemoryId) -> Result<Memory, DomainError> {
        sqlx::query(MEMORY_BY_ID)
            .bind(&w.0)
            .bind(&id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?
            .ok_or_else(|| DomainError::NotFound(format!("memory {}", id.0)))
            .and_then(memory)
    }
    async fn list_memories(
        &self,
        w: &WorkspaceId,
        status: Option<&MemoryStatus>,
        limit: u32,
    ) -> Result<Vec<Memory>, DomainError> {
        let rows = if let Some(s) = status {
            sqlx::query("SELECT memory_id,workspace_id,kind,subject,content,status,source_agent,source_event_id,observed_at,evidence_id,raw_handle,superseded_by FROM memories WHERE workspace_id = ? AND status = ? ORDER BY observed_at DESC LIMIT ?")
                .bind(&w.0)
                .bind(s.as_str())
                .bind(limit.min(200))
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query("SELECT memory_id,workspace_id,kind,subject,content,status,source_agent,source_event_id,observed_at,evidence_id,raw_handle,superseded_by FROM memories WHERE workspace_id = ? ORDER BY observed_at DESC LIMIT ?")
                .bind(&w.0)
                .bind(limit.min(200))
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|e| DomainError::Storage(e.to_string()))?;
        rows.into_iter().map(memory).collect()
    }
    async fn supersede(
        &self,
        w: &WorkspaceId,
        old: &MemoryId,
        new: &MemoryId,
        reason: &str,
    ) -> Result<SupersedeResponse, DomainError> {
        if old == new {
            return Err(DomainError::InvalidInput(
                "a memory cannot supersede itself".into(),
            ));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?;
        let oldm = sqlx::query(MEMORY_BY_ID)
            .bind(&w.0)
            .bind(&old.0)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?
            .ok_or_else(|| DomainError::NotFound(format!("memory {}", old.0)))
            .and_then(memory)?;
        let newm = sqlx::query(MEMORY_BY_ID)
            .bind(&w.0)
            .bind(&new.0)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?
            .ok_or_else(|| DomainError::NotFound(format!("memory {}", new.0)))
            .and_then(memory)?;
        if oldm.workspace_id != newm.workspace_id {
            return Err(DomainError::WorkspaceBoundary);
        }
        if matches!(oldm.status, MemoryStatus::Superseded)
            && oldm.superseded_by.as_ref() == Some(new)
        {
            return Ok(SupersedeResponse {
                superseded: oldm,
                current: newm,
                reason: reason.into(),
            });
        }
        if !matches!(newm.status, MemoryStatus::Current) {
            return Err(DomainError::Conflict("replacement must be current".into()));
        }
        sqlx::query("UPDATE memories SET status='superseded',superseded_by=? WHERE memory_id=? AND workspace_id=?").bind(&new.0).bind(&old.0).bind(&w.0).execute(&mut *tx).await.map_err(|e|DomainError::Storage(e.to_string()))?;
        sqlx::query("INSERT INTO supersessions(old_memory_id,new_memory_id,workspace_id,reason,created_at) VALUES(?,?,?,?,?)").bind(&old.0).bind(&new.0).bind(&w.0).bind(reason).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await.map_err(|e|DomainError::Storage(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Storage(e.to_string()))?;
        Ok(SupersedeResponse {
            superseded: self.get_memory(w, old).await?,
            current: self.get_memory(w, new).await?,
            reason: reason.into(),
        })
    }
    async fn list_activity(
        &self,
        w: &WorkspaceId,
        limit: u32,
    ) -> Result<Vec<ActivityRecord>, DomainError> {
        let rows=sqlx::query("SELECT event_id,workspace_id,agent_id,kind,content,received_at,promoted_memory_id FROM events WHERE workspace_id=? ORDER BY received_at DESC LIMIT ?").bind(&w.0).bind(limit.min(200)).fetch_all(&self.pool).await.map_err(|e|DomainError::Storage(e.to_string()))?;
        rows.into_iter()
            .map(|r| {
                let id = EventId(r.get("event_id"));
                let promoted = r
                    .get::<Option<String>, _>("promoted_memory_id")
                    .map(MemoryId);
                Ok(ActivityRecord {
                    event: EventRecord {
                        event_id: id.clone(),
                        workspace_id: WorkspaceId(r.get("workspace_id")),
                        agent_id: AgentId(r.get("agent_id")),
                        kind: MemoryKind::try_from(r.get::<String, _>("kind").as_str())?,
                        content: r.get("content"),
                        promoted_memory_id: promoted.clone(),
                        received_at: time(r.get("received_at"))?,
                    },
                    outcome: promoted.map(|x| EventOutcome {
                        event_id: id,
                        outcome: "PROMOTED".into(),
                        promoted_memory_id: Some(x),
                        raw_handle: None,
                    }),
                })
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct AppState {
    pub storage: std::sync::Arc<dyn Storage>,
    pub tokens: std::sync::Arc<HashMap<String, AgentId>>,
}
#[derive(Clone)]
struct AuthenticatedAgent(AgentId);
pub fn app(state: AppState) -> Router {
    let protected = Router::new()
        .route("/v1/events", post(post_events))
        .route("/v1/memories", post(post_memory).get(get_memories))
        .route("/v1/activity", get(get_activity))
        .route("/v1/context", post(post_context))
        .route("/v1/memories/{id}/supersede", post(post_supersede))
        .route("/v1/evidence/{hash}", get(get_evidence))
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate));
    Router::new()
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .merge(protected)
        .with_state(state)
}
async fn authenticate(State(s): State<AppState>, mut req: Request, next: Next) -> Response {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let Some(agent) = token.and_then(|v| s.tokens.get(v)).cloned() else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":{"code":"unauthorized","message":"valid bearer token required"}})),
        )
            .into_response();
    };
    req.extensions_mut().insert(AuthenticatedAgent(agent));
    next.run(req).await
}
fn agent(req: &Request) -> Result<AgentId, ApiError> {
    req.extensions()
        .get::<AuthenticatedAgent>()
        .map(|x| x.0.clone())
        .ok_or_else(ApiError::unauthorized)
}
async fn body<T: DeserializeOwned>(req: Request) -> Result<T, ApiError> {
    let b = axum::body::to_bytes(req.into_body(), 1_000_000)
        .await
        .map_err(|_| ApiError::bad("request body too large"))?;
    serde_json::from_slice(&b).map_err(|e| ApiError::bad(&e.to_string()))
}
async fn post_events(
    State(s): State<AppState>,
    req: Request,
) -> Result<Json<Vec<EventOutcome>>, ApiError> {
    let a = agent(&req)?;
    let p: EventsRequest = body(req).await?;
    if p.events.is_empty() || p.events.len() > 20 {
        return Err(ApiError::bad("events must contain 1-20 items"));
    }
    let mut out = Vec::new();
    for e in p.events {
        let r = s.storage.append_event(&p.workspace_id, &a, &e).await?;
        let result =
            promotion::PromotionEngine::process(s.storage.as_ref(), &p.workspace_id, &a, &r, &e)
                .await?;
        out.push(EventOutcome {
            event_id: r.event_id,
            outcome: result.status().into(),
            promoted_memory_id: result.memory_id(),
            raw_handle: result.raw_handle(),
        });
    }
    Ok(Json(out))
}
async fn post_memory(State(s): State<AppState>, req: Request) -> Result<Json<Memory>, ApiError> {
    let a = agent(&req)?;
    let p: CreateMemoryRequest = body(req).await?;
    Ok(Json(
        s.storage
            .store_memory(
                &p.workspace_id,
                &a,
                None,
                &p.kind,
                p.subject.as_deref(),
                &p.content,
                p.raw_evidence.as_deref(),
            )
            .await?,
    ))
}
async fn get_memories(
    State(s): State<AppState>,
    Query(q): Query<MemoryListQuery>,
) -> Result<Json<Vec<Memory>>, ApiError> {
    Ok(Json(
        s.storage
            .list_memories(&q.workspace_id, q.status.as_ref(), q.limit.unwrap_or(100))
            .await?,
    ))
}
async fn get_activity(
    State(s): State<AppState>,
    Query(q): Query<MemoryListQuery>,
) -> Result<Json<Vec<ActivityRecord>>, ApiError> {
    Ok(Json(
        s.storage
            .list_activity(&q.workspace_id, q.limit.unwrap_or(100))
            .await?,
    ))
}
async fn post_supersede(
    State(s): State<AppState>,
    Path(id): Path<String>,
    req: Request,
) -> Result<Json<SupersedeResponse>, ApiError> {
    let p: SupersedeRequest = body(req).await?;
    Ok(Json(
        s.storage
            .supersede(
                &p.workspace_id,
                &MemoryId(id),
                &p.replacement_memory_id,
                &p.reason,
            )
            .await?,
    ))
}
fn kind_priority(kind: &MemoryKind) -> usize {
    match kind {
        MemoryKind::Decision => 4,
        MemoryKind::State => 3,
        MemoryKind::Finding | MemoryKind::Constraint => 2,
        MemoryKind::Task => 1,
    }
}

fn context_identity(memory: &Memory) -> String {
    format!(
        "{}|{}|{}",
        memory.kind.as_str(),
        normalize_context(memory.subject.as_deref().unwrap_or_default()),
        normalize_context(&memory.content)
    )
}

fn normalize_context(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

async fn post_context(
    State(s): State<AppState>,
    req: Request,
) -> Result<Json<ContextPacket>, ApiError> {
    let p: ContextRequest = body(req).await?;
    let budget = p.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    if budget == 0 || budget > MAX_MAX_TOKENS {
        return Err(ApiError::bad("max_tokens must be between 1 and 8000"));
    }
    let terms: Vec<String> = p
        .query
        .to_lowercase()
        .split_whitespace()
        .map(|x| x.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|x| x.len() > 2)
        .collect();
    let mut ranked: Vec<(usize, usize, bool, DateTime<Utc>, Memory)> = s
        .storage
        .list_memories(&p.workspace_id, Some(&MemoryStatus::Current), 200)
        .await?
        .into_iter()
        .map(|m| {
            let subject = m.subject.as_deref().unwrap_or_default().to_lowercase();
            let content = m.content.to_lowercase();
            let score = terms
                .iter()
                .filter(|term| subject.contains(term.as_str()) || content.contains(term.as_str()))
                .count();
            let subject_match = terms.iter().any(|term| subject.contains(term.as_str()));
            (
                kind_priority(&m.kind),
                score,
                subject_match,
                m.provenance.observed_at,
                m,
            )
        })
        .filter(|entry| entry.1 > 0)
        .collect();
    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| right.3.cmp(&left.3))
            .then_with(|| left.4.id.0.cmp(&right.4.id.0))
    });
    let mut used = 0;
    let mut seen = HashSet::new();
    let mut evidence = Vec::new();
    for (priority, score, subject_match, _observed_at, m) in ranked {
        if !seen.insert(context_identity(&m)) {
            continue;
        }
        let n = tokens(&m.content)?;
        if used + n > budget {
            continue;
        }
        used += n;
        let mut reasons = vec![
            "workspace match".into(),
            format!("kind priority: {priority}"),
            format!("{score} query term matches"),
        ];
        if subject_match {
            reasons.push("subject match".into());
        }
        reasons.push("current truth".into());
        evidence.push(ContextEvidence {
            memory_id: m.id,
            evidence_id: m.evidence.evidence_id,
            subject: m.subject.clone(),
            kind: m.kind,
            content: m.content,
            provenance: m.provenance,
            reasons,
            estimated_tokens: n,
            raw_handle: m.evidence.handle,
            status: m.status,
        });
    }
    let summary = evidence
        .iter()
        .map(|x| x.content.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let state = evidence
        .iter()
        .filter(|x| matches!(x.kind, MemoryKind::Decision | MemoryKind::State))
        .map(|x| CurrentStateItem {
            key: x
                .subject
                .clone()
                .unwrap_or_else(|| format!("{:?}", x.kind).to_lowercase()),
            value: x.content.clone(),
            memory_id: x.memory_id.clone(),
        })
        .collect();
    Ok(Json(ContextPacket {
        request_id: Uuid::new_v4().to_string(),
        workspace_id: p.workspace_id,
        query: p.query,
        summary,
        current_state: state,
        evidence,
        token_usage: TokenUsage {
            budget,
            selected: used,
            tokenizer: "o200k_base",
            is_proxy: true,
        },
    }))
}
async fn get_evidence(
    State(s): State<AppState>,
    Path(hash): Path<String>,
    Query(q): Query<MemoryListQuery>,
) -> Result<Response, ApiError> {
    let h = format!("artifact://{}/{}", q.workspace_id.0, hash);
    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        s.storage.recover_raw(&h).await?,
    )
        .into_response())
}
fn tokens(text: &str) -> Result<usize, DomainError> {
    static BPE: OnceLock<Result<tiktoken_rs::CoreBPE, String>> = OnceLock::new();
    BPE.get_or_init(|| tiktoken_rs::o200k_base().map_err(|e| e.to_string()))
        .as_ref()
        .map(|b| b.encode_ordinary(text).len())
        .map_err(|e| DomainError::Tokenizer(e.clone()))
}
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}
impl ApiError {
    fn bad(s: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: s.into(),
        }
    }
    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "valid bearer token required".into(),
        }
    }
}
impl From<DomainError> for ApiError {
    fn from(e: DomainError) -> Self {
        let status = match e {
            DomainError::InvalidInput(_) | DomainError::Tokenizer(_) => StatusCode::BAD_REQUEST,
            DomainError::NotFound(_) => StatusCode::NOT_FOUND,
            DomainError::WorkspaceBoundary | DomainError::Conflict(_) => StatusCode::CONFLICT,
            DomainError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: e.to_string(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.status.as_str(),"message":self.message}})),
        )
            .into_response()
    }
}
pub async fn local_app(url: &str, auth: HashMap<String, AgentId>) -> Result<Router, DomainError> {
    Ok(app(AppState {
        storage: std::sync::Arc::new(LocalSqliteStorage::connect(url).await?),
        tokens: std::sync::Arc::new(auth),
    }))
}
pub async fn default_app() -> Result<Router, DomainError> {
    std::fs::create_dir_all("data").map_err(|e| DomainError::Storage(e.to_string()))?;
    let mut a = HashMap::new();
    a.insert("agent-a-token".into(), AgentId("agent-a".into()));
    a.insert("agent-b-token".into(), AgentId("agent-b".into()));
    local_app("sqlite://data/contextbridge.db?mode=rwc", a).await
}

pub async fn configured_app() -> Result<Router, DomainError> {
    let config = aws::AwsBackendConfig::from_env()?;
    let mut auth = HashMap::new();
    auth.insert("agent-a-token".into(), AgentId("agent-a".into()));
    auth.insert("agent-b-token".into(), AgentId("agent-b".into()));
    match config.mode {
        aws::StorageMode::Local => default_app().await,
        aws::StorageMode::Aws => {
            let sdk = match config.region {
                Some(region) => {
                    aws_config::defaults(aws_config::BehaviorVersion::latest())
                        .region(aws_sdk_dynamodb::config::Region::new(region))
                        .load()
                        .await
                }
                None => {
                    aws_config::defaults(aws_config::BehaviorVersion::latest())
                        .load()
                        .await
                }
            };
            let evidence = std::sync::Arc::new(aws::S3EvidenceStore::new(
                aws_sdk_s3::Client::new(&sdk),
                config.s3_bucket.expect("AWS config validated bucket"),
            ));
            let storage = aws::DynamoDbStorage::new(
                aws_sdk_dynamodb::Client::new(&sdk),
                config.dynamodb_table.expect("AWS config validated table"),
                evidence,
            );
            Ok(app(AppState {
                storage: std::sync::Arc::new(storage),
                tokens: std::sync::Arc::new(auth),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn test_app() -> Router {
        let mut auth = HashMap::new();
        auth.insert("a".into(), AgentId("agent-a".into()));
        auth.insert("b".into(), AgentId("agent-b".into()));
        local_app("sqlite::memory:", auth).await.unwrap()
    }

    async fn send(
        app: &Router,
        token: &str,
        method: &str,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(json!({"raw":"binary"})),
        )
    }

    #[tokio::test]
    async fn golden_path_promotes_supersedes_isolates_and_recovers() {
        let storage = LocalSqliteStorage::connect("sqlite::memory:")
            .await
            .unwrap();
        let mut auth = HashMap::new();
        auth.insert("a".into(), AgentId("agent-a".into()));
        auth.insert("b".into(), AgentId("agent-b".into()));
        let app = app(AppState {
            storage: std::sync::Arc::new(storage.clone()),
            tokens: std::sync::Arc::new(auth),
        });
        let (status, first) = send(&app, "a", "POST", "/v1/events", json!({"workspace_id":"project-alpha","events":[{"kind":"decision","subject":"persistence","content":"Persistence uses Redis.","promote":true}]})).await;
        assert_eq!(status, StatusCode::OK);
        let redis = first[0]["promoted_memory_id"].as_str().unwrap().to_owned();
        let (_, packet) = send(&app, "b", "POST", "/v1/context", json!({"workspace_id":"project-alpha","query":"Implement persistence","max_tokens":100})).await;
        assert!(packet["summary"].as_str().unwrap().contains("Redis"));
        assert_eq!(
            packet["evidence"][0]["provenance"]["source_agent"],
            "agent-a"
        );
        let raw_handle = packet["evidence"][0]["raw_handle"]
            .as_str()
            .unwrap()
            .to_owned();
        let (status, second) = send(&app, "a", "POST", "/v1/events", json!({"workspace_id":"project-alpha","events":[{"kind":"decision","subject":"persistence","content":"Persistence uses DynamoDB.","promote":true,"supersedes_memory_id":redis}]})).await;
        assert_eq!(status, StatusCode::OK);
        let (_, packet) = send(&app, "b", "POST", "/v1/context", json!({"workspace_id":"project-alpha","query":"Implement persistence","max_tokens":100})).await;
        assert!(packet["summary"].as_str().unwrap().contains("DynamoDB"));
        assert!(!packet["summary"].as_str().unwrap().contains("Redis"));
        assert!(packet["token_usage"]["selected"].as_u64().unwrap() <= 100);
        let (_, history) = send(
            &app,
            "b",
            "GET",
            "/v1/memories?workspace_id=project-alpha&status=superseded",
            json!({}),
        )
        .await;
        assert_eq!(history[0]["id"], redis);
        let (_, beta) = send(&app, "a", "POST", "/v1/events", json!({"workspace_id":"project-beta","events":[{"kind":"decision","subject":"persistence","content":"Persistence uses PostgreSQL.","promote":true}]})).await;
        assert!(beta[0]["promoted_memory_id"].is_string());
        let (_, isolated) = send(&app, "b", "POST", "/v1/context", json!({"workspace_id":"project-alpha","query":"Implement persistence","max_tokens":100})).await;
        assert!(!isolated["summary"].as_str().unwrap().contains("PostgreSQL"));
        assert_eq!(
            storage.recover_raw(&raw_handle).await.unwrap(),
            b"Persistence uses Redis."
        );
        assert!(second[0]["raw_handle"].is_object());
        assert!(raw_handle.starts_with("artifact://project-alpha/"));
    }

    #[tokio::test]
    async fn auth_derives_identity_and_rejects_spoofing() {
        let app = test_app().await;
        let (status, _) = send(
            &app,
            "a",
            "POST",
            "/v1/memories",
            json!({"workspace_id":"x","kind":"finding","content":"ok","agent_id":"trusted-agent"}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = send(
            &app,
            "bad",
            "POST",
            "/v1/memories",
            json!({"workspace_id":"x","kind":"finding","content":"ok"}),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn cross_workspace_supersession_and_missing_target_fail() {
        let app = test_app().await;
        let (_, a) = send(
            &app,
            "a",
            "POST",
            "/v1/memories",
            json!({"workspace_id":"a","kind":"decision","content":"A"}),
        )
        .await;
        let (_, b) = send(
            &app,
            "a",
            "POST",
            "/v1/memories",
            json!({"workspace_id":"b","kind":"decision","content":"B"}),
        )
        .await;
        let (status, _) = send(
            &app,
            "a",
            "POST",
            &format!("/v1/memories/{}/supersede", a["id"].as_str().unwrap()),
            json!({"workspace_id":"a","replacement_memory_id":b["id"],"reason":"wrong"}),
        )
        .await;
        assert!(status == StatusCode::NOT_FOUND || status == StatusCode::CONFLICT);
        let (status, _) = send(
            &app,
            "a",
            "POST",
            &format!("/v1/memories/{}/supersede", a["id"].as_str().unwrap()),
            json!({"workspace_id":"a","replacement_memory_id":"missing","reason":"missing"}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    #[test]
    fn context_fixture_matches_public_shape() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/api/context-response.json"))
                .unwrap();
        for key in [
            "request_id",
            "workspace_id",
            "query",
            "summary",
            "current_state",
            "evidence",
            "token_usage",
        ] {
            assert!(fixture.get(key).is_some(), "missing fixture field: {key}");
        }
        assert_eq!(fixture["token_usage"]["tokenizer"], "o200k_base");
        assert_eq!(fixture["token_usage"]["is_proxy"], true);
        assert!(
            fixture["evidence"][0]["raw_handle"]
                .as_str()
                .unwrap()
                .starts_with("artifact://")
        );
    }
}
