//! AWS persistence adapters for the existing ContextBridge ports.
//!
//! This module owns AWS SDK mappings only. Domain/API types remain in the
//! parent module and never depend on AWS SDK types.

use crate::{
    ActivityRecord, AgentId, DomainError, EventId, EventInput, EventOutcome, EvidenceStore, Memory,
    MemoryId, MemoryKind, MemoryStatus, Provenance, RawEvidenceHandle, Storage, SupersedeResponse,
    WorkspaceId,
};
use async_trait::async_trait;
use aws_sdk_dynamodb::{
    Client as DynamoDbClient,
    types::{AttributeValue, ConditionCheck, Put, TransactWriteItem, Update},
};
use aws_sdk_s3::{Client as S3Client, error::ProvideErrorMetadata, primitives::ByteStream};
use chrono::{DateTime, Utc};

use std::{collections::HashMap, env, sync::Arc};
use uuid::Uuid;

const PK: &str = "pk";
const SK: &str = "sk";

/// Runtime backend selection. Credentials are intentionally not represented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageMode {
    Local,
    Aws,
}

#[derive(Clone, Debug)]
pub struct AwsBackendConfig {
    pub mode: StorageMode,
    pub dynamodb_table: Option<String>,
    pub s3_bucket: Option<String>,
    pub region: Option<String>,
}

impl AwsBackendConfig {
    pub fn from_env() -> Result<Self, DomainError> {
        let mode = match env::var("CONTEXTBRIDGE_STORAGE")
            .unwrap_or_else(|_| "local".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "local" => StorageMode::Local,
            "aws" => StorageMode::Aws,
            other => {
                return Err(DomainError::InvalidInput(format!(
                    "CONTEXTBRIDGE_STORAGE must be local or aws, got {other}"
                )));
            }
        };
        let required = |name: &str| {
            env::var(name).map_err(|_| {
                DomainError::InvalidInput(format!("{name} is required for AWS storage"))
            })
        };
        let (dynamodb_table, s3_bucket) = match mode {
            StorageMode::Local => (None, None),
            StorageMode::Aws => (
                Some(required("CONTEXTBRIDGE_DYNAMODB_TABLE")?),
                Some(required("CONTEXTBRIDGE_S3_BUCKET")?),
            ),
        };
        Ok(Self {
            mode,
            dynamodb_table,
            s3_bucket,
            region: env::var("AWS_REGION")
                .or_else(|_| env::var("AWS_DEFAULT_REGION"))
                .ok(),
        })
    }
}

#[derive(Clone)]
pub struct S3EvidenceStore {
    client: S3Client,
    bucket: String,
}

impl S3EvidenceStore {
    pub fn new(client: S3Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    fn parse_handle(handle: &str) -> Result<(String, String), DomainError> {
        let rest = handle
            .strip_prefix("artifact://")
            .ok_or_else(|| DomainError::InvalidInput("invalid artifact handle".into()))?;
        let (workspace, hash) = rest
            .split_once('/')
            .ok_or_else(|| DomainError::InvalidInput("artifact handle missing hash".into()))?;
        validate_workspace(workspace)?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(DomainError::InvalidInput(
                "artifact handle must contain a BLAKE3 hex digest".into(),
            ));
        }
        Ok((workspace.to_owned(), hash.to_owned()))
    }

    fn key(workspace: &str, hash: &str) -> String {
        format!("workspaces/{workspace}/evidence/{hash}")
    }

    fn is_existing_object_error<E: std::fmt::Display>(
        error: &aws_sdk_s3::error::SdkError<E>,
    ) -> bool {
        let text = error.to_string();
        text.contains("PreconditionFailed") || text.contains("412")
    }
}

