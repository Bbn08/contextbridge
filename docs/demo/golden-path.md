# Golden-path demo specification

Shared Bhuvan/Prathick demo. The verified local automation is `scripts/demo-local.sh`; AWS-backed execution remains pending M2.5.

1. Agent A works independently.
2. Agent A records `Persistence uses Redis.`
3. Agent B requests persistence context and receives Redis with Agent A provenance.
4. Agent A records `Persistence uses DynamoDB.`
5. DynamoDB supersedes Redis.
6. Agent B requests the same context again.
7. ContextBridge supplies DynamoDB as current truth, keeps Redis historical and preserves both provenance records.
8. Context Inspector explains the packet.

## Context Inspector view

```text
Agent: Agent B
Task: Implement persistence
Budget: 1000 tokens

CURRENT PROJECT STATE
Persistence -> DynamoDB
DynamoDB supersedes Redis.

CONTEXT SENT TO AGENT
Decision: Persistence uses DynamoDB
Source: Agent A
Reasons: CURRENT_STATE, LEXICAL_MATCH
Raw evidence: artifact://project-alpha/<hash>
Token usage: selected / budget
```

The screen must answer: “What did ContextBridge tell this agent, and why?”
It should prioritize evidence, provenance, reasons, current-vs-historical state,
raw recovery and budget over login, settings or decorative analytics.

Later AWS-backed demos must preserve these exact semantics.
## Run locally

```bash
bash scripts/demo-local.sh
```

The script starts an isolated local SQLite service, drives both bearer-token agents, prints both context packets, shows superseded Redis history, and recovers raw evidence.
