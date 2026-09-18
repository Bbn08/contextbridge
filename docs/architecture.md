# Proposed architecture

## Ownership

```text
agent / MCP client
        |
        v
API adapter (HTTP now; MCP adapter at same contract)
        |
application service
  identity + workspace scope
  event policy + memory lifecycle
  supersession/conflict state
  context compiler
        |
ports: structured store | raw evidence store | tokenizer | optional extractor/ranker
        |
local adapters                         AWS adapters
SQLite/DynamoDB                         DynamoDB state
filesystem/S3                           S3 raw evidence
fake model/local rules                   Bedrock extraction/ranking
```

## Evidence hierarchy under evaluation

L0 active agent context; L1 compiled packet; L2 typed shared state; L3 raw evidence; L4 repository/source systems. This is a retrieval model, not a promise that every layer exists in MVP.

## Core entities

- Workspace: isolation boundary and project identity.
- Agent: server-authenticated participant in a workspace.
- Event: append-oriented observation; not automatically a memory.
- Memory: typed durable knowledge with status, confidence, provenance and evidence handle.
- Decision: memory with subject, chosen value, rationale and supersession links.
- Evidence: compact or raw artifact reference with stable identity and token metadata.
- Context packet: bounded, explainable response assembled for one task.

## Packet stages

1. Validate workspace, agent and budget.
2. Read current state and active decisions.
3. Retrieve cheap deterministic candidates by workspace/type/subject/lexical match.
4. Add semantic or Bedrock ranking only when deterministic candidates are insufficient and the boundary is enabled.
5. Remove cross-workspace, secret, duplicate, stale-current and superseded-current candidates.
6. Pack by token budget, preserving required evidence and source reasons.
7. Return packet plus diagnostics and expandable handles.

No universal score is required in MVP. Selection reasons must be explicit.


## Milestone 1 implementation boundary

The current crate keeps the implementation compact but exposes two ports:

- `Storage`: events, memories, activity and supersession.
- `EvidenceStore`: content-addressed raw evidence insertion and recovery.

`LocalSqliteStorage` implements both ports today. Future DynamoDB and S3
adapters must satisfy the same behavior without entering domain types.

Context packet token usage counts selected evidence content with `o200k_base`.
It is a labelled proxy and does not count complete serialized packet overhead.