#[async_trait]
impl EvidenceStore for S3EvidenceStore {
    async fn put_raw(
        &self,
        workspace: &WorkspaceId,
        evidence_id: &crate::EvidenceId,
        content: &[u8],
    ) -> Result<String, DomainError> {
        validate_workspace(&workspace.0)?;
        let hash = blake3::hash(content).to_hex().to_string();
        let handle = format!("artifact://{}/{hash}", workspace.0);
        let key = Self::key(&workspace.0, &hash);
        let result = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .if_none_match("*")
            .metadata("contextbridge-evidence-id", &evidence_id.0)
            .metadata("contextbridge-workspace", &workspace.0)
            .body(ByteStream::from(content.to_vec()))
            .send()
            .await;
        match result {
            Ok(_) => Ok(handle),
            Err(error) if Self::is_existing_object_error(&error) => Ok(handle),
            Err(error) => Err(DomainError::Storage(format!(
                "S3 put evidence failed: {error}"
            ))),
        }
    }

    async fn recover_raw(&self, handle: &str) -> Result<Vec<u8>, DomainError> {
        let (workspace, hash) = Self::parse_handle(handle)?;
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(Self::key(&workspace, &hash))
            .send()
            .await
            .map_err(|error| {
                let code = error
                    .as_service_error()
                    .and_then(|service| service.code())
                    .unwrap_or_default();
                if code == "NoSuchKey" || code == "NotFound" || error.to_string().contains("404") {
                    DomainError::NotFound(format!("raw evidence {handle}"))
                } else {
                    DomainError::Storage(format!("S3 get evidence failed: {error}"))
                }
            })?;
        output
            .body
            .collect()
            .await
            .map(|bytes| bytes.into_bytes().to_vec())
            .map_err(|error| DomainError::Storage(format!("S3 body read failed: {error}")))
    }
}

#[derive(Clone)]
pub struct DynamoDbStorage {
    client: DynamoDbClient,
    table: String,
    evidence: Arc<dyn EvidenceStore>,
}

impl DynamoDbStorage {
    pub fn new(
        client: DynamoDbClient,
        table: impl Into<String>,
        evidence: Arc<dyn EvidenceStore>,
    ) -> Self {
        Self {
            client,
            table: table.into(),
            evidence,
        }
    }

    fn workspace_key(workspace: &WorkspaceId) -> String {
        format!("WORKSPACE#{}", workspace.0)
    }

    fn entity_key(kind: &str, id: &str) -> String {
        format!("{kind}#{id}")
    }

    fn base(kind: &str, id: &str, workspace: &WorkspaceId) -> HashMap<String, AttributeValue> {
        let mut item = HashMap::new();
        item.insert(PK.into(), s(Self::workspace_key(workspace)));
        item.insert(SK.into(), s(Self::entity_key(kind, id)));
        item.insert("entity".into(), s(kind));
        item
    }

    async fn get_item(
        &self,
        workspace: &WorkspaceId,
        sort_key: &str,
    ) -> Result<Option<HashMap<String, AttributeValue>>, DomainError> {
        self.client
            .get_item()
            .table_name(&self.table)
            .key(PK, s(Self::workspace_key(workspace)))
            .key(SK, s(sort_key))
            .consistent_read(true)
            .send()
            .await
            .map(|output| output.item)
            .map_err(|error| aws_error("DynamoDB get item", error))
    }

    async fn query_prefix(
        &self,
        workspace: &WorkspaceId,
        prefix: &str,
    ) -> Result<Vec<HashMap<String, AttributeValue>>, DomainError> {
        let mut items = Vec::new();
        let mut start = None;
        loop {
            let mut request = self
                .client
                .query()
                .table_name(&self.table)
                .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
                .expression_attribute_names("#pk", PK)
                .expression_attribute_names("#sk", SK)
                .expression_attribute_values(":pk", s(Self::workspace_key(workspace)))
                .expression_attribute_values(":prefix", s(prefix))
                .consistent_read(true);
            if let Some(key) = start {
                request = request.set_exclusive_start_key(Some(key));
            }
            let output = request
                .send()
                .await
                .map_err(|error| aws_error("DynamoDB query", error))?;
            if let Some(page) = output.items {
                items.extend(page);
            }
            start = output.last_evaluated_key;
            if start.is_none() {
                break;
            }
        }
        Ok(items)
    }

