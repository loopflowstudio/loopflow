# LOO-1082: Avoid deep Graph loops and cold-cache single-slice passes

## Direction (2026-11-23)

Jack Heart, exact words:
- "i dont like the cap on loops exactly. the problem here was that the iterations were too small, not that it kept going whent there was good work to do. we can tell it to aim for no more than 5 loops" — no `max_loop_passes`, no cap in `finish_step`, no held stop or `Blocked` plumbing for it. Guidance only; the decision prompt shows the pass number and steps repeated; the decider may iterate past 5.
- "we should not say thinly. we should say end to end in something more like thickly" — remove "Build end to end, thinly" everywhere.
- "i thikn you can do all 4 as soft guidelines" — soft guidance in builtin skills, no runtime checks: (1) keep building while the next item is the same kind of work; end a pass when the next item needs a decision a review or the plan could change; (2) a pass is about as much as one review can judge, one diff a person reads in a sitting; (3) kickoff splits into 2–4 passes (aim ≤5), realign regroups the rest into the remaining passes; (4) ~2k lines of non-test change is a point to pause, a ceiling not a target.
- On the `lf <step>:` prefix: "i generally like it" — the rule stays.
- "i dont tink commit messages are a problem really since they are already routed to cheap agents" — no scratch-only commit-message skip; `commit_step` and `commit-message` stay.
- "-p should be default" and "change it to you have to pass someting to not push. just put it in this one."

Field case behind this: five `manabot` `coverage-working` Tasks (MTG-418..422) ran 9–17 six-skill passes each, often one card per pass (629 sessions; 399 `task-session` wakeup turns, 72.7M cached input). Out of scope: sharing a provider conversation across skills (LOO-435); editing other repos' Graphs.

## Done

**Manual `lf commit` pushes by default** (commits 6701b8a0df, 44d1fc2eef). Branch checkpoints push, selected paths included; default branch and detached HEAD stay local; `--no-push` opts out; the served `push` boolean carries it through Desktop. Checked: in a disposable repo, commits on the advertised `trunk` default and on detached HEAD stayed local; `lf help commit` showed `--no-push`; old flags failed to parse; `lf server describe --json` matched the schema fixture byte for byte. `cargo test -p loopflow --test integration commit_tests::manual_commit_pushes_branch_checkpoints_unless_kept_local -- --exact` passed; with an unavailable remote the commit stayed and the same SHA pushed on retry.

**Pass sizing.** Kickoff groups larger changes into 2–4 passes. Implement batches related work into one reviewable diff, and realign regroups the rest. Graph decisions name a substantial next pass and can continue past five. The prompt takes its count from the current loop's first step through its decider, counting both.

- `cargo test -p loopflow --test session_lifecycle_tests public_taskless_graph_records_distinct_completed_loop_passes` passed. Six skill launches across three passes. The provider received `Decision pass 2. Iterating repeats 2 steps, back to work-proof.` on the second decision.
- `cargo test -p loopflow --test golden_prompt` passed after refreshing the implement and realign snapshots.
- `uv run pytest python/tests/test_instruction_levels.py python/tests/test_loopflow_skill_alignment.py` passed all 23 tests.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed for this slice.

**Task conversation wakeups.** Taking up a Graph and passing through steps write no steer. The Task keeps its last settled position while the Graph runs. Its next run update gives the outcome, the last step, and any recorded run error. The update reads all three from one snapshot. A process that died between steps without recording its exit reports an unknown outcome.

- `cargo nextest run -p loopflow-execution -p loopflow --lib -E 'test(harness::task_updates::tests::) | test(server::wake::tests::an_idle_task_conversation_wakes_only_when_its_graph_settles)'` passed all 16 tests. The idle inbox received zero connections during the run and one message carrying `lint failed` after settlement. A reading saved mid-run by the previous format delivered a new comment and then the settled run.

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.

