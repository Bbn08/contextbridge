use crate::{
    AgentId, DomainError, EventInput, EventRecord, Memory, MemoryKind, MemoryStatus,
    RawEvidenceHandle, Storage, WorkspaceId,
};

/// Deterministic event-to-memory policy for the local product path.
pub struct PromotionEngine;

#[derive(Debug)]
pub enum PromotionOutcome {
    Discarded { reason: String },
    Retained { reason: String },
    Promoted(Memory),
    Deduped(Memory),
}

impl PromotionOutcome {
    pub fn status(&self) -> &'static str {
        match self {
            Self::Discarded { .. } => "DISCARDED",
            Self::Retained { .. } => "RETAINED",
            Self::Promoted(_) => "PROMOTED",
            Self::Deduped(_) => "DEDUPED",
        }
    }

    pub fn memory_id(&self) -> Option<crate::MemoryId> {
        match self {
            Self::Promoted(memory) | Self::Deduped(memory) => Some(memory.id.clone()),
            Self::Discarded { .. } | Self::Retained { .. } => None,
        }
    }

    pub fn raw_handle(&self) -> Option<RawEvidenceHandle> {
        match self {
            Self::Promoted(memory) | Self::Deduped(memory) => Some(memory.evidence.clone()),
            Self::Discarded { .. } | Self::Retained { .. } => None,
        }
    }
}

impl PromotionEngine {
    pub async fn process(
        storage: &dyn Storage,
        workspace: &WorkspaceId,
        agent: &AgentId,
        event_record: &EventRecord,
        event: &EventInput,
    ) -> Result<PromotionOutcome, DomainError> {
        if contains_secret(event) {
            return Ok(PromotionOutcome::Retained {
                reason: "secret-like content was not promoted".into(),
            });
        }

        if !event.promote && !is_high_value(event.kind.clone()) {
            return Ok(PromotionOutcome::Retained {
                reason: "event kind is retained without promotion".into(),
            });
        }

        let current = storage
            .list_memories(workspace, Some(&MemoryStatus::Current), 200)
            .await?;

        if let Some(existing) = current.iter().find(|memory| {
            same_identity(
                memory,
                &event.kind,
                event.subject.as_deref(),
                &event.content,
            )
        }) {
            storage
                .mark_event_promoted(&event_record.event_id, &existing.id)
                .await?;
            return Ok(PromotionOutcome::Deduped(existing.clone()));
        }

        let memory = storage
            .store_memory(
                workspace,
                agent,
                Some(&event_record.event_id),
                &event.kind,
                event.subject.as_deref(),
                &event.content,
                event.raw_evidence.as_deref(),
            )
            .await?;
        storage
            .mark_event_promoted(&event_record.event_id, &memory.id)
            .await?;

        let mut superseded = Vec::new();
        if let Some(old) = &event.supersedes_memory_id {
            superseded.push(old.clone());
        }
        if matches!(event.kind, MemoryKind::Decision) {
            superseded.extend(
                current
                    .iter()
                    .filter(|candidate| {
                        candidate.kind == MemoryKind::Decision
                            && normalize_subject(candidate.subject.as_deref())
                                == normalize_subject(event.subject.as_deref())
                            && candidate.content != event.content
                    })
                    .map(|candidate| candidate.id.clone()),
            );
        }
        superseded.sort_by(|left, right| left.0.cmp(&right.0));
        superseded.dedup();
        for old in superseded {
            storage
                .supersede(
                    workspace,
                    &old,
                    &memory.id,
                    "automatic decision supersession",
                )
                .await?;
        }

        Ok(PromotionOutcome::Promoted(memory))
    }
}

fn is_high_value(kind: MemoryKind) -> bool {
    matches!(
        kind,
        MemoryKind::Decision
            | MemoryKind::Finding
            | MemoryKind::Constraint
            | MemoryKind::Task
            | MemoryKind::State
    )
}

fn same_identity(memory: &Memory, kind: &MemoryKind, subject: Option<&str>, content: &str) -> bool {
    memory.kind == *kind
        && normalize_subject(memory.subject.as_deref()) == normalize_subject(subject)
        && normalize(content) == normalize(&memory.content)
}

fn normalize_subject(subject: Option<&str>) -> String {
    normalize(subject.unwrap_or_default())
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn contains_secret(event: &EventInput) -> bool {
    [
        event.subject.as_deref().unwrap_or_default(),
        event.content.as_str(),
        event.raw_evidence.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .any(secret_like)
}

fn secret_like(value: &str) -> bool {
    let lower = value.to_lowercase();
    if lower.contains("-----begin ") && lower.contains("private key-----") {
        return true;
    }
    for marker in [
        "aws_secret_access_key",
        "secret_access_key",
        "api_key",
        "api-key",
        "authorization:",
        "password=",
        "passphrase=",
    ] {
        if lower.contains(marker) {
            return true;
        }
    }
    if let Some(rest) = lower.split("bearer ").nth(1) {
        let token = rest.split_whitespace().next().unwrap_or_default();
        if token.len() >= 16 {
            return true;
        }
    }
    lower.split_whitespace().any(|word| {
        let rest = word.strip_prefix("akia").unwrap_or_default();
        rest.len() >= 16 && rest.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_for_exact_deduplication() {
        assert_eq!(
            normalize(" Persistence   uses DynamoDB. "),
            "persistence uses dynamodb."
        );
        assert_eq!(normalize_subject(Some(" Persistence ")), "persistence");
    }

    #[test]
    fn rejects_obvious_synthetic_secrets() {
        let event = EventInput {
            event_id: None,
            kind: MemoryKind::Decision,
            subject: Some("credentials".into()),
            content: "Use this bearer abcdefghijklmnop".into(),
            observed_at: None,
            raw_evidence: None,
            promote: false,
            supersedes_memory_id: None,
        };
        assert!(contains_secret(&event));
    }
}