    async fn put_item(&self, item: HashMap<String, AttributeValue>) -> Result<(), DomainError> {
        self.client
            .put_item()
            .table_name(&self.table)
            .set_item(Some(item))
            .condition_expression("attribute_not_exists(#pk)")
            .expression_attribute_names("#pk", PK)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| {
                if error.to_string().contains("ConditionalCheckFailed") {
                    DomainError::Conflict("DynamoDB item already exists".into())
                } else {
                    aws_error("DynamoDB put item", error)
                }
            })
    }
}

#[async_trait]
impl EvidenceStore for DynamoDbStorage {
    async fn put_raw(
        &self,
        workspace: &WorkspaceId,
        evidence_id: &crate::EvidenceId,
        content: &[u8],
    ) -> Result<String, DomainError> {
        self.evidence.put_raw(workspace, evidence_id, content).await
    }

    async fn recover_raw(&self, handle: &str) -> Result<Vec<u8>, DomainError> {
        self.evidence.recover_raw(handle).await
    }
}

#[async_trait]
impl Storage for DynamoDbStorage {
    async fn append_event(
        &self,
        workspace: &WorkspaceId,
        agent: &AgentId,
        event: &EventInput,
    ) -> Result<crate::EventRecord, DomainError> {
        if event.content.trim().is_empty() {
            return Err(DomainError::InvalidInput(
                "event content cannot be empty".into(),
            ));
        }
        let id = event
            .event_id
            .clone()
            .unwrap_or_else(|| EventId(Uuid::new_v4().to_string()));
        let observed = event.observed_at.unwrap_or_else(Utc::now);
        let received = Utc::now();
        let mut record = Self::base("EVENT", &id.0, workspace);
        record.insert("event_id".into(), s(&id.0));
        record.insert(PK.into(), s(format!("EVENT#{}", id.0)));
        record.insert(SK.into(), s("EVENT"));
        record.insert("workspace_id".into(), s(&workspace.0));
        record.insert("agent_id".into(), s(&agent.0));
        record.insert("kind".into(), s(kind_name(&event.kind)));
        record.insert("content".into(), s(&event.content));
        record.insert("observed_at".into(), s(observed.to_rfc3339()));
        record.insert("received_at".into(), s(received.to_rfc3339()));
        if let Some(subject) = &event.subject {
            record.insert("subject".into(), s(subject));
        }
        if let Some(raw) = &event.raw_evidence {
            record.insert("raw_evidence".into(), s(raw));
        }
        record.insert("promote".into(), AttributeValue::Bool(event.promote));
        if let Some(old) = &event.supersedes_memory_id {
            record.insert("supersedes_memory_id".into(), s(&old.0));
        }

        let mut activity = Self::base(
            "ACTIVITY",
            &format!("{}#{}", received.to_rfc3339(), id.0),
            workspace,
        );
        activity.extend(
            record
                .iter()
                .filter(|(key, _)| !matches!(key.as_str(), "pk" | "sk" | "entity"))
                .map(|(key, value)| (key.clone(), value.clone())),
        );

        self.client
            .transact_write_items()
            .transact_items(
                TransactWriteItem::builder()
                    .put(
                        Put::builder()
                            .set_item(Some(record))
                            .condition_expression("attribute_not_exists(#pk)")
                            .expression_attribute_names("#pk", PK)
                            .build()
                            .map_err(|e| DomainError::Storage(e.to_string()))?,
                    )
                    .build(),
            )
            .transact_items(
                TransactWriteItem::builder()
                    .put(
                        Put::builder()
                            .set_item(Some(activity))
                            .condition_expression("attribute_not_exists(#pk)")
                            .expression_attribute_names("#pk", PK)
                            .build()
                            .map_err(|e| DomainError::Storage(e.to_string()))?,
                    )
                    .build(),
            )
            .send()
            .await
            .map_err(|error| {
                if error.to_string().contains("ConditionalCheckFailed") {
                    DomainError::Conflict(format!("event already exists: {}", id.0))
                } else {
                    aws_error("DynamoDB append event", error)
                }
            })?;
        Ok(crate::EventRecord {
            event_id: id,
            workspace_id: workspace.clone(),
            agent_id: agent.clone(),
            kind: event.kind.clone(),
            content: event.content.clone(),
            promoted_memory_id: None,
            received_at: received,
        })
    }
    async fn mark_event_promoted(
        &self,
        event_id: &EventId,
        memory_id: &MemoryId,
    ) -> Result<(), DomainError> {
        let item = self
            .client
            .get_item()
            .table_name(&self.table)
            .key(PK, s(format!("EVENT#{}", event_id.0)))
            .key(SK, s("EVENT"))
            .consistent_read(true)
            .send()
            .await
            .map_err(|error| aws_error("DynamoDB find event", error))?
            .item
            .ok_or_else(|| DomainError::NotFound(format!("event {}", event_id.0)))?;
        let workspace = required(&item, "workspace_id")?;
        let received = required(&item, "received_at")?;
        let event_update = Update::builder()
            .table_name(&self.table)
            .key(PK, s(format!("EVENT#{}", event_id.0)))
            .key(SK, s("EVENT"))
            .update_expression("SET promoted_memory_id = :memory")
            .expression_attribute_values(":memory", s(&memory_id.0))
            .build()
            .map_err(|error| DomainError::Storage(error.to_string()))?;
        let activity_update = Update::builder()
            .table_name(&self.table)
            .key(PK, s(format!("WORKSPACE#{workspace}")))
            .key(SK, s(format!("ACTIVITY#{received}#{}", event_id.0)))
            .update_expression("SET promoted_memory_id = :memory")
            .expression_attribute_values(":memory", s(&memory_id.0))
            .build()
            .map_err(|error| DomainError::Storage(error.to_string()))?;
        self.client
            .transact_write_items()
            .transact_items(TransactWriteItem::builder().update(event_update).build())
            .transact_items(TransactWriteItem::builder().update(activity_update).build())
            .send()
            .await
            .map(|_| ())
            .map_err(|error| aws_error("DynamoDB mark event promoted", error))
    }

