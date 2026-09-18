# ContextBridge benchmarks

Owner: Prathick. Bhuvan owns the backend observability and contract surfaces
needed to measure the system.

This directory contains benchmark specifications only. No benchmark has been
run in Milestone 1. Do not add performance claims to product or README docs
without reproducible raw results.

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
