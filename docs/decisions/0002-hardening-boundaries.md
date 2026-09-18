# 0002 Hardening boundaries

Status: accepted for Milestone 1 hardening

Keep the Milestone 1 implementation in one Rust crate and one primary module
for now. The storage trait and evidence trait are the meaningful boundaries;
extracting every type into folders during this pass would increase regression
risk without changing ownership. Split internal modules when the next feature
creates a stable responsibility boundary, not to imitate a directory tree.

Raw evidence has a separate `EvidenceStore` port. The local SQLite adapter
implements both structured and evidence ports today; future DynamoDB and S3
adapters can implement them independently.
