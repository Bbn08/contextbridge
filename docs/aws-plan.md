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


## Milestone 2 implemented

The current branch includes:

- DynamoDbStorage behind the existing Storage and EvidenceStore ports.
- S3EvidenceStore behind the existing EvidenceStore port.
- local/AWS backend selection through CONTEXTBRIDGE_STORAGE.
- opt-in, ignored AWS contract and golden-path tests.

### Configuration

Local mode is the default:

    CONTEXTBRIDGE_STORAGE=local

AWS mode requires:

    CONTEXTBRIDGE_STORAGE=aws
    CONTEXTBRIDGE_DYNAMODB_TABLE=contextbridge-m2
    CONTEXTBRIDGE_S3_BUCKET=example-bucket
    AWS_REGION=us-east-1

AWS credentials are never read from ContextBridge-specific configuration.
aws-config uses the standard SDK provider chain: environment, shared
profiles/credentials, IAM Identity Center, container credentials, or instance
roles. See the AWS SDK for Rust configuration and credential-provider docs.

### Access model

One DynamoDB table uses pk and sk. Workspace queries use
WORKSPACE#<workspace> as the partition key and entity prefixes:

- MEMORY#<id> — typed memories and current/superseded state.
- ACTIVITY#<received_at>#<event_id> — activity history.
- SUPERSESSION#<old_id> — supersession audit records.
- events use EVENT#<event_id> / EVENT for direct promotion updates.


Reads use strongly consistent DynamoDB queries where the API promises an
immediate view of current state. Queries paginate until LastEvaluatedKey is
absent. Event insertion and activity insertion are one transaction; event
promotion updates both records transactionally. Memory creation and
supersession use conditional writes to reject duplicates and invalid state
transitions.

### Evidence model

The public handle remains `artifact://<workspace>/<blake3-hex>`. S3 keys are
internal and use:

    workspaces/<workspace>/evidence/<blake3-hex>

Workspace and hash components are validated before key construction. Evidence
is written before the DynamoDB memory record. `If-None-Match: *` makes repeated
identical writes idempotent; a precondition failure is treated as success for
the same deterministic object. DynamoDB and S3 are not one transaction, so a
failed later DynamoDB write can leave an orphan S3 object, but no committed
memory points at missing evidence and retrying is safe.

### Testing and limits

Normal `cargo test --workspace` is offline and does not run AWS tests. With a
pre-created, dedicated table and bucket, an operator may explicitly run:

    CONTEXTBRIDGE_RUN_AWS_TESTS=1 CONTEXTBRIDGE_STORAGE=aws \
    CONTEXTBRIDGE_DYNAMODB_TABLE=<table> CONTEXTBRIDGE_S3_BUCKET=<bucket> \
    AWS_REGION=<region> cargo test --features aws-tests \
    --test aws_integration -- --ignored

The tests do not provision or broadly delete resources. They use unique test
workspaces and require credentials from the normal AWS SDK provider chain.
Automatic orphan cleanup, deployment/IAM infrastructure, Bedrock, AgentCore,
MCP, embeddings and semantic retrieval remain deferred.
