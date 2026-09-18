# ContextBridge agent instructions

## Mission

Build shared project intelligence for independent AI agents. The product's proof is Agent B completing a task with a bounded packet built from Agent A's useful knowledge, without receiving Agent A's conversation.

## Repository boundaries

- Work only in `contextbridge/`.
- `../second-brain` and `../effimiser` are read-only references. Never edit, format, reset, clean, stash, checkout, commit, or generate files there.
- Do not copy reference code, tests, fixtures, comments, or data without an explicit provenance and license decision.
- Prefer clean-room adaptation of observed behavior.

## Design rules

- Deterministic logic owns identity, workspace isolation, lifecycle, supersession, filtering, budget enforcement and packet structure.
- AWS or Bedrock may assist extraction/ranking only behind a testable boundary; cloud use must perform meaningful product work.
- Raw evidence remains addressable. A summary without a raw handle is invalid when information was omitted.
- Stale or superseded decisions stay queryable as history but must not appear as current truth.
- Never optimize packet size alone. A task failure or required-evidence miss is a regression.
- Do not silently introduce infrastructure, providers, queues, services, or abstraction layers.

## API discipline

The contract in `docs/api-contract.md` is the frontend/backend boundary. Breaking changes update schemas, fixtures, docs and compatibility notes together. Server-derived `agent_id` overrides any client-supplied identity.

## Testing and benchmarks

- Every behavior has unit or integration proof.
- Golden-path tests cover event ingestion, promotion, supersession and bounded context.
- Benchmarks compare cold, naive and ContextBridge arms under identical model, repository state, task and environment.
- Never publish unsupported performance claims. Store raw benchmark artifacts.

## Security

Validate workspace scope, request sizes, handles and paths. Do not persist secrets. Keep raw evidence access authenticated and auditable. Use least-privilege AWS roles and local fakes for tests.

## Git hygiene

Keep commits narrow and runnable. Explain material architecture decisions in `docs/decisions/`. Stop at the requested cut line; do not build speculative features.


## Monorepo ownership

ContextBridge is one repository. Keep one Rust crate until dependency boundaries justify a split.

- Bhuvan owns `crates/`, backend/API/auth, domain semantics, context selection, storage/evidence ports, future AWS work, infrastructure and integrations.
- Prathick owns `apps/web/`, product UX, Context Inspector and `benchmarks/`.
- Shared ownership: `fixtures/api/`, `docs/api-contract.md`, `docs/frontend-handoff.md`, `docs/benchmark-plan.md` and `docs/demo/`.
- Add only directories with current purpose. Do not scaffold speculative `infra/`, `integrations/`, frontend source or benchmark runners.
- Shared contract changes update implementation, docs, fixtures and relevant tests together.
- Context Inspector is primary product surface: show what context was sent, why, provenance, current state, raw handle and token usage.
- Benchmark claims require reproducible results. Correctness and required-evidence recall outrank token reduction.
- Short-lived branches: `bhuvan/<feature>` or `prathick/<feature>`, merged into green `main`. Avoid long-lived develop/staging branches.