**Compression checks.** The focused nextest run passed all 39 tests, including `documented_commands`, Task wakeups, Graph loop and router prompts, manual commits and merge revocation before push. A second run of `manual_commit_pushes_branch_checkpoints_unless_kept_local` passed with a staged edit reverted in the working tree. Restaging it published the existing checkpoint at the same SHA.

**QA (2026-11-23).** Real runs on a private Machine with Claude Haiku: manual commits, a three-pass loop Graph, and a Task's idle conversation hearing a command Graph settle (`scratch/qa.html`). Two fixes: a failed push says the commit stays local and prints git's error once (`OpsError::PushFailed`, served `unavailable`); the Task position waits for every run's outcome, so the arrival at `end` no longer wakes the conversation with an empty update a second before the real one.

## Remaining

**Surface blocked loop decisions and post-step Graph failures in `lf task status` (`project_execution`).** Post-build concept review (`scratch/concept-review.html`) found that `task_updates` (updated in this branch) and `ops::task_execution::project_execution()` disagree when the latest step exited `0` (`ExecutionResult::Succeeded`) but `graph.run` failed:

1. A loop decider returning `blocked` (including the new non-convergence rule in `DECISION_CRITERIA`) exits `0` while `graph.run` exits `3` (`GraphHeld::EXIT`) with `graph.run.error = Some("Graph is blocked: <reason>")`.
2. A post-step `lf commit --step` failing to push (`OpsError::PushFailed`) runs after the skill step exited `0` while `graph.run` exits `1` with `graph.run.error`.

In both cases, `task_updates` wakes `task-session` with `graph.run.error` and tells it to read `lf task status`, but `project_execution()` (`rust/loopflow-execution/src/ops/task_execution.rs:348-365`) checks only `latest.run.result()` (`Succeeded`), reports `TaskExecutionState::Idle` (`"Graph stopped after <step>..."`), drops `graph.run.error`, and causes `task_actions` / `lf wave status` to recommend `lf task run` instead of showing `Blocked`.

**Fix (one pass):**
- In `rust/loopflow-execution/src/ops/task_execution.rs` (`project_execution`, `(Dead, Dead)` branch when `process.result() == ExecutionResult::Succeeded`): inspect `graph.run` when `graph.run.result() == ExecutionResult::Failed`:
  - If `graph.run.exit_code == Some(i32::from(crate::process::GraphHeld::EXIT))`: when `graph.run.error` starts with `"Graph is blocked:"`, return `(TaskExecutionState::Blocked, error.trim_end_matches('.').to_string())`; otherwise keep `TaskExecutionState::Idle` so `GraphOutcome::Waiting` (`lf pr land`) still turns into `Waiting` via `snapshot.rs`.
  - Otherwise (non-held `graph.run` failure after the latest step succeeded, e.g. `lf commit --step` `PushFailed`): return `(TaskExecutionState::Blocked, format!("{label}: {ended}. Inspect its history and effects, then launch fresh work"))` using `graph.run.error` / `graph.run.exit_code`.
  - Keep `TaskExecutionState::Idle` when `graph.run.result() != ExecutionResult::Failed` (e.g. Graph killed with no exit record, preserving `a_graph_runs_while_its_processes_do_and_blocks_on_a_failed_step`).
- Add focused unit tests in `rust/loopflow-execution/src/ops/task_execution.rs` covering: `GraphOutcome::Blocked` (`exit_code = 3`, `"Graph is blocked: ..."` -> `Blocked` with reason), `GraphOutcome::Waiting` (`exit_code = 3`, `"Graph stopped before its last step..."` -> `Idle`), and post-step `graph.run` failure (`exit_code = 1`, `PushFailed` error -> `Blocked` with reason).
- Verify with `cargo test -p loopflow-execution --lib ops::task_execution::tests::` and `cargo test -p loopflow --lib task_graph_launch_tests::an_auto_run_blocked_at_verification_waits_there_without_building_again`, plus `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.

Then gate runs branch CI.
