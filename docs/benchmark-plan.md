# Benchmark contract

## Arms

- A: cold Agent B; no Agent A knowledge.
- B: naive shared history or broad retrieval.
- C: ContextBridge bounded packet.

## Controls

Same model, reasoning effort, repository commit, task, tools, environment, time limit and agent prompt except shared-context arm. Random seed and run count recorded. Raw inputs and outputs retained.

## Primary success

Task success and required-evidence recall. A token reduction with lower success or required evidence is a failure.

## Metrics

Success; evidence recall/precision; irrelevant and stale evidence rate; workspace leakage; duplicates; fallback searches; tool calls; repeated reads; rediscovery operations; model-visible tokens; latency; raw-handle recovery rate.

## Artifact schema

Each JSONL row includes `run_id`, `arm`, `task_id`, `model`, `repo_revision`, `environment`, `success`, `required_evidence`, `provided_evidence`, `tokens_visible`, `latency_ms`, `tool_calls`, `fallback_searches`, `raw_recovery_checks`, `failure_reason`, and timestamp. No aggregate claim without raw rows.

## Initial scenario

Seed Agent A with Redis-to-DynamoDB decision and evidence. Start Agent B independently with “Implement persistence”. Assert C presents DynamoDB current, excludes Redis from current state, retains supersession history, includes raw handle, and stays within budget.



## Milestone 1 measurement boundary

The local packet reports `o200k_base` with `is_proxy: true`. It counts selected
evidence content, not complete serialized packet overhead. This is suitable for
bounded local selection, not provider-exact billing or savings claims.