    async fn store_memory(
        &self,
        workspace: &WorkspaceId,
        agent: &AgentId,
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
        validate_workspace(&workspace.0)?;
        let id = MemoryId(Uuid::new_v4().to_string());
        let evidence_id = crate::EvidenceId(Uuid::new_v4().to_string());
        let handle = self
            .put_raw(workspace, &evidence_id, raw.unwrap_or(content).as_bytes())
            .await?;
        let observed = Utc::now();
        let mut item = Self::base("MEMORY", &id.0, workspace);
        item.insert("kind".into(), s(kind_name(kind)));
        item.insert("content".into(), s(content));
        item.insert("status".into(), s("current"));
        item.insert("source_agent".into(), s(&agent.0));
        item.insert("observed_at".into(), s(observed.to_rfc3339()));
        item.insert("evidence_id".into(), s(&evidence_id.0));
        item.insert("raw_handle".into(), s(&handle));
        if let Some(subject) = subject {
            item.insert("subject".into(), s(subject));
        }
        if let Some(event) = event {
            item.insert("source_event_id".into(), s(&event.0));
        }
        self.put_item(item).await?;
        Ok(Memory {
            id,
            workspace_id: workspace.clone(),
            kind: kind.clone(),
            subject: subject.map(str::to_owned),
            content: content.to_owned(),
            status: MemoryStatus::Current,
            provenance: Provenance {
                source_agent: agent.clone(),
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

    async fn get_memory(
        &self,
        workspace: &WorkspaceId,
        id: &MemoryId,
    ) -> Result<Memory, DomainError> {
        self.get_item(workspace, &Self::entity_key("MEMORY", &id.0))
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("memory {}", id.0)))
            .and_then(decode_memory)
    }

    async fn list_memories(
        &self,
        workspace: &WorkspaceId,
        status: Option<&MemoryStatus>,
        limit: u32,
    ) -> Result<Vec<Memory>, DomainError> {
        let mut memories = self
            .query_prefix(workspace, "MEMORY#")
            .await?
            .into_iter()
            .map(decode_memory)
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(status) = status {
            memories.retain(|memory| &memory.status == status);
        }
        memories.sort_by(|a, b| {
            b.provenance
                .observed_at
                .cmp(&a.provenance.observed_at)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        memories.truncate(limit.min(200) as usize);
        Ok(memories)
    }

    async fn supersede(
        &self,
        workspace: &WorkspaceId,
        old: &MemoryId,
        new: &MemoryId,
        reason: &str,
    ) -> Result<SupersedeResponse, DomainError> {
        if old == new {
            return Err(DomainError::InvalidInput(
                "a memory cannot supersede itself".into(),
            ));
        }
        let old_memory = self.get_memory(workspace, old).await?;
        let new_memory = self.get_memory(workspace, new).await?;
        if old_memory.status == MemoryStatus::Superseded
            && old_memory.superseded_by.as_ref() == Some(new)
        {
            return Ok(SupersedeResponse {
                superseded: old_memory,
                current: new_memory,
                reason: reason.to_owned(),
            });
        }
        if new_memory.status != MemoryStatus::Current {
            return Err(DomainError::Conflict("replacement must be current".into()));
        }
        let update = Update::builder()
            .table_name(&self.table)
            .key(PK, s(Self::workspace_key(workspace)))
            .key(SK, s(Self::entity_key("MEMORY", &old.0)))
            .update_expression("SET #status = :superseded, superseded_by = :replacement")
            .condition_expression("#status = :current AND attribute_not_exists(superseded_by)")
            .expression_attribute_names("#status", "status")
            .expression_attribute_values(":superseded", s("superseded"))
            .expression_attribute_values(":current", s("current"))
            .expression_attribute_values(":replacement", s(&new.0))
            .build()
            .map_err(|error| DomainError::Storage(error.to_string()))?;
        let check = ConditionCheck::builder()
            .table_name(&self.table)
            .key(PK, s(Self::workspace_key(workspace)))
            .key(SK, s(Self::entity_key("MEMORY", &new.0)))
            .condition_expression("#status = :current")
            .expression_attribute_names("#status", "status")
            .expression_attribute_values(":current", s("current"))
            .build()
            .map_err(|error| DomainError::Storage(error.to_string()))?;
        let mut history = Self::base("SUPERSESSION", &old.0, workspace);
        history.insert("replacement_memory_id".into(), s(&new.0));
        history.insert("reason".into(), s(reason));
        history.insert("created_at".into(), s(Utc::now().to_rfc3339()));
        let put = Put::builder()
            .set_item(Some(history))
            .condition_expression("attribute_not_exists(#pk)")
            .expression_attribute_names("#pk", PK)
            .build()
            .map_err(|error| DomainError::Storage(error.to_string()))?;
        self.client
            .transact_write_items()
            .transact_items(TransactWriteItem::builder().update(update).build())
            .transact_items(TransactWriteItem::builder().condition_check(check).build())
            .transact_items(TransactWriteItem::builder().put(put).build())
            .send()
            .await
            .map_err(|error| {
                if error.to_string().contains("ConditionalCheckFailed") {
                    DomainError::Conflict("supersession condition failed".into())
                } else {
                    aws_error("DynamoDB supersession transaction", error)
                }
            })?;
        Ok(SupersedeResponse {
            superseded: self.get_memory(workspace, old).await?,
            current: self.get_memory(workspace, new).await?,
            reason: reason.to_owned(),
        })
    }

    async fn list_activity(
        &self,
        workspace: &WorkspaceId,
        limit: u32,
    ) -> Result<Vec<ActivityRecord>, DomainError> {
        let mut records = self
            .query_prefix(workspace, "ACTIVITY#")
            .await?
            .into_iter()
            .map(decode_activity)
            .collect::<Result<Vec<_>, _>>()?;
        records.sort_by_key(|a| std::cmp::Reverse(a.event.received_at));
        records.truncate(limit.min(200) as usize);
        Ok(records)
    }
}

fn decode_memory(item: HashMap<String, AttributeValue>) -> Result<Memory, DomainError> {
    let workspace = required(&item, PK)?
        .strip_prefix("WORKSPACE#")
        .ok_or_else(|| DomainError::Storage("memory has malformed workspace key".into()))?
        .to_owned();
    let id = required(&item, SK)?
        .strip_prefix("MEMORY#")
        .ok_or_else(|| DomainError::Storage("memory has malformed sort key".into()))?
        .to_owned();
    Ok(Memory {
        id: MemoryId(id),
        workspace_id: WorkspaceId(workspace),
        kind: MemoryKind::try_from(required(&item, "kind")?.as_str())?,
        subject: get_s(&item, "subject"),
        content: required(&item, "content")?,
        status: MemoryStatus::try_from(required(&item, "status")?.as_str())?,
        provenance: Provenance {
            source_agent: AgentId(required(&item, "source_agent")?),
            event_id: get_s(&item, "source_event_id").map(EventId),
            observed_at: parse_time(&required(&item, "observed_at")?)?,
        },
        evidence: RawEvidenceHandle {
            handle: required(&item, "raw_handle")?,
            evidence_id: crate::EvidenceId(required(&item, "evidence_id")?),
        },
        superseded_by: get_s(&item, "superseded_by").map(MemoryId),
    })
}

fn decode_activity(item: HashMap<String, AttributeValue>) -> Result<ActivityRecord, DomainError> {
    let id = EventId(required(&item, "event_id")?);
    let promoted = get_s(&item, "promoted_memory_id").map(MemoryId);
    Ok(ActivityRecord {
        event: crate::EventRecord {
            event_id: id.clone(),
            workspace_id: WorkspaceId(
                required(&item, PK)?
                    .strip_prefix("WORKSPACE#")
                    .unwrap_or_default()
                    .to_owned(),
            ),
            agent_id: AgentId(required(&item, "agent_id")?),
            kind: MemoryKind::try_from(required(&item, "kind")?.as_str())?,
            content: required(&item, "content")?,
            promoted_memory_id: promoted.clone(),
            received_at: parse_time(&required(&item, "received_at")?)?,
        },
        outcome: promoted.map(|memory_id| EventOutcome {
            event_id: id,
            outcome: "PROMOTED".into(),
            promoted_memory_id: Some(memory_id),
            raw_handle: None,
        }),
    })
}

fn required(item: &HashMap<String, AttributeValue>, name: &str) -> Result<String, DomainError> {
    get_s(item, name)
        .ok_or_else(|| DomainError::Storage(format!("persisted record missing {name}")))
}

fn get_s(item: &HashMap<String, AttributeValue>, name: &str) -> Option<String> {
    item.get(name).and_then(|value| value.as_s().ok()).cloned()
}

fn s(value: impl Into<String>) -> AttributeValue {
    AttributeValue::S(value.into())
}

fn kind_name(kind: &MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Decision => "decision",
        MemoryKind::Finding => "finding",
        MemoryKind::Constraint => "constraint",
        MemoryKind::Task => "task",
        MemoryKind::State => "state",
    }
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, DomainError> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| DomainError::Storage(format!("invalid persisted timestamp: {error}")))
}

fn validate_workspace(workspace: &str) -> Result<(), DomainError> {
    if workspace.is_empty()
        || workspace.len() > 128
        || !workspace
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(DomainError::InvalidInput(
            "workspace must contain only letters, numbers, '-', '_' or '.'".into(),
        ));
    }
    Ok(())
}

fn aws_error<E: std::fmt::Display>(operation: &str, error: E) -> DomainError {
    DomainError::Storage(format!("{operation}: {error}"))
}
