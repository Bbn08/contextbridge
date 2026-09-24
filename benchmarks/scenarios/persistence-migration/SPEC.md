# Persistence migration scenario

Status: implemented by `engineering_benchmark`; raw results remain local artifacts.

## Setup

Use one repository revision and one isolated workspace named `project-alpha`.
Use identical agent/model/tool configuration across arms.

## Agent A knowledge

1. Agent A records: `Persistence uses Redis.`
2. Agent A later records: `Persistence uses DynamoDB.`
3. DynamoDB supersedes Redis.
4. Both provenance records and raw evidence remain available.

Create `project-beta` with `Persistence uses PostgreSQL.` to test isolation.

## Agent B task

`Implement persistence.`

## Arms

- **COLD:** no Agent A knowledge; Agent B must rediscover persistence.
- **NAIVE:** broad shared history containing Redis and DynamoDB, without current-state filtering.
- **CONTEXTBRIDGE:** bounded packet containing current DynamoDB decision, Agent A provenance, reasons and raw handle.

## Required evidence

- Current persistence choice is DynamoDB.
- DynamoDB superseded Redis.
- Agent A is source.
- Raw evidence handle resolves.
- PostgreSQL from `project-beta` is absent.

## Success criteria

Task succeeds; required evidence is present; current packet excludes Redis as
current truth; no workspace leakage; packet obeys requested budget; raw handle
recovers original evidence.

## Failure criteria

Task failure; missing required evidence; Redis presented as current; stale or
cross-workspace evidence included; duplicate evidence; unrecoverable raw handle;
or token reduction measured without task correctness.

## Metrics

Record arm, task success, required/provided evidence, stale and irrelevant
items, leakage, duplicates, fallback searches, tool calls, repeated reads,
rediscovery operations, visible tokens, latency and raw recovery checks.
