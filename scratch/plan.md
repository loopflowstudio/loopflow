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

## Remaining

### Pass 3: quiet running steps in `task_updates`

- `rust/loopflow-execution/src/harness/README.md` describes the intended Task updates.
- `rust/loopflow-execution/src/harness/task_updates.rs`: for a Task's own conversation, while the Graph runs keep the last settled position (run start and intermediate steps write no steer), as conducted children already do (`"Steps passing inside a run are not news"`). Settled runs (finished, failed, crashed without `ended_at`), interactive/end stops, merges, comments and steers still report once. Add `graph.run.error` to `RunObservation` and its report. A stored mid-run reading stays quiet until the run settles.
- Replace `a_run_between_steps_keeps_its_last_report` and the "child's own conversation hears its steps" assertion with: a run advancing across steps writes zero updates; settling writes one, with the error when failed.
- Check through `server::wake` that an idle Task session is not woken mid-run.

Final checks for gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `documented_commands`.
