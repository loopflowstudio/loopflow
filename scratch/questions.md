# Open questions

- Choose execution is still deterministic (alphabetical option) and ignores the choose prompt. Should we wire choose to agent invocation and parse a structured choice output?
- AgentStepRunner only uses config.area (no flow run area). Do we want to thread FlowRun.area into context assembly?
- LoopUntilEmpty now checks roadmap/<wave> and runs only Step items. Should loops support fork/choose items and propagate interactive steps instead of failing?
- Context assembly parity gaps remain (summaries, loopflow doc embedding, area parent docs, exclude patterns). Which pieces are required for Stage 2 parity?

## Stage 4: lf Client Refactor

- Decision: keep `lf ops` as the name; no rename to `lf git`.
- Decision: remove `lfops` binary with no backwards compatibility.
- Decision: Rust `lf ops` API is primary; shell out to `gh` from Rust as needed. Python proxy is fallback only.
- Decision: `lf ops` should default to Rust when `internal.rust` is enabled; Rust `lfd` can come later.
- Direction: move the entire `lf` CLI toward Rust over time; decide which non-ops commands are next.
- Decision: include at least one non-ops Rust-backed `lf` surface in this diff (e.g., `lf --version`/`lf info`) gated by `internal.rust`.
- For `lf --version` (Rust-backed), should `lf-engine version` mirror `loopflow.__version__`? Workspace/Cargo version and `VERSION` file are `0.7.1` while Python `__version__` is `0.7.2`.
- Agent-assisted conflict resolution: should this live in `lf ops rebase` only, or also in daemon's rebase for waves?
- Should we add worktree operations to Rust? `wt` CLI handles them now, but daemon may want direct control.
- gh CLI wrappers: keep in Python forever, or eventually move to Rust for daemon integration?
- With the `lfops` console script removed, should Concerto/Swift WorktreeService switch to `lf ops` (or is a shim expected)?
