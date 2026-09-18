# Roadmap and ownership

## Completed

- **M0 — Recon / Architecture:** reference inspection, contracts, provenance and ownership boundaries.
- **M1 — Local Vertical Slice:** local API, SQLite, events, typed memories, supersession, bounded context, provenance, raw recovery and tests.

## Bhuvan track

- **M2 — AWS Persistence:** DynamoDB structured storage and S3 evidence adapter, both passing existing behavioral contracts.
- **M3 — Intelligence:** optional Bedrock extraction/compression/ranking behind `EvidenceStore`/intelligence boundaries.
- **M4 — Agent Integration:** MCP and vendor-neutral agent adapters.

## Prathick track

- **P1 — Frontend shell + fixture client**
- **P2 — Context Inspector**
- **P3 — Memories/activity/supersession UI**
- **P4 — Benchmark harness**
- **P5 — Demo and benchmark visualization**
- **P6 — Submission/blog polish**

## Shared later milestone

- **M5 — Multi-Agent Demo:** Bhuvan backend and Prathick Context Inspector show the same Redis-to-DynamoDB supersession semantics.
- **M6 — Benchmark + Polish + Submission:** controlled A/B/C evaluation, demo hardening and submission material.

## Parallel work

P1 can start now from `fixtures/api/`, `docs/api-contract.md` and
`docs/frontend-handoff.md`. P2/P3 depend on the fixture client, not deployed
backend. P4 can begin from the scenario specification without running yet.
M2 depends only on the stable backend ports and contract tests; it does not
block frontend fixture work.

No stage claims performance improvement until measured artifacts exist.
