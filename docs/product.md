# Product contract

## Problem

Independent agents repeatedly rediscover decisions, failures, architecture and project state because their contexts are isolated. Dumping all shared history into every request creates noise, stale truth and token waste.

## ContextBridge owns

Agent identity; workspace identity; event ingestion; memory lifecycle; typed memories; provenance; current state; decision supersession; staleness/conflict handling; relevance selection; token budgets; context packet construction; cross-agent retrieval; recoverable evidence; observability; API/MCP surface.

## It does not own

The agent loop, source repository truth, generic chat history, a vector database as a product, or a clone of Effimiser. AgentCore Memory can store and retrieve memory; ContextBridge decides what this agent should know now.

## MVP acceptance

Given Agent A emits “migrated persistence from Redis to DynamoDB because X”, Agent B requests persistence context, and a prior Redis decision exists, the response contains DynamoDB as current, Redis as superseded history, reason X, provenance, a raw/evidence handle, and token usage within budget.

