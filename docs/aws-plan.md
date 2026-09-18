# AWS plan

## Meaningful MVP use

- DynamoDB stores workspace-scoped structured state, active decisions, event metadata and supersession links.
- S3 stores immutable raw evidence/artifacts addressed by content hash; packet items carry handles, not large bodies.
- Lambda or a small API service exposes the contract only if deployment time permits; local application services remain provider-neutral.
- Bedrock is optional and narrow: extraction of candidate typed memories or reranking only after deterministic filtering. Its output is untrusted until provenance, schema and workspace checks pass.

## AgentCore boundary

Current AWS documentation describes AgentCore Memory short-term events, long-term records, semantic retrieval, namespaces and self-managed strategies. ContextBridge should not duplicate raw conversation continuity or generic semantic memory. Use AgentCore only as an optional event/long-term-memory substrate or comparison arm; ContextBridge owns project/workspace identity, typed decisions, supersession, current state and bounded packet assembly.

## Cost and safety

Local fakes must exercise all core tests. No AWS call in unit tests. IAM least privilege: workspace data access only, no broad S3 or Bedrock permissions. Set payload, artifact, retries and timeout limits. Record provider/model/version in observability. Do not add OpenSearch, queues, or extra services without a measured acceptance need.

## Cloud cut line

Demo must show one meaningful AWS-backed path: DynamoDB state plus S3 raw evidence, or a documented local fallback if credentials/deployment fail. Bedrock is demo enhancement, not correctness dependency.

