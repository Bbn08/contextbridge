# 0001 Initial boundaries

Status: accepted for review

ContextBridge starts as a contract-first Rust workspace with no empty crates. Core services use ports for structured state, raw evidence and optional model providers. Local adapters prove behavior before AWS deployment. Reference repositories inform decisions but contribute no source code.

Reason: second-brain has useful working boundaries but its V2.4 context path is still in progress; Effimiser has excellent built foundations but its compiler/index/MCP roadmap is largely planned. Starting with contracts avoids importing unfinished assumptions.

