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

**Stopped Graph status.** After a successful step, Task status reports a blocked decision in the decider's words and a subsequent Graph failure with its recorded diagnostic. A landing handoff or missing Graph exit keeps its previous status. The earlier review's push example does not stop a Graph: `RunningGraph::commit_step` runs `lf commit --step` best-effort. Exhausted structured-output corrections are a post-step error written on the Graph itself.

- `cargo test -p loopflow-execution --lib ops::task_execution::tests::` passed all 3 tests.
- `cargo test -p loopflow --lib task_graph_launch_tests::an_auto_run_blocked_at_verification_waits_there_without_building_again` passed. The CLI printed the dependency in Task status and the second run started at verification.
- `cargo test -p loopflow --test integration documented_commands::` passed all 4 tests. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.

## Remaining

Gate runs branch CI.
