# Open questions — W2-241

## Directive v2 interpretation

The replacement directive says "OpenCode is stalled before first output at
prompt preparation. Preserve all Task state; the portfolio driver is handing
active dependency fronts to Claude and will resume children after their parent
lands."

I acknowledged the directive and proceeded with the kickoff. The stall appears
resolved (this process is running). W2-151 (the predecessor) has merged. If
there is a different parent dependency that hasn't landed, the gate can return
the task to iteration.

## `lf roadmap` "list all waves" default

Roadmap is the one command where global scope (no wave resolved) is a valid
default — it shows every wave's tasks. The fix routes through the resolver so
a stale UUID errors, but `NoContext` still falls through to "list all waves."
This is the only command where `NoContext` is non-fatal AND the command
succeeds. The matrix should classify this as `Resolved(all)` or have a special
`GlobalDefault` classification for roadmap's absent-context cell.

**Assumption:** Roadmap gets a `GlobalDefault` classification for the
`absent` environment only. Every other environment uses the shared
classifications. This is not "blessing divergence" — it's recognizing that
roadmap's design intent is global when unscoped, which is different from
silently dropping a stale UUID.

## `lf radio sub` in the matrix

`lf radio sub` subscribes to the bus and blocks (polls). It's a long-running
command. The matrix needs to run it with a timeout and verify the channel
resolution from the initial output, not wait for it to complete.

**Assumption:** Test `lf radio sub` with a short timeout, verify the channel
name in the first frame or error, then kill the process.

## PM webhook commands need a secret

`lf pm webhook serve/register` require `LF_LINEAR_WEBHOOK_SECRET`. In the
matrix, these will fail at the secret check before reaching wave resolution.

**Assumption:** Set `LF_LINEAR_WEBHOOK_SECRET=test` in the test env. The
command then resolves the wave and fails at the Linear API. The wave name in
the error proves resolution.
