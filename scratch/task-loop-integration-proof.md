# Task transition integration

Bounded contribution, 2026-09-25. Owns the shared transition module, Task
controller/store adapter, pursue definition and concept-review skill. No edits
outside the assigned paths. `durable.rs` already had the agreed FlowProgress
field and pending-decision query, so it needed no change.

## Result and review

- Navigation is Advance/Iterate. Serde reads historical complete/continue and
  interim next/repeat, while new writes and Clap values use advance/iterate.
  Blocked is no longer a navigation variant.
- Implicit forward progress retains direction throughout the pass; explicit
  Advance consumes it. Per-edge counts survive forward movement. Missing or
  empty decisions and exhausted limits still block without changing progress.
- The existing SQLite reader translates both historical LoopReview and interim
  FlowProgress blocked verdicts to a recoverable TaskFlowBlocker. It preserves
  the reason, timestamp, counters, direction, pinned invocation and cursor;
  it does not manufacture a successful decision. An existing failure remains
  authoritative and retains its restart requirement; distinct old blocker
  evidence is appended to its reason.
- Review found that read translation alone would resurrect an old blocked
  verdict after retry clears failure_json. Claim/reclaim now persist normalized
  progress in their existing fenced transaction. Retry starts at the same node
  without a pending verdict. No schema/table or competing execution owner added.
- Pursue runs implement → compress → review-slice → concept-review → loop-decide.
  The dedicated deciding occurrence owns the backward edge, followed by human
  demo. Concept-review returns review evidence only. Already saved definitions
  are not rewritten or re-expanded by this change.
- Task seeds provide prior direction, the current edge's pass and limit, target,
  missing-history guidance, Advance/Iterate protocol and the separate
  `lf flow blocked` Ask protocol. Main owns implementing that command and Ask.
- Existing Run/claim, invocation, generation, cursor and exact human decision
  checks remain. Rejected unrelated, conflicting and stale verdict tests now
  compare the entire persisted position before and after rejection.

## Focused proof plan and validation

The real Task driver fixture now expects two five-step passes, ten distinct
provider Run IDs, direction on every step of the second pass, and saved Advance
recovery without another provider. Provider and Linear transport are simulated;
the fixture truncates the Flow before human demo. It also retains the late-error
replacement-invocation check. The separate feature transition fixture reaches
the human delivery boundary after two passes.

Added SQLite tests read all four old navigation spellings in both stored shapes,
preserving progress and definition. Another reads legacy blockers repeatedly,
then retries at the same cursor without a decision. Reducer tests cover direction,
two independent edges, overlapping budgets, missing/empty decisions, invalid
definitions, exhaustion/overflow and pending-result serialization.

Formatting and owned-file `git diff --check` passed. No Clippy or full suite run.

Focused command (environment authority cleared):

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  cargo test -p loopflow --lib -- \
  engine::transitions::tests \
  controller::task::planning_tests::feature_repeats_the_whole_slice_then_stops_for_delivery \
  controller::task::planning_tests::loop_never_treats_missing_or_exhausted_decisions_as_done \
  controller::task::planning_tests::driver_runs_fresh_slice_turns_until_the_flow_finishes \
  store::sqlite::durable::durable_store_tests::legacy_flow_decisions_preserve_pinned_progress \
  store::sqlite::durable::durable_store_tests::legacy_blocked_verdict_stops_and_retries_without_advancing \
  store::sqlite::durable::durable_store_tests::loop_verdict_survives_recovery_and_rejects_unrelated_runs \
  controller::task::planning_tests::stale_human_decisions_cannot_target_a_replacement_invocation \
  controller::task::planning_tests::concurrent_human_decisions_settle_once
```

Initial compile stopped before tests on two errors in unowned
`lf/commands/flow.rs`: E0063 at line 696 (`ExecutionContext.direction` missing)
and E0004 at line 97 (`FlowOutcome::Blocked` unhandled). No unowned file was
changed to bypass them. The first attempted command also used incorrect
`tests` module qualifiers for controller/store tests; the command above corrects
them to `planning_tests` and `durable_store_tests`. Its result is recorded below.

The corrected command also exited 101 before executing tests. The two CLI
errors remained; concurrent edits in unowned `ops/human_session.rs` added six
more compiler errors: unresolved `record` (164), `launch_lock` (165),
`tests::ASK_LAUNCHERS` (1373, 1396), `tests::FAILED_ASK_LAUNCHERS` (1393), and a
missing `AskSessionRecord.retain_completed` initializer field (1749). These line
numbers describe the sampled shared checkout. **No behavioral test pass is
claimed.** Rerun the focused command after main finishes those adapters.

## Limits and integration remaining

This contribution does not prove generic/Task execution parity, live Ask
deduplication/recovery, real provider death, Ghostty/app handoff, or deployment.
No live provider, Ask, Task worker, installation, commit, push or PM mutation was
performed. Main must implement the advertised CLI/Ask protocol and reconcile
older pinned prompt instructions with command compatibility. The historical
navigation decoding here preserves stored decisions, not old CLI command names.

## Main integration evidence

The 2026-09-25 integrated run passed all 25 selected transition/engine/Task/store
tests, including the two five-step Task passes and old decision-state migration.
Later main changes authored backward edges on design/demo, unified the human
navigation enum, and updated the fixture to require explicit design approval and
prove delivery Iterate returns to implement. That revised fixture passed in the
next focused run. Its old source-format and autonomous-only test assumptions
were corrected to the accepted authored-edge behavior. See protocol-review.md
for the remaining full-design gaps; these passes do not prove Wave parity.
