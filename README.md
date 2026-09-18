# ContextBridge

Shared project intelligence for independent AI agents: preserve project
knowledge once, then give each agent the smallest useful task-specific slice.

## Problem

Independent agents working on one project repeatedly rediscover decisions,
state, failures and prior work. Sharing entire conversations fixes recall by
creating noise, stale truth and unnecessary context.

## What ContextBridge does

```text
Agent A event
    |
    v
ContextBridge event + provenance
    |
    v
current typed project knowledge
    |
    v
bounded task context packet
    |
    v
Agent B continues without Agent A's conversation
```

ContextBridge keeps event history separate from promoted memory. Current
knowledge can supersede historical decisions, while raw evidence remains
recoverable through stable handles.

## Core principles

- Current truth over stale history
- Provenance on shared knowledge
- Bounded, task-specific context
- Workspace isolation
- Raw evidence recoverability
- Deterministic-first selection
- Correctness before token reduction

## Current status

Active AWS hackathon MVP. Milestone 1 local vertical slice works today:

- Rust/Axum API with bearer-token, server-derived agent identity
- SQLite structured persistence behind `Storage`
- raw evidence behind `EvidenceStore`, content-addressed with BLAKE3
- event retention and deterministic promotion
- typed memories and decision supersession
- workspace isolation
- deterministic lexical context selection
- `o200k_base` token accounting, explicitly labelled as a proxy
- golden-path, security, storage-contract and fixture-shape tests

AWS adapters, Bedrock, AgentCore, MCP, semantic retrieval and frontend are
future work. No benchmark improvement is claimed yet.

## Example

Agent A records:

```text
Persistence uses Redis.
```

Agent B asks for persistence context and receives Redis with Agent A
provenance. Agent A later records:

```text
Persistence uses DynamoDB.
```

DynamoDB becomes current truth. Redis remains historical and recoverable, but
is not presented as current context.

## Architecture

```text
domain/API types
      ^
context selection + token budget
      ^
Storage and EvidenceStore ports
      ^
Local SQLite adapter today
      ^
DynamoDB/S3 adapters later
```

Milestone 1 intentionally keeps one Rust crate. Internal module extraction
will follow stable ownership boundaries, not directory aesthetics.

## Run locally

Requires Rust and Cargo.

```bash
cargo run -p contextbridge
```

The server listens on `127.0.0.1:3000`. Local bearer tokens are
`agent-a-token` and `agent-b-token`. The default database is
`data/contextbridge.db`; runtime data is ignored by Git.

## API

See [docs/api-contract.md](docs/api-contract.md) and
[docs/frontend-handoff.md](docs/frontend-handoff.md).

- `POST /v1/events`
- `POST /v1/memories`
- `GET /v1/memories`
- `GET /v1/activity`
- `POST /v1/context`
- `POST /v1/memories/{id}/supersede`
- `GET /v1/evidence/{hash}`
- `GET /health`

API fixtures are under [fixtures/api](fixtures/api). Team ownership and parallel work are documented in [docs/ownership.md](docs/ownership.md). The primary future product surface is the [Context Inspector](docs/demo/golden-path.md).

## Testing

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

## Roadmap

Next review boundary: AWS-backed implementations of the existing structured
and raw-evidence ports, followed by the same behavioral contract tests.

Later candidates: Bedrock-assisted extraction or ranking, AgentCore
integration, MCP, frontend, and controlled A/B/C benchmarking. None are
implemented or claimed here.

## License

ContextBridge is licensed under the [Apache License 2.0](LICENSE). No third-party source code has been copied
into this repository. See [docs/provenance.md](docs/provenance.md).

## Team workspaces

Prathick can start frontend fixture work in [`apps/web/`](apps/web/README.md).
Benchmark specification lives in [`benchmarks/`](benchmarks/README.md). Bhuvan
continues backend and future AWS work from the existing ports.
