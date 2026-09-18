# Provenance and licensing

## ContextBridge evidence

Every packet item records `evidence_id`, source agent, observed time, memory/event identity, retrieval reason, status, workspace, estimated tokens, and `raw_handle` when representation omits detail. Raw content is immutable or versioned; updates create new evidence identity.

## Reference findings

| Reference | Classification | Rationale |
|---|---|---|
| second-brain API/auth/event policy | ADAPT | Working Rust/Axum/SQLite implementation proves useful boundaries; redesign identity/workspace and contract for ContextBridge. No source copied at this stage. |
| second-brain hybrid retrieval/context tests | LEARN | Deterministic FTS + semantic + bounded packing is implemented, but V2.4 remains in progress and graph path is optional. Validate independently. |
| second-brain event ledger/promotion | ADAPT | Event != memory, server-derived identity, dedupe and selective promotion fit product. Reimplement against new entities. |
| second-brain Obsidian/Graphify | IGNORE for MVP | Human vault and local code graph are outside golden path; revisit only with measured need. |
| Effimiser stable handles/raw blobs | LEARN; possible ADAPT later | Content-addressed bytes, idempotent writes and progressive disclosure fit. No code copied. |
| Effimiser provenance/token accounting | ADAPT concept | Stable source/hash/method/raw references and real tokenizer labeling fit; use own types and verify tokenizer/model limits. |
| Effimiser cache hierarchy/snapshot-delta | LEARN | Useful for future evidence reads, not MVP dependency. |
| Effimiser benchmark discipline | ADAPT | Correctness before savings, negative controls and raw artifacts are mandatory. |
| Effimiser planned compiler/MCP/index | IGNORE for MVP | Actual repository marks these planned or partial; do not treat roadmap as built. |

## License posture

`effimiser` declares Apache-2.0 and contains `LICENSE`/`NOTICE`; direct reuse is legally possible with attribution, but clean-room reimplementation remains preferred. `second-brain` has no verified project license in inspected root; treat it as internal reference, not distributable source. Current ContextBridge reuses no reference code.

If direct code enters ContextBridge, record file origin, commit, copyright/license text and NOTICE update in the same change. Do not copy fixtures or comments by default.

