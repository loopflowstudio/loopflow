# LOO-447 — unresolved transport cut (2026-10-10)

The assumption that a named lifeline is sufficient for Claude takeover is false:
`ClaudeHarness` retains `ChildStdin`, `ChildStdout` and its reader in the launching
LfProcess. SIGKILL closes those pipes even if a second attachment holds the
watchdog FIFO. Pending-result and request correlation also live there. No source
inspection establishes that Claude cannot support takeover at all; Jack Heart's
explicit-refusal allowance is not permission to declare it impossible.

The draft plan preserves the full requested outcome. Dependent lifeline/transport
changes need a coherent reconnect design, including where output is drained and
pending requests survive. The independent common-close slice can be verified
without deciding that transport. It is not a shippable completion of LOO-447.

OpenCode has an additional independent counterexample: stop/abort and child drop
bypass the attachment fence. The runtime close repair does not repair those
harness paths; the plan retains them explicitly.
