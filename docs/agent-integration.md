# Agent integration

## Chosen M2/DAY-2 surface

ContextBridge exposes a small authenticated HTTP/JSON interface. An external
agent needs only a workspace identifier, its bearer credential, event/context
schemas and evidence handles. It does not need to know about SQLite,
DynamoDB, S3, BLAKE3 or storage keys.

The stable agent flow is:

1. POST /v1/events with a typed event.
2. POST /v1/context with workspace, task query and token budget.
3. GET /v1/evidence/<hash>?workspace_id=<workspace> when raw evidence is needed.

The server derives agent identity from the bearer token. Clients must not send
or trust a caller-provided agent identity.

## Tool-shaped mapping

The HTTP contract maps directly to a minimal future tool surface:

- contextbridge_record_event -> POST /v1/events
- contextbridge_get_context -> POST /v1/context
- contextbridge_get_evidence -> GET /v1/evidence/<hash>

The HTTP handlers remain the business boundary; a future MCP adapter, if added,
must call the same application behavior rather than implement a second memory
lifecycle.

## Current limitation

MCP is intentionally not implemented in this milestone. No MCP client/server
compatibility claim is made. The tested HTTP boundary is sufficient for the
local multi-agent demo and can be wrapped later without changing domain
semantics.
