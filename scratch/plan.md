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

**Docs** (write-docs, describe the target): `docs/orchestration.md` Loops section — pass sizing guidelines, the prompt line `Decision pass 3. Iterating repeats 3 steps, back to implement.`, outer-review-loop example. `rust/loopflow-execution/src/harness/README.md` Task updates — run starts and passing steps are not news; settled runs carry their error. Code must match these.

## Remaining

### Pass 2: pass-sizing guidance + decider pass count (one pass)

- `rust/loopflow-definitions/src/builtins/task/skill/implement.md:18` and `kickoff.md:68`: replace "end to end, thinly" with thick end-to-end passes; add guidelines (1)–(4) where each skill uses them (implement: 1, 2, 4; kickoff: 3; realign "What is next?": regroup into the remaining passes, aim ≤5 total).
- `loop-or-next.md`: drop stale pre-#1962 `step:` wording; each `iterate` restarts every loop step fresh, so name a substantial next pass.
- `rust/loopflow-execution/src/graph/output.rs:11` `DECISION_CRITERIA`: drop "not from … the number of passes"; iterate names a substantial pass; blocked when passes shrink into small increments that don't converge.
- `rust/loopflow-execution/src/graph/instructions.rs:69`: drop "There is no arbitrary pass limit"; same guidance.
- `rust/loopflow/src/lf/commands/graph.rs:1272`: prompt reads `Decision pass {n}. Iterating repeats {k} steps, back to {target}.` where `k` counts the steps the backward edge repeats (incl. the decider). Keep or trim the evidence-gap sentence; it is the current text, not in the docs.
- Refresh `tests/goldens/builtin_{implement,realign,…}.md` (`cargo test -p loopflow --test golden_prompt`), update any test asserting the old prompt (`grep -rn "Decision pass" rust/`).
- Check: a CLI Graph fixture with a stand-in decider shows the new line on pass 2; `uv run pytest python/tests/test_instruction_levels.py python/tests/test_loopflow_skill_alignment.py`.

### Pass 3: quiet running steps in `task_updates`

- `rust/loopflow-execution/src/harness/task_updates.rs`: for a Task's own conversation, while the Graph runs keep the last settled position (run start and intermediate steps write no steer), as conducted children already do (`"Steps passing inside a run are not news"`). Settled runs (finished, failed, crashed without `ended_at`), interactive/end stops, merges, comments and steers still report once. Add `graph.run.error` to `RunObservation` and its report. A stored mid-run reading stays quiet until the run settles.
- Replace `a_run_between_steps_keeps_its_last_report` and the "child's own conversation hears its steps" assertion with: a run advancing across steps writes zero updates; settling writes one, with the error when failed.
- Check through `server::wake` that an idle Task session is not woken mid-run.

Final checks for gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `documented_commands`.
