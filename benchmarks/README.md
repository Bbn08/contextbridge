# ContextBridge benchmarks

Owner: Prathick. Bhuvan owns the backend observability and contract surfaces
needed to measure the system.

This directory contains benchmark specifications and a deterministic engineering
runner. Raw JSONL output is required for any interpretation; no performance
claim is implied by a run.

## Experimental arms

- **A — COLD:** Agent B receives no shared ContextBridge knowledge.
- **B — NAIVE SHARED CONTEXT:** Agent B receives broad or unoptimized shared history, including possible stale information.
- **C — CONTEXTBRIDGE:** Agent B receives bounded, current, task-specific context with provenance.

## Primary rule

Task success and required-evidence recall outrank token reduction. A smaller
packet that causes failure is a regression.

## Controlled variables

Hold model, reasoning effort, repository revision, task, environment, tools,
agent configuration, time limit and seed constant where practical. Retain raw
inputs and outputs.

## Metrics

Task success, required-evidence recall, context precision, irrelevant/stale
rate, project leakage, duplicate evidence, fallback searches, tool calls,
repeated reads, rediscovery operations, model-visible tokens, latency and raw
evidence recovery.

See [`scenarios/persistence-migration/SPEC.md`](scenarios/persistence-migration/SPEC.md).
## Harness

The runner prepares one controlled input fixture per arm and captures command output in a structured result. It does not invoke a model or claim results. Example:

```bash
benchmarks/runners/run-arm.sh --arm contextbridge --output /tmp/contextbridge-result.json -- ./your-agent-command
```

The command receives CONTEXTBRIDGE_BENCHMARK_ARM and CONTEXTBRIDGE_BENCHMARK_INPUT. The result keeps metric fields null until the evaluator records measured values. Use the same command, model, repository revision, environment and task for all three arms.

## Engineering baseline runner

Run real local product path across five scenarios and dataset sizes 10, 100
and 1,000:

```bash
cargo run -p contextbridge --bin engineering_benchmark -- --output benchmarks/results/engineering-baseline.jsonl
```

Runner uses existing in-process HTTP API and local SQLite. It emits one JSONL
row per scenario, size and arm (45 rows by default), retaining selected
evidence and raw handles. `benchmarks/results/` is ignored so raw artifacts
are not silently committed.

Scenarios: supersession, relevance, workspace isolation, duplication and token
pressure.

Formulas:

- required-evidence recall = required selected / required available
- context precision = relevant selected / total selected
- stale evidence rate = stale selected / total selected
- duplicate evidence count = repeated normalized identities after first occurrence
- workspace leakage = selected items whose workspace differs from target

`task_success` requires complete required evidence, zero stale evidence,
zero leakage, zero duplicates, raw recovery and model-visible tokens within the
ContextBridge budget. Cold has no shared context; naive receives broad API
history; ContextBridge uses the real bounded context endpoint. No LLM judge runs.

Current memory retrieval caps list results at 200 records. The 1,000-memory
run measures this current bounded-storage behavior and may expose retrieval
weaknesses; the benchmark does not work around the limit.
