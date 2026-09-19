# Roadmap and ownership

## Hackathon schedule

- **Day 1 — Architecture and foundations:** reconnaissance, local core and AWS adapter implementation. Completed in #1–#5.
- **Day 2 — Feature completion:** #6–#15. AWS verification is currently blocked on non-root Identity Center authentication.
- **Day 3 — Validation and submission:** #16–#20. Refinement, real benchmarks, demo rehearsal, final regression and submission polish.

The master tracker is [EPIC: ContextBridge Hackathon MVP](https://github.com/Bbn08/contextbridge/issues/21).

## Completed

- **M0 — Recon / Architecture:** reference inspection, contracts, provenance and ownership boundaries. Closed in #1.
- **M1 — Local Vertical Slice:** local API, SQLite, events, typed memories, supersession, bounded context, provenance, raw recovery and tests. Closed in #2.
- **M1.5 — Monorepo/team structure:** ownership boundaries and parallel-work contracts. Closed in #3.
- **Publication — Apache-2.0/public repository:** closed in #4.

## Bhuvan track

- **M2 — AWS Persistence:** DynamoDB structured storage and S3 evidence adapter. Implemented in #5; real verification is #6.
- **M3 — Intelligence:** automatic/deterministic promotion with optional Bedrock assistance. #7.
- **M4 — Context Compiler:** bounded current-state packet hardening. #8.
- **M5 — Agent Integration:** minimal vendor-neutral external-agent surface. #9.
- **M6 — Multi-Agent Demo:** complete Redis-to-DynamoDB handoff flow. #10.

## Prathick track

- **P1 — Frontend shell + fixture client:** #11.
- **P2 — Context Inspector:** #12.
- **P3 — Memories/activity/supersession UI:** #13.
- **P4 — Benchmark harness:** #15.
- **P5 — Demo and benchmark visualization:** included in #18.
- **P6 — Submission/blog polish:** #19.

## Shared execution

- **D1 — Reproducible demo seed/scenario:** #14.
- **Day-3 hardening, benchmark execution, demo polish, documentation and final regression:** #16–#20.

## Parallel work

P1 can start from `fixtures/api/`, `docs/api-contract.md` and
`docs/frontend-handoff.md`. P2/P3 depend on the fixture client, not deployed
backend. B1 can begin from the scenario specification without running yet.
M2.5 blocks AWS-dependent backend claims but does not block frontend fixture
work. The backend critical path is #6 -> #7 -> #9 -> #10 -> #14.

Frontend path: #11 -> #12 -> #13. Benchmark harness #15 can proceed from
the existing scenario and stabilizes after #10; final execution is #17.

No stage claims performance improvement until measured artifacts exist.
