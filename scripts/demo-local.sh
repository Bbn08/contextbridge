#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/contextbridge-demo.XXXXXX")"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]]; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
  rm -rf "${RUN_DIR}"
}
trap cleanup EXIT INT TERM

if curl -fsS "http://127.0.0.1:3000/health" >/dev/null 2>&1; then
  echo "port 3000 is already in use; stop the existing service first" >&2
  exit 1
fi

(
  cd "${RUN_DIR}"
  CONTEXTBRIDGE_STORAGE=local cargo run --quiet --manifest-path "${ROOT}/Cargo.toml" -p contextbridge
) >"${RUN_DIR}/server.log" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 60); do
  if curl -fsS "http://127.0.0.1:3000/health" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
curl -fsS "http://127.0.0.1:3000/health" >/dev/null

api() {
  local token="$1"
  local method="$2"
  local path="$3"
  local body="$4"
  curl -fsS -X "${method}" "http://127.0.0.1:3000${path}" \
    -H "Authorization: Bearer ${token}" \
    -H "Content-Type: application/json" \
    --data "${body}"
}

echo "== Agent A: Redis decision =="
api agent-a-token POST /v1/events '{
  "workspace_id": "project-alpha",
  "events": [{
    "event_id": "demo-redis",
    "kind": "decision",
    "subject": "persistence",
    "content": "Persistence uses Redis.",
    "raw_evidence": "Agent A evidence: Redis was the original persistence choice."
  }]
}' | jq .

echo "== Agent B: first context =="
api agent-b-token POST /v1/context '{
  "workspace_id": "project-alpha",
  "query": "Implement persistence",
  "max_tokens": 100
}' | jq .

echo "== Agent A: DynamoDB decision supersedes Redis =="
api agent-a-token POST /v1/events '{
  "workspace_id": "project-alpha",
  "events": [{
    "event_id": "demo-dynamo",
    "kind": "decision",
    "subject": "persistence",
    "content": "Persistence uses DynamoDB.",
    "raw_evidence": "Agent A evidence: DynamoDB replaced Redis."
  }]
}' | jq .

echo "== Agent B: current context =="
api agent-b-token POST /v1/context '{
  "workspace_id": "project-alpha",
  "query": "Implement persistence",
  "max_tokens": 100
}' | jq .

echo "== Historical superseded memory =="
api agent-b-token GET '/v1/memories?workspace_id=project-alpha&status=superseded' '{}' | jq .

echo "== Raw evidence recovery =="
handle="$(api agent-b-token POST /v1/context '{"workspace_id":"project-alpha","query":"Implement persistence","max_tokens":100}' | jq -r '.evidence[0].raw_handle')"
hash="${handle#artifact://project-alpha/}"
api agent-b-token GET "/v1/evidence/${hash}?workspace_id=project-alpha" '{}'
echo
