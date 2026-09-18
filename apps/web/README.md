# ContextBridge web workspace

Owner: Prathick

Purpose: human-facing ContextBridge dashboard and product experience.

Source of truth:

- [`../../fixtures/api/`](../../fixtures/api/)
- [`../../docs/api-contract.md`](../../docs/api-contract.md)
- [`../../docs/frontend-handoff.md`](../../docs/frontend-handoff.md)

Frontend work must run against fixtures without requiring a backend. Framework
choice is intentionally undecided; do not add frontend tooling until the team
agrees on it.

## Product priority

The first useful experience is Context Inspector. It should answer:

> What did ContextBridge tell this agent, and why?

Expected future surfaces, in priority order:

1. Context Inspector
2. Memories and current state
3. Activity and decision/supersession timeline
4. Evidence viewer
5. Benchmark results
6. Dashboard shell

Login, account settings, profiles and decorative analytics are not MVP goals.
