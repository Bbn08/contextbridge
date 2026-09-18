# ContextBridge

Shared project intelligence for independent AI agents.

ContextBridge receives agent events, promotes valuable knowledge into typed project memories, tracks current state and supersession, then builds a bounded task-specific context packet. It gives agents the smallest useful slice of collective knowledge while keeping raw evidence recoverable.

## Golden path

1. Agent A emits an event.
2. ContextBridge records identity, workspace, timestamp and provenance.
3. Policy promotes valuable evidence into a typed memory or decision.
4. Agent B requests context for a task.
5. ContextBridge selects current, relevant evidence within the requested token budget.
6. Agent B receives source and raw handles, not Agent A's whole conversation.

## Status

Repository initialized at contract/reconnaissance stage. No product crate exists yet. See [docs/roadmap.md](docs/roadmap.md) and [docs/decisions/0001-initial-boundaries.md](docs/decisions/0001-initial-boundaries.md).

## Principles

- Correctness and required-evidence recall outrank token reduction.
- Compress visibility, not information.
- Every lossy representation needs recoverable raw evidence.
- Deterministic selection first; model assistance only where it adds measured value.
- Current truth must be distinguishable from superseded history.
- Project isolation and server-derived agent identity are security invariants.

## Local development

The workspace intentionally has no crates until the first approved implementation milestone. Frontend and benchmark work can begin against [docs/api-contract.md](docs/api-contract.md) and [fixtures/api](fixtures/api).

