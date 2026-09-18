use crate::{
    AgentId, DomainError, EventId, EventInput, MemoryKind, MemoryStatus, Storage, WorkspaceId,
};

/// Reusable behavioral contract for structured and evidence adapters.
/// Future AWS adapter tests should call this with the same assertions.
pub async fn storage_contract<S: Storage>(storage: &S) -> Result<(), DomainError> {
    let alpha = WorkspaceId("contract-alpha".into());
    let beta = WorkspaceId("contract-beta".into());
    let agent = AgentId("contract-agent".into());
    let event = EventInput {
        event_id: Some(EventId("contract-event".into())),
        kind: MemoryKind::Finding,
        subject: Some("storage".into()),
        content: "Local storage preserves evidence.".into(),
        observed_at: None,
        raw_evidence: Some("full storage evidence".into()),
        promote: true,
        supersedes_memory_id: None,
    };
    let event_record = storage.append_event(&alpha, &agent, &event).await?;
    let first = storage
        .store_memory(
            &alpha,
            &agent,
            Some(&event_record.event_id),
            &event.kind,
            event.subject.as_deref(),
            &event.content,
            event.raw_evidence.as_deref(),
        )
        .await?;
    storage
        .mark_event_promoted(&event_record.event_id, &first.id)
        .await?;
    assert_eq!(
        storage
            .get_memory(&alpha, &first.id)
            .await?
            .provenance
            .source_agent,
        agent
    );
    assert_eq!(storage.list_memories(&beta, None, 100).await?.len(), 0);
    assert_eq!(storage.list_activity(&alpha, 100).await?.len(), 1);
    assert_eq!(
        storage.recover_raw(&first.evidence.handle).await?,
        b"full storage evidence"
    );

    let old = storage
        .store_memory(
            &alpha,
            &agent,
            None,
            &MemoryKind::Decision,
            Some("persistence"),
            "Persistence uses Redis.",
            None,
        )
        .await?;
    let new = storage
        .store_memory(
            &alpha,
            &agent,
            None,
            &MemoryKind::Decision,
            Some("persistence"),
            "Persistence uses DynamoDB.",
            None,
        )
        .await?;
    let beta_memory = storage
        .store_memory(
            &beta,
            &agent,
            None,
            &MemoryKind::Decision,
            Some("persistence"),
            "Persistence uses PostgreSQL.",
            None,
        )
        .await?;
    let result = storage
        .supersede(&alpha, &old.id, &new.id, "new decision")
        .await?;
    assert_eq!(result.superseded.status, MemoryStatus::Superseded);
    assert_eq!(result.current.status, MemoryStatus::Current);
    assert_eq!(
        storage
            .supersede(&alpha, &old.id, &new.id, "repeat")
            .await?
            .current
            .id,
        new.id
    );
    assert_eq!(
        storage.get_memory(&alpha, &old.id).await?.status,
        MemoryStatus::Superseded
    );
    assert_eq!(
        storage
            .list_memories(&alpha, Some(&MemoryStatus::Current), 100)
            .await?
            .iter()
            .filter(|m| m.id == old.id)
            .count(),
        0
    );
    assert_eq!(
        storage
            .list_memories(&alpha, Some(&MemoryStatus::Superseded), 100)
            .await?
            .iter()
            .filter(|m| m.id == old.id)
            .count(),
        1
    );
    assert!(matches!(
        storage
            .supersede(&alpha, &old.id, &beta_memory.id, "cross workspace")
            .await,
        Err(DomainError::NotFound(_) | DomainError::WorkspaceBoundary)
    ));
    assert!(matches!(
        storage
            .supersede(
                &alpha,
                &old.id,
                &crate::MemoryId("missing".into()),
                "missing"
            )
            .await,
        Err(DomainError::NotFound(_))
    ));
    assert!(matches!(
        storage.append_event(&alpha, &agent, &event).await,
        Err(DomainError::Conflict(_))
    ));
    Ok(())
}
