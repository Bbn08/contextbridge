# API contract v0.1

Base path `/v1`. JSON. Workspace-scoped endpoints require a bearer token.
Agent identity is derived server-side; clients must not send `agent_id`.

## `POST /v1/events`

```json
{"workspace_id":"demo","session_id":"session-a","events":[{"event_id":"evt-1","kind":"decision","subject":"persistence","content":"Persistence uses DynamoDB because X.","promote":true,"raw_evidence":"full evidence","supersedes_memory_id":null}]}
```

Response:

```json
[{"event_id":"evt-1","outcome":"PROMOTED","promoted_memory_id":"mem-dynamo","raw_handle":{"handle":"artifact://demo/hash","evidence_id":"evidence-1"}}]
```

Typed high-value events (decision, finding, constraint, task and state) are promoted deterministically even when `promote` is omitted; the flag remains a compatibility hint. Secret-like events are retained without promotion. Explicit event IDs
are unique; a duplicate returns `409 conflict`.

## `POST /v1/memories`

Creates explicit typed knowledge. Required: `workspace_id`, `kind`, `content`.
Optional: `subject`, `raw_evidence`.

`kind`: `decision | finding | constraint | task | state`.

## `GET /v1/memories?workspace_id=demo&status=current&limit=100`

Returns a JSON array of serialized `Memory` objects.
The current endpoint has no cursor wrapper or pagination cursor; `limit` bounds the returned array. `status` may be `current`, `superseded`,
`stale`, or `conflicted`. Omit it to list all workspace memories.

Each memory contains nested `provenance` and `evidence` objects. Historical
memories remain retrievable.

## `GET /v1/activity?workspace_id=demo&limit=50`

Returns event records and promotion outcomes. Raw content is not included.

## `POST /v1/context`

Request:

```json
{"workspace_id":"demo","query":"Implement persistence","max_tokens":1000}
```

Response shape:

```json
{"request_id":"ctx-1","workspace_id":"demo","query":"Implement persistence","summary":"DynamoDB superseded Redis. Reason: X.","current_state":[{"key":"persistence","value":"DynamoDB superseded Redis. Reason: X.","memory_id":"mem-dynamo"}],"evidence":[{"memory_id":"mem-dynamo","evidence_id":"evidence-dynamo","subject":"persistence","kind":"decision","content":"DynamoDB superseded Redis. Reason: X.","provenance":{"source_agent":"agent-a","event_id":"evt-dynamo","observed_at":"2026-09-18T10:00:00Z"},"reasons":["workspace match","kind priority: 4","1 query term matches","subject match","current truth"],"estimated_tokens":9,"raw_handle":"artifact://demo/hash-dynamo","status":"current"}],"token_usage":{"budget":1000,"selected":9,"tokenizer":"o200k_base","is_proxy":true}}
```

Selection is deterministic lexical matching over current memories in the
requested workspace. Current decisions and state receive deterministic priority; subject matches and selection reasons are explicit. Duplicate packet items are removed, and superseded memories are excluded from current context.
Token usage counts selected evidence content with `o200k_base`; it is a labelled
proxy and does not count full serialized packet overhead.

## `POST /v1/memories/{id}/supersede`

Request:

```json
{"workspace_id":"demo","replacement_memory_id":"mem-dynamo","reason":"DynamoDB is current persistence."}
```

The old memory becomes `superseded`; replacement must be `current`. Both remain
provenance-preserving and workspace-scoped. Repeating the same supersession is
idempotent. Cross-workspace and missing targets fail.

## `GET /v1/evidence/{hash}?workspace_id=demo`

Returns raw evidence bytes for a handle such as
`artifact://demo/<blake3-hash>`. Access requires bearer authentication and the
workspace query parameter.

## Errors

```json
{"error":{"code":"conflict","message":"..."}}
```

`401` unauthorized; `400` malformed/unsupported input; `404` missing memory or
evidence; `409` conflict or workspace violation; `500` local storage failure.

## Compatibility

Unknown response fields may be added. Required fields and enum meanings require
contract and fixture updates before breaking changes.
