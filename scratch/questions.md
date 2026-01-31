# Open questions

- Choose execution is still deterministic (alphabetical option) and ignores the choose prompt. Should we wire choose to agent invocation and parse a structured choice output?
- AgentStepRunner only uses config.area (no flow run area). Do we want to thread FlowRun.area into context assembly?
- LoopUntilEmpty now checks roadmap/<wave> and runs only Step items. Should loops support fork/choose items and propagate interactive steps instead of failing?
- Context assembly parity gaps remain (summaries, loopflow doc embedding, area parent docs, exclude patterns). Which pieces are required for Stage 2 parity?

## Stage 4: lf Client Refactor

- Should `lf ops` become `lf git` for clarity? "ops" is vague; "git" is precise.
- Agent-assisted conflict resolution: should this live in `lf ops rebase` only, or also in daemon's rebase for waves?
- Should we add worktree operations to Rust? `wt` CLI handles them now, but daemon may want direct control.
- gh CLI wrappers: keep in Python forever, or eventually move to Rust for daemon integration?
