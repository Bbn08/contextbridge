#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --arm cold|naive|contextbridge --output FILE -- COMMAND [ARGS...]" >&2
  exit 2
}

arm=""
output=""
while (($#)); do
  case "$1" in
    --arm) arm="${2:-}"; shift 2 ;;
    --output) output="${2:-}"; shift 2 ;;
    --) shift; break ;;
    *) usage ;;
  esac
done
[[ -n "$arm" && -n "$output" && $# -gt 0 ]] || usage

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
case "$arm" in
  cold) input="${root}/benchmarks/fixtures/persistence-cold.json" ;;
  naive) input="${root}/benchmarks/fixtures/persistence-naive.json" ;;
  contextbridge) input="${root}/benchmarks/fixtures/persistence-contextbridge.json" ;;
  *) usage ;;
esac

mkdir -p "$(dirname "$output")"
stdout_file="$(mktemp)"
stderr_file="$(mktemp)"
cleanup() { rm -f "$stdout_file" "$stderr_file"; }
trap cleanup EXIT

started_ms="$(date +%s%3N)"
set +e
CONTEXTBRIDGE_BENCHMARK_ARM="$arm" CONTEXTBRIDGE_BENCHMARK_INPUT="$input" "$@" >"$stdout_file" 2>"$stderr_file"
exit_code=$?
set -e
finished_ms="$(date +%s%3N)"

jq -n   --arg arm "$arm"   --arg task "$(jq -r '.task' "$input")"   --arg input "$input"   --arg status "$([[ "$exit_code" -eq 0 ]] && echo completed || echo failed)"   --arg stdout "$(<"$stdout_file")"   --arg stderr "$(<"$stderr_file")"   --argjson exit_code "$exit_code"   --argjson latency_ms "$((finished_ms - started_ms))"   '{
    arm: $arm,
    task: $task,
    input_fixture: $input,
    status: $status,
    exit_code: $exit_code,
    latency_ms: $latency_ms,
    stdout: $stdout,
    stderr: $stderr,
    metrics: {
      task_success: null,
      required_evidence_recall: null,
      context_precision: null,
      stale_evidence_rate: null,
      project_leakage: null,
      duplicate_evidence_rate: null,
      fallback_searches: null,
      tool_calls: null,
      repeated_file_reads: null,
      rediscovery_operations: null,
      model_visible_tokens: null,
      raw_evidence_recoverability: null
    }
  }' >"$output"

echo "$output"
