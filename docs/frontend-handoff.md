# Frontend handoff

Base URL is local service root. Protected endpoints require `Authorization: Bearer agent-a-token` or `Bearer agent-b-token`. Identity comes from token; do not send `agent_id`.

| Endpoint | Use | Fixture |
|---|---|---|
| `POST /v1/events` | append/promote event | `fixtures/api/event-request.json` |
| `POST /v1/memories` | explicit typed memory | `fixtures/api/memory-request.json` |
| `GET /v1/memories?workspace_id=demo&status=current` | current/history list | `fixtures/api/memories.json` |
| `GET /v1/activity?workspace_id=demo` | event activity | `fixtures/api/activity.json` |
| `POST /v1/context` | bounded task packet | `fixtures/api/context-response.json` (shape fixture; IDs are illustrative) |
| `POST /v1/memories/{id}/supersede` | current decision replacement | `fixtures/api/supersede-request.json` |
| `GET /v1/evidence/{hash}?workspace_id=demo` | recover raw bytes | handle from context packet |

Errors use `{ "error": { "code": "...", "message": "..." } }`. `401` means invalid bearer token; `400` malformed/unsupported input; `404` missing memory/evidence; `409` conflict or isolation violation.


## Primary product surface

Context Inspector is the first user-facing experience. Show Agent B, task,
budget, current project state, selected context, selection reasons, Agent A
provenance, raw handle and selected/budget token usage. Build from fixtures;
backend deployment is not required. See [`../demo/golden-path.md`](demo/golden-path.md).
