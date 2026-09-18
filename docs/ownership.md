# Team ownership

## Bhuvan

Owns backend and core: `crates/`, domain semantics, context selection,
storage/evidence ports, local adapters, future `infra/`, AWS adapters, API,
authentication boundaries, agent integrations and future MCP.

## Prathick

Owns `apps/web/`, frontend/dashboard, Context Inspector, product UX,
`benchmarks/`, evaluation UX, demo presentation and blog/submission material.

## Shared

Owns `fixtures/api/`, `docs/api-contract.md`,
`docs/frontend-handoff.md`, `docs/benchmark-plan.md` and `docs/demo/`.

Any shared API shape change updates implementation, docs, fixtures and relevant
tests in the same commit. Keep changes additive/backward-compatible where
practical and communicate contract changes before merging.

## Boundaries

ContextBridge stays one repository and one Rust crate for now. Add directories
only when they own real work. `infra/` and `integrations/` wait for their
implementation milestones. `../second-brain` and `../effimiser` remain
read-only references.
