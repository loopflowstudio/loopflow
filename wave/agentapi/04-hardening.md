# 04: Hardening

Production failure modes that cause stuck or lost sessions. Do this after 05 and 06 — runtime convergence and the third adapter will reshape what needs hardening.

## What to address

- **Process crash recovery**: dead harness process → stuck session. Codex reader task sees EOF and closes the sender; bridge task should transition session to `failed` and emit `Error`. Claude's process-per-turn model: only non-zero exits during an active turn are crashes. Emit `ItemCompleted(Failed)` for in-flight tools when process exits abnormally.
- **lfd restart / orphan cleanup**: `SessionRuntime` lives in memory — active sessions become orphans on restart. Events survive in the store. Startup recovery pass: mark orphaned `active`/`starting` sessions as `failed`.
- **Wave integration**: session end triggers wave advancement (continue/commit logic, run state guards).

## Not worth doing separately

These either work well enough today or belong in other phases:

- **SSE broadcast lag**: reconnect with `after_seq` already provides full replay. In-stream backfill is polish.
- **Concurrent clients**: works now. Fix when it breaks under real load.
- **Provider conformance tests**: belongs with 06 (OpenCode adapter validates the abstraction).
- **Claude reader-task-stop race**: `AtomicBool` guard prevents corruption. Event ordering is surprising but not broken.

## Done when

- Dead harness processes detected and session marked `failed`
- lfd restart doesn't leave orphaned active sessions
- Session end advances wave run
