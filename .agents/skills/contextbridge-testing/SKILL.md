---
name: contextbridge-testing
description: Use when adding unit, integration, contract, golden-path, or end-to-end tests.
---

Test the golden path first. Cover workspace isolation, server-derived identity, event promotion, idempotency, supersession, bounded packing and raw-handle recovery. Use local fakes for AWS/model ports. Keep fixtures realistic and versioned. A packet-size assertion never replaces correctness assertions.
