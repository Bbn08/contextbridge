# API contract v0.1

Base path `/v1`. JSON. IDs are opaque strings. All workspace-scoped endpoints require authenticated server-derived agent identity. Mock responses live in `fixtures/api/`.

## `POST /v1/events`

Request:

```json
{"workspace_id":"demo","session_id":"session-a","events":[{"event_id":"evt-1","type":"DECISION","summary":"Persistence migrated from Redis to DynamoDB because X.","occurred_at":"2026-09-18T10:00:00Z","metadata":{"subject":"persistence","chosen":"DynamoDB","supersedes":"redis-decision"}}]}
```

Response returns per-event `accepted`, `promotion`, `memory_id`, `deduplicated` and `raw_handle`. Client `agent_id` is ignored/rejected; auth derives it.

## `POST /v1/memories`

Creates explicit typed knowledge. Required: workspace, `kind`, `content`. Optional: subject, status (`active|historical|stale|contested`), confidence, source event, raw handle, tags, observed time.

## `GET /v1/memories?workspace_id=demo&kind=decision&status=active`

Returns bounded list with current status and provenance. Default excludes historical/stale unless requested.

## `GET /v1/activity?workspace_id=demo&limit=50`

Returns event activity and promotion outcomes, never raw secrets.

## `POST /v1/context`

Request:

```json
{"workspace_id":"demo","query":"Implement persistence","max_tokens":1000,"include":{"current_state":true,"decisions":true,"evidence":true}}
```

Response:

```json
{"request_id":"ctx-1","workspace_id":"demo","query":"Implement persistence","summary":"Persistence uses DynamoDB; Redis is historical.","current_state":[{"key":"persistence","value":"DynamoDB","memory_id":"mem-dynamo"}],"evidence":[{"evidence_id":"mem-dynamo","kind":"decision","content":"DynamoDB superseded Redis. Reason: X.","source_agent":"agent-a","observed_at":"2026-09-18T10:00:00Z","status":"active","reasons":["subject match","current decision"],"estimated_tokens":24,"raw_handle":"memory://demo/mem-dynamo/raw"}],"token_usage":{"budget":1000,"estimated":47,"tokenizer":"declared-estimator-v0"},"diagnostics":{"excluded_superseded":["mem-redis"],"retrieval_methods":["structured","lexical"]}}
```

The backend must never claim exact model tokens when tokenizer is unavailable. `tokenizer` names measurement method.

## `POST /v1/memories/{id}/supersede`

Request: `{"replacement_memory_id":"mem-dynamo","reason":"DynamoDB is current persistence."}`. Response links both records and marks old record historical. Operation is idempotent and workspace-scoped.

## Compatibility rules

Unknown response fields may be added. Required fields and enum meanings cannot change without contract versioning. Breaking changes update this file and all fixtures before backend implementation.

