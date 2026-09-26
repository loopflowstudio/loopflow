# Shared cursor integration — implementation and remaining proof

The bounded lf contributions for definition capture, Task cursor storage,
protocol docs, architecture docs and fork history have finished. Their changes
are integrated with engine/execution.rs, engine/transitions.rs,
controller/task/mod.rs, ordinary Flow runner/Session and CLI integration.
The human's Ghostty concept-review remains open in this shared checkout.

The Ghostty no-pass-limits change is observed and preserved. RepeatPolicy now
contains only from. Counts remain observations; they never stop traversal.

ExecutionCursor is the shared traversal value. Task FlowPosition replaces its
flat step_index/iteration/progress with cursor. SQL index/iteration remain
projections; the former review_json cell stores the full cursor, with explicit
legacy decode. Task claims/transactions retain their authority. Ordinary Flows
retain their file authority. Cursor.finish handles selected paths, forward and
backward movement, human legacy revision, and parent return in one place.

XOR expansion captures router + every path before starting. Router candidates
use lf flow route PATH and the exact active Run authority. Failed Runs discard
candidates; saved successful routes recover without source reads. No shared
scratch route file. Proof must include nested Task human gates, stale route
writers, source deletion, direction carryover and repeat after human revision.

Wave playhead is not extended. Its deletion boundary is a separate concrete
inventory, preserving cadence/chat/ownership and required Task representations.

## Integrated evidence, 2026-09-25

The finish line for this slice is one navigation implementation for ordinary
Flows and Tasks, with captured XOR definitions and exact route/decision
authority. A passing simulated provider fixture is not live provider acceptance.

- Nextest `504e3b9a-56df-4c5e-a2df-a74848cad443`: 91/92 passed. Full
  definition capture exposed a rendering fixture that referenced nonexistent
  skills. Supplying those fixture sources made its focused rerun pass
  (`be973b03-2277-44df-9543-5e9a4fe72fce`).
- Final compression/recovery selection
  `6afa78c4-85d0-402a-85bc-eb111f407d29`: 22/22 passed. This covers ordinary
  traversal, nested human reviews through both adapters, stale decisions,
  exact route ownership, saved successful outcome recovery and failed outcome
  discard. The Task driver executes a router and two five-skill passes in 11
  fresh Runs after the source definitions have been deleted.
- The final selection also proves an older saved completed child returns to
  its parent without replaying work. Settlement normalizes that return; tests
  no longer assume an intermediate completed child remains active.
- All-target Clippy with warnings denied, formatting, whitespace and
  architecture ownership checks passed. Tests used isolated stores and
  simulated provider/PM effects with ambient Run/Home authority cleared.

Compression removed separate human-navigation logic, transient router skill
wrappers, the shared scratch route file, unused executor hooks and recursive
engine dispatch. Cursor.finish owns traversal; Task transactions and ordinary
Flow persistence retain their existing authority.

## Remaining release and review work

- Upstream integration is complete: `lf rebase --manual` applied cleanly and
  the migration check passes with 52 shipped migrations unchanged. No installed
  binary or production Home was changed.
- Historical unresolved Wave XOR records now use the existing non-executable
  definition/restart disposition. Journal bytes and later messages survive;
  explicit restart captures a new invocation and retains queued Flow intent.
  An unreadable ordinary position leaves other Sessions usable and reports its
  exact file and new-invocation recovery. Source bytes that were never captured
  are not presented as recoverable execution. See saved-xor-recovery.md.
- NestedCursor no longer copies branch steps. The captured parent owns the
  definition; old redundant child copies are ignored only in favor of that
  parent, and missing parents/paths stop traversal. See cursor-dedup.md.
- Ask recovery fixes preserve completion before teardown, serialize reopen
  resets with completion, and remove failed Runs' blocker authority. Focused
  proof passes. The development binary/Home mismatch is also repaired and the
  real CLI reopen regression passes; see ask-protocol-audit.md and
  saved-xor-recovery.md.
- Exercise native provider/Ask handoff and human approval through the real
  configured path. Full CI and hosted UI evidence remain outstanding.
- Wave interpreter deletion has a concrete boundary in
  wave-playhead-removal.md. It is not required to extend obsolete execution
  semantics here. Existing choose-one XOR integration belongs in LOO-295;
  prototype/review-prototype skills and UX exploration are filed as LOO-297.

No push, install or merge is part of this integration checkpoint.
