---
name: contextbridge-aws
description: Use when adding or reviewing DynamoDB, S3, Bedrock, AgentCore, Lambda, API Gateway, IAM, or deployment work.
---

Read `docs/aws-plan.md`. Keep AWS behind ports and local fakes. Add a service only for a measured product responsibility. Use least privilege, bounded payloads, explicit timeouts and provider metadata. Bedrock output is untrusted until schema, provenance and workspace checks pass. Never make core correctness depend on cloud credentials.
