# Review: lfd real CLI executor

## What was implemented

- Added a shared `FlowEngine`/`StepExecutor` execution layer so both `lf` and `lfd` use the same flow semantics for sequencing, xor routing, loops, and fork/and.
- Refactored the CLI flow command onto that shared engine while preserving in-process step execution and journal emission.
- Replaced the daemon's legacy flow interpreter with a process-supervising executor that launches one `lf <step>` child per headless step, launches interactive steps as terminal sessions, persists nested execution cursors, and resumes from the exact nested xor/loop position after waits.
- Added daemon run env injection (`LFD_WAVE_ID`, `LFD_RUN_ID`, `LF_RUN_ID`, `LFD_SESSION_ID`) and journal replay plumbing so daemon-supervised CLI children report progress through the existing event stream.
- Hardened wave-stop cancellation so stopping a waiting interactive run cancels its active terminal session instead of leaving orphaned interactive state behind.
- Hardened branch renames after worktree moves so rename retries tolerate the brief git metadata lag that showed up in the wave-worktree test suite.

## Key choices

- Keep flow traversal pure and shared; vary only the step executor so CLI and daemon can diverge on supervision without diverging on semantics.
- Persist the full nested execution cursor on `WaveRun` rather than trying to reconstruct nested xor/loop position from top-level `step_index`.
- Prefer daemon-hosted tmux sessions for interactive steps, but preserve the wrapped-command fallback when tmux is unavailable.
- Treat journal events as observability only; the daemon advances runs from its own execution cursor rather than deriving control flow from the journal.
- Cancel interactive terminal sessions during wave stop so cancellation semantics match process-backed steps and waiting runs do not leak terminal state.

## How it fits together

`expand_flow()` still produces the concrete flow plan, but now both entrypoints hand that plan to `FlowEngine`. The CLI executor runs steps inline; the daemon executor wraps each step with sync/supervision, stores cursor state on waits, and resumes by re-entering the same plan with the stored cursor. Journal replay and websocket events stay unchanged because the daemon-launched `lf` children now write directly into the daemon-owned run id.

## Risks and bottlenecks

- `or` flow execution is still intentionally unimplemented in the shared engine.
- Interactive terminal hosting still depends on tmux availability for the preferred path; the fallback path remains less integrated.
- `LfObserver` still polls and rereads event files rather than tailing incrementally.
- The local Concerto UI-test bootstrap crash remains outside this branch.

## What's not included

- Shared execution for `or` multi-select flows.
- Push-based journal ingestion or any observer polling rewrite.
- A broader tmux/shell management system beyond the minimal hosted interactive-step path.
- A fix for the local `ConcertoUITests-Runner` bootstrap failure.

## Validation

- `cargo fmt --check` ✅
- `cargo clippy -p loopflow -- -D warnings` ✅
- `cargo test -p loopflow` ✅
