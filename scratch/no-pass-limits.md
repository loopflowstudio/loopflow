# Remove Flow pass limits

Accepted human direction, 2026-09-25: looping forever is not inherently wrong.
Delete the pass budget, rather than resetting it at human revision or adding an
unlimited option. A backward edge needs only its target.

Keep observed pass counts for decision context and saved history; they confer
no stopping authority. Iterate follows the edge, Advance follows the sequence,
and Blocked asks for help when the decision agent judges it necessary. Exact
human approval, failed-provider handling and stale decision rejection remain.

Remove max_iterations from RepeatPolicy, parsing validation, builtins, prompts,
docs and fixtures. Older saved definitions may contain the retired field;
loading them must preserve their target, progress and pending decision while
discarding the obsolete limit. No new setting, reset API or lifecycle.

Proof: iterate beyond the previous builtin limit, accept human revision and
iterate again, then Advance normally. Exercise ordinary and Task adapters,
saved definition recovery, missing decisions and explicit Blocked outcomes.
An actual endless provider run is neither needed nor useful for this proof.

## Integration handoff

The Ghostty review implemented the limit removal in RepeatPolicy, finish_step,
Task decision context, builtin flows, loop-decide guidance, and user docs. The
reducer keeps saturating descriptive counts, with no exhaustion branch.
Existing exhaustion tests now require continued iteration. Added an ordinary
20-iteration proof and a saved-definition test containing a retired limit.
The Task delivery fixture performs ten passes, takes human Iterate, then takes
another autonomous Iterate without resetting anything.

Concurrent shared-cursor and XOR work is owned by the main implementation Run
per cursor-integration.md. Avoid overwriting those edits. The first repository
proof attempt did not execute tests: compilation encountered its in-progress
test migrations (Task cursor usize vs old test helper u32, old path.flow access,
and settled.step_index). Those were repaired during integration. A subsequent
compile caught a duplicate ConcreteSkill test import; the review removed it.

Final integrated proof: **29 passed, 0 failed** using the command below,
including ordinary 20-iteration completion, Task revision after the old limit,
saved legacy-definition recovery, pending decisions, explicit blockers, nested
recovery, and the real Task driver with simulated provider/PM side effects.
All-target Clippy with warnings denied, cargo fmt --all -- --check, and
git diff --check passed. No live provider or UI acceptance demo is claimed.

Focused command (clear LF_CONTROL_HOME, LF_CONTROL_DB_PATH, LF_HOME, LF_DB_PATH,
LF_RUN_CONTEXT, LF_RUN_ID, LF_RUN_DIR, LF_FLOW_STEP and LF_HUMAN_SESSION first):

```sh
cargo test -p loopflow --lib -- \
  engine::transitions::tests \
  engine::execution::tests \
  engine::flow::tests::backward_edges_require_an_earlier_target \
  engine::flow::tests::authored_human_edges_keep_their_identity_and_revision_target \
  controller::task::planning_tests::feature_repeats_the_whole_slice_then_stops_for_delivery \
  controller::task::planning_tests::loop_requires_a_decision_and_discards_it_on_interrupt \
  controller::task::planning_tests::driver_runs_fresh_slice_turns_until_the_flow_finishes \
  ops::flow_session::tests::saved_nested_human_review_requires_exact_approval_and_recovers_once
```
