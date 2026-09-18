# Roadmap and ownership

## Parallel graph

```text
API contract + fixtures ─┬─> frontend/dashboard
                        ├─> benchmark harness/scenarios
                        └─> backend adapters
reference/license report ─> core entity model
core local service ───────> AWS adapters + MCP adapter
benchmark fixtures ───────> golden-path tests + result dashboard
```

## Bhuvan

1. Confirm entity/state model and implement local backend service.
2. Implement event ingestion, promotion, provenance and supersession.
3. Implement bounded context compiler and API.
4. Add DynamoDB/S3 ports and one meaningful cloud path.
5. Add MCP adapter and deployment only after local path passes.

## Prathick

1. Build dashboard against fixtures and contract.
2. Build benchmark runner and A/B/C result schema.
3. Add golden-path scenario tests and correctness assertions.
4. Integrate live API when stable; report observed metrics only.
5. Prepare technical narrative with provenance and limitations.

## First implementation milestone — complete

Local vertical slice: two agents, one workspace, event ingestion, decision promotion, explicit supersession, context response under budget, raw handle recovery, and fixture-backed contract tests. No embeddings, Bedrock, MCP, dashboard polish or deployment required.

## MVP cut line

Must work: workspace isolation; server-derived agent identity; event ingestion; typed decision/current state; provenance/raw handle; supersession; bounded context packet; local persistence; API fixtures; A/B/C benchmark scenario; one meaningful AWS-backed path for demo.

Drop if time runs short: semantic embeddings; automatic LLM extraction; AgentCore integration; code graph; generic MCP breadth; multi-region deployment; real-time activity polish; advanced conflict resolution; token-optimization claims.

