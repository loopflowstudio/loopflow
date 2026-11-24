# LOO-1082: Avoid deep Graph loops and cold-cache single-slice passes

## Direction after concept-review (2026-11-23, supersedes conflicting sections below)

Jack Heart, exact words:
- "i dont like the cap on loops exactly. the problem here was that the iterations were too small, not that it kept going whent there was good work to do. we can tell it to aim for no more than 5 loops" — **drop `max_loop_passes`, the cap in `finish_step`, verdict-preserving held stops, the `Blocked` status plumbing for it, and their tests/docs.** Guidance only; the decision prompt shows the pass number and steps repeated; the decider may iterate past 5.
- "we should not say thinly. we should say end to end in something more like thickly" — remove "Build end to end, thinly" everywhere, Slice 1 included.
- "i thikn you can do all 4 as soft guidelines" — soft guidance in builtin skills, no runtime checks: (1) keep building while the next item is the same kind of work; end a pass when the next item needs a decision a review or the plan could change; (2) a pass is about as much as one review can judge, one diff a person reads in a sitting; (3) kickoff splits into 2–4 passes (aim ≤5), realign regroups the rest into the remaining passes; (4) ~2k lines of non-test change is a point to pause, a ceiling not a target.
- On the `lf <step>:` prefix: "i generally like it" — the rule stays as it is.
- "i dont tink commit messages are a problem really since they are already routed to cheap agents" — **drop the scratch-only commit-message skip**; `commit_step` and `commit-message` stay as they are.

Still in scope: quiet `task-session` during running steps (settled-run `error` in the update stays useful for failed runs), pass-sizing skill text, pass number in the decision prompt.

### Implementation order after the manual commit steer

Jack Heart: "-p should be default" and "change it to you have to pass someting to not push. just put it in this one."

1. Built and tested: manual `lf commit` pushes branch checkpoints, including selected paths. The default branch and detached HEAD keep the commit local. `--no-push` opts out. The existing served `push` boolean carries that choice through Desktop and the CLI.
2. Build the substantial-pass guidance and the decider's pass count, repeated span and return target together. Check the prompt through a running loop and refresh the builtin goldens.
3. Suppress healthy running-step updates to a Task's own conversation and include the error in the settled update. Check the wakeup through the Task session.

In a disposable repository, the branch CLI kept both a commit on the advertised `trunk` default and a detached-HEAD commit local. `lf help commit` showed `--no-push`, both old flags exited with a parser error, and `lf server describe --json` matched the schema fixture byte for byte.

`cargo test -p loopflow --test integration commit_tests::manual_commit_pushes_branch_checkpoints_unless_kept_local -- --exact` passed. A separate CLI run with an unavailable remote retained the commit and pushed the same SHA on retry.

### Docs written (write-docs)

- `docs/orchestration.md` — Loops section (pass sizing as guidelines, decision-prompt line `Decision pass 3. Iterating repeats 3 steps, back to implement.`, outer-review-loop example).
- `rust/loopflow-execution/src/harness/README.md` — Task updates: run starts and passing steps are not news; settled runs carry their error.
- `docs/config.md` — no change: no new setting.

The decision-prompt wording in the docs is the target; `graph.rs` and its tests should match it.


## User-visible outcome

Tasks running looped Graphs complete in **2–4 substantial passes** instead of 10–17 single-item micro-passes, without burning cold-cache tokens on loop overhead, scratch-only commit-message sessions, or per-step `task-session` wakeups — and when an autonomous loop does hit its pass cap, `lf task status`, `task-session`, and `lf task run` explain why and recover in one command with zero wasted decider sessions:

1. **Cohesive passes by default:** `kickoff`, `implement`, `realign`, `loop-or-next`, `DECISION_CRITERIA`, and `instructions.rs` size passes to match the cold-cache cost of a loop iteration—using Slice 1 to de-risk the shape end to end, then batching cohesive/straightforward items inside warm-cache `implement` sessions.
2. **Visible, bounded autonomous loops with truthful status and clean recovery (building on LOO-883):**
   - Every loop decision prompt states the pass count, pass limit, and number of steps repeated per pass. From pass 4 onward (or the penultimate pass when `max < 5`), the decider receives an explicit deep-loop warning; on the final allowed pass, it is told that returning `iterate` will stop the Graph here.
   - `max_loop_passes` (default `6`; `0` or `null` = unlimited) is a typed `Launch` config setting configurable globally, per repo, per Wave, per Skill, per Graph step occurrence, or via `--setting` / `LF_SETTINGS`. Live human interactive steps (`skill.interactive && !standing_in`) are never capped.
   - `transitions::finish_step` owns loop-cap enforcement. When an autonomous decider returns `iterate` on pass `>= max`, its `GraphVerdict` stays preserved on `ExecutionCursor.progress.verdict` and the Graph stops cleanly with `GraphOutcome::Blocked` (`GraphHeld`, exit code `3`, zero automatic retries) naming the deciding step, pass/max, last direction, and exact recovery commands (`--from <next_step>`, `--from <decider>`, or raising `max_loop_passes`).
   - Raising `max_loop_passes` and rerunning `lf task run` settles the saved `iterate` verdict immediately (zero re-run of the decider, saved direction preserved) and jumps straight into the loop body. Running bare `lf task run` without raising the cap stops immediately in <10ms with zero LLM calls.
   - `lf task status` (`ops::task_execution::project_execution`) reports `TaskExecutionState::Blocked` with `graph.run.error` whenever a Graph blocks after a succeeded step (fixing the latent bug where `Blocked` Graphs displayed as `Idle` with a generic message), and `task_updates` includes `graph.run.error` in the single settled wakeup to `task-session`.
   - If an auto-standing-in `interactive:` decider (e.g. `ship-review`) hits the cap, `execute()` hands the Task back to the person at `At { step }` via `arrive_task_graph`.
3. **Zero LLM sessions for scratch-only step commits:** Automatic `lf commit --step <skill>` still commits and pushes `scratch/` checkpoints, but writes a deterministic title (`lf <step>: update scratch/...`) instead of spawning a cold-cache `commit-message` agent when every staged path (`git diff --cached --name-only --no-renames -z`) is under `scratch/`.
4. **Quiet `task-session` during healthy running steps:** Idle Task conversations wake on settled Graph runs (finished, failed, blocked, crashed), interactive/end stops, PR merges, closes/reopens, comments, and steers—not on run starts or intermediate step ticks inside a live Graph. Desktop and `lf task status` continue reading live step progress directly from SQLite.

**Done when:**
- A multi-item Task running a looped Graph (`build` or a custom loop like `coverage-working`) gets pass-sizing guidance, reports pass count + repeated-step span to its decider, warns at pass 4+, and stops cleanly once at `max_loop_passes` (default `6`) without auto-retrying or losing committed work.
- `lf task status` reports `Blocked` with the pass-limit reason and recovery commands; bare `lf task run` stops immediately with zero LLM calls; raising `max_loop_passes` (via `--setting`, Wave/repo/global config, or `0`/`null`) continues straight into the loop body with the saved direction without re-running the decider; `--from <next_step>` advances past the loop; live human interactive deciders are never capped.
- A skill step that edits only `scratch/` commits and pushes without launching a `commit-message` session; a step that edits or renames code outside `scratch/` still invokes `commit-message`.
- A Task's interactive `task-session` receives zero progress wakeups while a Graph starts or advances through healthy running steps, and receives one update (carrying `graph.run.error` when blocked/failed) when the run settles, blocks, fails, or stops at an interactive step.
- Builtin prompt goldens (`tests/goldens/`), `docs/orchestration.md`, `docs/config.md`, and `rust/loopflow-execution/src/harness/README.md` match the new behavior.

## Root causes (verified in code against the 2026-11-23 `manabot` field data)

Across five `manabot` `coverage-working` Tasks (`MTG-418..422`, 9–17 loops each = 629 sessions / 1,024 turns / 17.75M uncached input / 460.7M cached input / 1.27M output), five code-level mechanisms compounded:

1. **Micro-slice prompting + pass-count blindness:**
   - [implement.md](rust/loopflow-definitions/src/builtins/task/skill/implement.md) instructs "Build end to end, thinly" with no guidance to batch cohesive items after Slice 1, discarding a 50:1 warm prompt cache inside `implement` (237.7M cached vs 4.74M uncached) to cold-start 5–6 downstream sessions.
   - [realign.md](rust/loopflow-definitions/src/builtins/task/skill/realign.md) asks for "the next slice" singular, causing agents to queue one card per loop (e.g., *Diregraf Ghoul*, a 1-line vanilla 2/2).
   - [loop-or-next.md](rust/loopflow-definitions/src/builtins/task/skill/loop-or-next.md#L8-L9) still references removed `step:` syntax from before PR #1962 and mentions no pass cost.
   - [DECISION_CRITERIA](rust/loopflow-execution/src/graph/output.rs#L11) explicitly says `"judge from results, not from confidence or the number of passes"`, [instructions.rs](rust/loopflow-execution/src/graph/instructions.rs#L69) and [orchestration.md](docs/orchestration.md#L146) say `"There is no arbitrary pass limit"`, and [RunningGraph::run_skill](rust/loopflow/src/lf/commands/graph.rs#L1264-L1273) sends the same neutral `"Decision pass N"` text on pass 17 as on pass 1 without mentioning step span or depth.
2. **Unbounded autonomous loops:**
   - [transitions.rs](rust/loopflow-execution/src/graph/transitions.rs#L84-L98) accepts `GraphDecision::Iterate` up to `u32::MAX`. Unlike LOO-883 / PR #1906 (`TASK_RUN_ATTEMPTS = 3` and `TaskRunStopped` in [graph.rs](rust/loopflow/src/lf/commands/graph.rs#L164-L198)) which bounded failed-run retries, autonomous loop iteration has no default cap.
3. **Invisible `Blocked` Graph stops on status and `task-session`:**
   - [project_execution](rust/loopflow-execution/src/ops/task_execution.rs#L330-L364) checks only `latest.run.result()` (the last step's exit code) when both Graph and step processes are dead. When a deciding step exits `0` (`Succeeded`) and the Graph exits `3` (`GraphHeld`) with `error = Some("Graph is blocked: ...")`, `project_execution` reports `TaskExecutionState::Idle` (`"Graph stopped after <step>; inspect its history and effects before launching fresh work"`) and drops `graph.run.error`.
   - [RunObservation](rust/loopflow-execution/src/harness/task_updates.rs#L140-L169) never records or formats `graph.run.error`, leaving `task-session` with `"Run build · failed · realign: succeeded"` and no explanation.
4. **Cold-cache `commit-message` sessions on temporary `scratch/` edits (246 sessions, 2.95M uncached in):**
   - PR #1955 (`528ccd8eda`) added automatic `lf commit --step <skill>` after every skill step ([commit.rs](rust/loopflow-execution/src/ops/commit.rs#L188-L216), also called for standalone skills in [lf.rs](rust/loopflow/src/bin/lf.rs#L456-L458)). Steps like `realign`, `concept-review`, `coverage-behavior-review`, `kickoff`, and `ship-review` routinely edit only `scratch/` (files `lf scratch clear` deletes before merge), yet each spawned a fresh `commit-message` agent session.
5. **Per-step `task-session` wakeups during healthy runs (399 turns, 72.7M cached + 764k uncached in):**
   - [task_updates.rs](rust/loopflow-execution/src/harness/task_updates.rs#L244-L322) compares `RunObservation { step: graph.steps.last() }` and `TaskGraphPosition::Run { at, running: true }` on every `task_updates` revision bump. Every step start inside a running Graph wrote an `update:-:<task>:<key>` steer, causing [server/wake.rs](rust/loopflow/src/server/wake.rs#L160-L207) to wake the idle `task-session` LLM conversation for pure narration. Meanwhile, conducted child Tasks in the exact same file ([task_updates.rs](rust/loopflow-execution/src/harness/task_updates.rs#L105-L137), PR #2029) already suppress intermediate steps (`"Steps passing inside a run are not news"`).

## Design and choices made for review

No one is present in this headless run; the choices below are made for review (also summarized in [scratch/concept-review.html](scratch/concept-review.html) and [scratch/questions.md](scratch/questions.md)):

### 1. Bounded autonomous loops: single owner in `transitions` + `GraphRunner` + truthful `Blocked` reporting

- **Config key ([catalog.rs](rust/loopflow-execution/src/config/catalog.rs)):**
  - Add `max_loop_passes: Option<u32> = "6"` to `Config` with `Applicability::Launch, Combination::Replace, Portability::Portable, Lifecycle::NextLaunch, Category::Agents`.
  - `Some(0)` or `None` (`null`) disables the cap (unlimited passes; depth warning at pass 4+ still fires).
  - `Some(n)` (`n >= 1`) allows at most `n` total passes per loop edge in the run (initial pass 1 + `n - 1` backward returns). Cumulative per-edge counts naturally prevent nested loops (like `build.yaml`'s inner + outer loops) from multiplying into `6 × 6 = 36` passes.
  - Because `Applicability::Launch` participates in [settings::resolve](rust/loopflow-execution/src/config/settings.rs#L977), callers can set it globally, per repo, per Wave, per deciding skill (`skill.optimize-plan.max_loop_passes`), per Graph step occurrence, or per run (`lf --setting max_loop_passes=10 ...` / `LF_SETTINGS`).
  - *Compatibility note:* `Config` uses `#[serde(deny_unknown_fields)]` ([catalog.rs](rust/loopflow-execution/src/config/catalog.rs#L212)). Do **not** write `max_loop_passes` into this repository's committed `.lf/config.yaml` until a release containing it ships.
  - *Passed over:* Adding a YAML field on `loop:` blocks (breaks PR #1962's list-only syntax, breaks older parsers, and freezes the limit inside a Task's `CapturedGraph` so recovery couldn't raise it); prompt-only warning without a mechanical cap; default `4` (tight for `optimize` and legitimate 5–6 pass work).

- **Wave-aware config resolution in `RunningGraph` ([graph.rs](rust/loopflow/src/lf/commands/graph.rs)):**
  - Add a helper on `RunningGraph` that resolves the Wave slug from `self.launcher.wave` or, when absent and `self.task` is `Some(task_id)`, from `self.store.sqlite` (`task.wave_id` -> `wave.slug()`, matching [ops/task.rs](rust/loopflow-execution/src/ops/task.rs#L825)).
  - Load config with `load_wave_config(Some(self.cwd), wave.as_deref()).ok().flatten().unwrap_or_default()` and call `config::settings::resolve(&config, Some(&skill.skill))` (which folds in `INVOCATION` from `--setting`, `LF_SETTINGS`, Wave/repo/personal layers, skill frontmatter, and Graph step `occurrence`).
  - Normalize `Some(0)` to `None` (unlimited).
  - For live human interactive steps (`skill.interactive && !standing_in`), return `None` (never capped). For all autonomous steps (`!skill.interactive || standing_in`), return the resolved `Option<u32>`.

- **Decision prompt enrichment ([graph.rs](rust/loopflow/src/lf/commands/graph.rs#L1264-L1273)):**
  - Compute `pass = traversals + 1` and `body_steps = returns + 1` (steps repeated per iteration, including the decider).
  - Always tell the decider: `Decision pass {pass}[ of {max}]. The backward edge repeats {body_steps} steps back to {target}.`
  - When `pass >= 4` (or `max.is_some_and(|m| pass + 1 >= m)`), append a deep-loop warning reminding the agent of cold-cache step cost and telling it to batch remaining work, advance, or block.
  - On the final allowed pass (`max == Some(pass)`), tell the decider explicitly that returning `iterate` will stop the Graph here.

- **Single transition owner with verdict preservation ([transitions.rs](rust/loopflow-execution/src/graph/transitions.rs), [runner.rs](rust/loopflow-execution/src/graph/runner.rs), [graph.rs](rust/loopflow/src/lf/commands/graph.rs)):**
  - `transitions::finish_step(steps, index, progress, max_passes: Option<u32>)` owns the cap check on `GraphDecision::Iterate`:
    - When `max_passes` is `Some(max)` (`max > 0`) and `pass >= max` (where `pass = traversals + 1`):
      - Leave `progress` **unchanged** (keeping `progress.verdict = Some(GraphVerdict { decision: Iterate, summary })` and `progress.repeats` intact).
      - Return `GraphTransition::Blocked(reason)` naming the deciding step, `pass` of `max`, the decider's `summary` (last direction), and concrete recovery options:
        - advance past the loop with `--from <next_step>` (derived directly from `steps.get(index + 1)` when present),
        - re-evaluate with `--from <decider>`,
        - split remaining work into a follow-up Task, or
        - raise `max_loop_passes` (e.g. `lf --setting max_loop_passes=10 task run` or `0` for unlimited) and rerun to continue with the saved direction.
  - `ExecutionCursor::finish_with_limit(steps, max_passes)` (or `finish(steps, max_passes)`) passes `max_passes` into `finish_step` at the leaf body being settled (never passing an outer limit down into unrelated frames).
  - `SkillExecutor` adds a defaulted method `fn loop_pass_limit(&self, _skill: &ConcreteSkill) -> Result<Option<u32>> { Ok(None) }`. `GraphRunner::tick` queries `loop_pass_limit` for the current `ConcreteStep::Skill` before calling `settle_step`, so:
    1. Pass `max`: `run_skill` returns `SkillOutcome::Decided(Iterate, summary)` → `tick` stores `cursor.leaf_mut().progress.verdict = Some(verdict)` and checkpoints.
    2. Next `tick`: sees `verdict.is_some()` → queries `loop_pass_limit` (`Some(max)`) → `finish_step` sees `pass >= max` → returns `GraphOutcome::Blocked(reason)` while keeping `verdict` checkpointed on `cursor`.
    3. Recovery with raised cap (`lf --setting max_loop_passes=10 task run ISSUE` or `lf set max_loop_passes 10`): `GraphRunner::tick` sees the saved `verdict`, queries `loop_pass_limit` (`Some(10)`), settles `Iterate` immediately (**0 wasted decider sessions**, `summary` carried into `direction`), and jumps straight to the loop start.
    4. Accidental bare `lf task run ISSUE` without raising the cap: `tick` sees the saved `verdict`, checks `pass >= 6`, and stops immediately in <10ms (**0 LLM calls**) with the exact recovery message.
    5. Auto-standing-in `interactive:` decider hitting the cap: `execute()` ([graph.rs](rust/loopflow/src/lf/commands/graph.rs#L644-L662)) already matches `(GraphOutcome::Blocked(_), Some(ConcreteStep::Skill(skill)))` and calls `arrive_task_graph(task, carrier, step)`, returning the Task to `At { step }` for the human.
  - `run_task_graph_for` ([graph.rs](rust/loopflow/src/lf/commands/graph.rs#L282)) treats exit code `3` (`GraphHeld::EXIT`) as held (`failed == false`), so it stops once without retrying.
  - *Passed over:* Intercepting `Iterate` inside `RunningGraph::run_skill` and dropping the verdict (traps bare `lf task run` in wasted decider sessions and forces `--setting max_loop_passes=10` to re-run the decider from scratch).

- **Truthful `Blocked` status in `lf task status` ([task_execution.rs](rust/loopflow-execution/src/ops/task_execution.rs#L348-L366)):**
  - In `project_execution`, when both `graph_process` and `step` are `Dead` and `process.result() == ExecutionResult::Succeeded`, check `graph.run.error`:
    - If `graph.run.error` is present and either `graph.run.exit_code != Some(i32::from(GraphHeld::EXIT))` or the error starts with `"Graph is blocked:"`, report `TaskExecutionState::Blocked` with `graph.run.error` (trimmed of trailing `.`).
    - Otherwise (e.g., `GraphOutcome::Waiting` landing handoff or silent crash between steps with no error), keep `TaskExecutionState::Idle`.

### 2. Deterministic commit messages for `scratch/`-only step commits

- In [commit.rs](rust/loopflow-execution/src/ops/commit.rs#L188-L216), read staged paths via `git diff --cached --name-only --no-renames -z` (null-delimited to avoid path-quoting issues; `--no-renames` so a rename from code into `scratch/` still exposes the non-scratch path).
- If non-empty and **every** staged path starts with `"scratch/"`, skip `generate_commit_message` completely and commit + push with a deterministic message:
  - 1–2 files: `lf <step>: update scratch/plan.md` (or `scratch/plan.md, scratch/questions.md`)
  - 3+ files: `lf <step>: update scratch (<N> files)`
- If any staged path is outside `scratch/`, call `generate_commit_message` unchanged. Manual `lf commit` (`commit_workflow`) is unchanged. Because standalone skills in [lf.rs](rust/loopflow/src/bin/lf.rs#L456-L458) also call `commit_step`, this single owner covers both Graph steps and standalone runs.
- *Passed over:* Skipping commits for `scratch/` altogether (loses pushed crash-recovery checkpoints); keeping LLM messages for scratch-only diffs (burned ~150+ cold-cache sessions across 5 field Tasks for files deleted at PR submit/land).

### 3. Quiet running steps + block-reason reporting in `task_updates`

- In [task_updates.rs](rust/loopflow-execution/src/harness/task_updates.rs#L140-L342):
  - Add `#[serde(default, skip_serializing_if = "Option::is_none")] error: Option<String>` to `RunObservation`, populated from `graph.run.error` on settled runs and appended in `RunObservation::report` when present, so `task-session` wakes once with the exact block/failure reason.
  - Only record/report a Graph run once it has settled (`graph.entry.ended_at.is_some() || outcome.is_some()`, preserving detection of crashed runs whose process died without writing `ended_at`).
  - While `graph.position` is `TaskGraphPosition::Run { running: true, .. }`, keep `old.position` (or `None` on a baseline reading) so neither starting a run nor advancing across healthy steps (`implement` → `compress` → `realign`) writes a steer or wakes an idle `task-session` via [server/wake.rs](rust/loopflow/src/server/wake.rs#L160-L207).
  - Keep reporting every settled transition: run finished/failed/blocked/crashed, Task arrived at an interactive step (`At { step }`) or `end`, Task stopped (`Run { running: false, .. }`), PR merged, Task completed/abandoned/reopened, new non-direction comments, explicit steers, and conducted child notices (`TaskObservation::news`).
  - Backwards-compatible with pre-upgrade `task_update_readings` rows: a stored in-progress reading stays quiet while the run is live and emits one settled update once the run ends.
- *Passed over:* Keeping per-step wakeups (399 turns / 72.7M cached tokens across 5 Tasks); waking on run start only; time-based throttling.

### 4. Builtin skills, decision criteria, and docs

- **`implement.md`**: Keep "Build end to end, thinly" for Slice 1 (de-risking). Add pass-sizing guidance under `## Choose the slice`: each pass opens fresh sessions for the rest of the loop, so once the shape is proven, batch cohesive or straightforward items together (multiple cards, handlers, call sites, tests) to finish in **1–4 substantial passes** rather than one small item per pass. Commit coherent chunks as you go.
- **`realign.md`**: Under `**What is next?**`, group cohesive remaining work into one substantial batch (aiming for 1–3 remaining passes total) rather than queuing one small item at a time.
- **`kickoff.md`**: Under `**Slices**`, target **1–4 substantial passes** for the whole Task: Slice 1 proves the riskiest assumption end to end; later slices batch cohesive/repetitive items to amortize loop overhead.
- **`loop-or-next.md`**: Fix stale pre-PR #1962 `step:` wording (the last step of `loop:` decides); note that each `iterate` cold-starts every step in the loop; direct the next pass to batch cohesive remaining work; on deep loops (pass 4+) shrinking into tiny increments, advance if obligations are met or return `blocked` to split/replan instead of spinning micro-passes.
- **`DECISION_CRITERIA` (`output.rs`) & `instructions.rs`**: Replace pass-count blindness with pass-cost awareness (batch cohesive remaining work on `iterate`; `blocked` when passes shrink into repeated micro-increments or fail to converge).
- **`build.yaml` & `docs/orchestration.md`**: Keep `build.yaml`'s two-loop topology (putting `compress`, `qa`, and `concept-review` outside the lightweight `implement → sync → realign` loop); update inline comments in `build.yaml` and teach the outer-review-loop pattern + `max_loop_passes` (including cumulative per-edge bounds in nested loops) in `docs/orchestration.md` and `docs/config.md`.
- *Passed over:* Collapsing `build.yaml`'s inner loop into a single pass (large Tasks still benefit from 2–4 clean fresh-context passes once properly sized).

## Acceptance check at gate

Run isolated from ambient `LF_*` (per `AGENTS.md` / `scripts/testing.md`):

1. **Loop pass cap, depth prompt, truthful `Blocked` status, and zero-waste recovery:**
   - Focused Rust tests covering:
     - `transitions::tests::` and `runner::tests::`: `finish_step` with `max_passes = Some(6)` blocks on pass 6, leaves `cursor.progress.verdict` intact, and settles straight into `Repeat` when `max_passes` is raised (`Some(7)` or `None`).
     - `RunningGraph` / CLI Graph tests: sends `"Decision pass 1 of 6"`, warns at pass 4+, warns on final pass `6 of 6`, stops with `GraphHeld` (`exit 3`, cursor + verdict preserved, zero automatic retries) when an autonomous decider returns `iterate` on pass 6.
     - Recovery: bare `lf task run` without raising `max_loop_passes` exits 3 with **0** decider invocations; raising `max_loop_passes` (via `--setting` or Wave config) settles the saved verdict with **0** decider re-runs and carries the saved direction into the loop body; `--from <next_step>` advances past the loop; `max_loop_passes=0`/`null` disables the cap; live human interactive decider (`!standing_in`) is not capped; auto-standing-in interactive decider hands back `At { step }`.
     - `ops::task_execution::tests::`: `project_execution` reports `TaskExecutionState::Blocked` with `graph.run.error` when a Graph blocks after a succeeded step, while keeping `GraphOutcome::Waiting` handoffs `Idle`.
2. **Scratch-only commit skips LLM:**
   - `commit_tests::` proves `commit_step` with only `scratch/plan.md` staged commits and pushes with `lf realign: update scratch/plan.md` and **zero** agent invocations (stand-in agent fails test if called), while a code edit or a rename from code into `scratch/` still invokes `commit-message`.
3. **Quiet running steps & block reason in `task_updates`:**
   - `harness::task_updates::tests::` and `server::wake::tests::` prove a running Graph starting and advancing across multiple steps writes zero updates and wakes no idle conversation, while finishing, failing, crashing, blocking at `max_loop_passes` (including `graph.run.error`), or arriving at an interactive step wakes it once.
4. **Skills, goldens, and docs:**
   - `cargo test -p loopflow --test golden_prompt`
   - `cargo test -p loopflow --test documented_commands`
   - `uv run pytest python/tests/test_instruction_levels.py python/tests/test_loopflow_skill_alignment.py`
   - `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

## Source of truth

- **Loop iteration counts & pending verdict:** `ExecutionCursor.progress` (`repeats`, `verdict`, `direction`) in [transitions.rs](rust/loopflow-execution/src/graph/transitions.rs) / [runner.rs](rust/loopflow-execution/src/graph/runner.rs).
- **Loop pass limit:** `Config.max_loop_passes` in [catalog.rs](rust/loopflow-execution/src/config/catalog.rs), resolved with Wave + Skill + occurrence + CLI/env via `load_wave_config` + `config::settings::resolve`.
- **Blocked execution status:** `ops::task_execution::project_execution` reading `graph.run` + `latest.run` in [task_execution.rs](rust/loopflow-execution/src/ops/task_execution.rs).
- **Settled Task updates:** `task_updates::write` in [task_updates.rs](rust/loopflow-execution/src/harness/task_updates.rs) reading SQLite `graph_processes` + `task_progress`.
- **Automatic step commits:** `ops::commit::commit_step` in [commit.rs](rust/loopflow-execution/src/ops/commit.rs).

## Affected surfaces and consumers

- **Config catalog & settings:** `rust/loopflow-execution/src/config/catalog.rs` (picked up automatically by `lf set`, `lf config catalog`, `lf config settings`, Desktop settings browser).
- **Graph transitions, runner, prompts, and execution:** `rust/loopflow-execution/src/graph/transitions.rs`, `rust/loopflow-execution/src/graph/runner.rs`, `rust/loopflow-execution/src/graph/output.rs`, `rust/loopflow-execution/src/graph/instructions.rs`, `rust/loopflow/src/lf/commands/graph.rs`.
- **Task execution status:** `rust/loopflow-execution/src/ops/task_execution.rs` (read by `lf task status`, Wave board, Desktop).
- **Step commits:** `rust/loopflow-execution/src/ops/commit.rs`, `rust/loopflow/tests/commit_tests.rs`.
- **Task updates & server wakeups:** `rust/loopflow-execution/src/harness/task_updates.rs`, `rust/loopflow-execution/src/harness/README.md`, `rust/loopflow/src/server/wake.rs`.
- **Builtin skills, goldens, and user docs:** `rust/loopflow-definitions/src/builtins/task/skill/{implement,realign,kickoff,loop-or-next}.md`, `rust/loopflow-definitions/src/builtins/task/graph/build.yaml`, `tests/goldens/builtin_{implement,realign}.md`, `docs/orchestration.md`, `docs/config.md`.
- **Unaffected (must keep working):** Desktop live Graph progress & `lf task status` live step view (read SQLite directly), `TaskSummary.loop_passes`, `ExecutionCursor` JSON format, `loop:` YAML block syntax, manual `lf commit`, conducted child Task notices.

## Absent and error states

- **`max_loop_passes` unset:** defaults to `Some(6)`.
- **`max_loop_passes` set to `0` or `null`:** treated as unlimited (no mechanical stop; depth warning at pass 4+ still appears in decision prompt).
- **Invalid `max_loop_passes` (negative or non-integer):** rejected by typed catalog schema validation (`settings::validate_value`), same as every other `u32` config field.
- **Unreadable config when resolving `max_loop_passes` in `RunningGraph`:** falls back to `Config::default()` (`Some(6)`).
- **Empty staged diff in `commit_step`:** unchanged (no commit, push if unpushed commits exist).
- **Pre-upgrade `task_update_readings` row with in-progress run:** ignored while live; emits one update when settled; missing `error` field deserializes as `None`.
- **Crashed Graph step (process died without `ended_at`):** `observed_result` returns `Some("unknown")`, so the failure is still reported once to `task-session`.
- **Healthy `GraphOutcome::Waiting` handoff (e.g., `lf pr land`):** `project_execution` keeps `TaskExecutionState::Idle` (not falsely marked `Blocked`).

## Operational boundary

- **Zero extra SQLite queries or schema migrations.**
- **Zero extra LLM calls on hot paths:** scratch-only commits drop from 1 cold-cache LLM session (~12k uncached tokens, ~15s) to 0 tokens (<10ms); healthy running Graph steps drop from 1 `task-session` LLM turn per step (~180k cached tokens) to 0 turns; resuming or accidentally rerunning after a `max_loop_passes` stop costs 0 decider sessions.
- **10,000-Task store load:** `task_updates::write` runs the same indexed queries as before while writing far fewer `steers` rows (less SQLite write-lock contention).

## Exclusions

- Cross-skill provider session/thread sharing (reusing one provider conversation across different skills in a Graph) — separate architectural design (see LOO-435).
- Editing external repositories' custom Graphs (such as `manabot/.lf/graphs/coverage-working.yaml`) — those benefit automatically from builtin skill updates (`extends: builtin`), `max_loop_passes`, scratch-only commit skipping, and quiet `task-session` wakeups, and can adopt `build.yaml`'s outer-review pattern via `docs/orchestration.md`.
- Hiding `Write commit message` from the Task Session list (separate store concept noted in PR #1955).

## What goes

- The `"not from ... the number of passes"` wording in [output.rs](rust/loopflow-execution/src/graph/output.rs#L11) and `"There is no arbitrary pass limit"` in [instructions.rs](rust/loopflow-execution/src/graph/instructions.rs#L69) and [orchestration.md](docs/orchestration.md#L146).
- The stale `step:` wording in [loop-or-next.md](rust/loopflow-definitions/src/builtins/task/skill/loop-or-next.md#L8-L9).
- The per-step running-update assertions in [task_updates.rs](rust/loopflow-execution/src/harness/task_updates.rs#L439) (`a_run_between_steps_keeps_its_last_report`) and the `"the child's own conversation hears its steps"` sub-assertion in [task_updates.rs](rust/loopflow-execution/src/harness/task_updates.rs#L617-L621), replaced by tests asserting quiet running steps and single settled-run/position reports (including `graph.run.error`).

## Slices

### Slice 1 (de-risking first): `max_loop_passes` config + verdict-preserving cap in `transitions`/`GraphRunner` + depth-aware prompt + truthful `Blocked` status
Proves the core loop-bound and recovery architecture end to end first: typed `Launch` config (`max_loop_passes`), Wave/skill/occurrence/`--setting` resolution in `RunningGraph`, pass-depth + step-span + pass-4/final-pass prompt injection, verdict-preserving cap enforcement in `transitions::finish_step` via `SkillExecutor::loop_pass_limit`, zero-waste resume & bare-rerun behavior, human interactive exemption, auto-standing-in handback, and `project_execution` reporting `TaskExecutionState::Blocked` with `graph.run.error`.
- **Files:** `rust/loopflow-execution/src/config/catalog.rs`, `rust/loopflow-execution/src/graph/transitions.rs`, `rust/loopflow-execution/src/graph/runner.rs`, `rust/loopflow-execution/src/graph/output.rs`, `rust/loopflow-execution/src/graph/instructions.rs`, `rust/loopflow-execution/src/ops/task_execution.rs`, `rust/loopflow/src/lf/commands/graph.rs`, plus focused unit & CLI Graph tests.
- **What it shows:** A looped Graph fixture whose stand-in decider returns `iterate` receives pass counts + pass-4 warning + final-pass warning, stops cleanly at pass 6 with exit 3 (`GraphHeld`), shows `Blocked` with the exact reason in `lf task status`, stops in 0 LLM calls on bare rerun, and continues straight into the loop body with the saved direction (0 decider re-runs) when rerun with `--setting max_loop_passes=10` (or Wave config / `0`).

### Slice 2: Deterministic `commit_step` for `scratch/`-only diffs
Skips `generate_commit_message` in `ops::commit::commit_step` when every staged path (`git diff --cached --name-only --no-renames -z`) is under `scratch/`, still committing and pushing with `lf <step>: update scratch/...`.
- **Files:** `rust/loopflow-execution/src/ops/commit.rs`, `rust/loopflow/tests/commit_tests.rs`.
- **What it shows:** `commit_step` commits and pushes `scratch/plan.md` with zero agent calls, while code edits or code-to-`scratch/` renames still invoke `commit-message`.

### Slice 3: Quiet running steps + block-reason reporting in `task_updates`
Updates `harness::task_updates` so starting or advancing through healthy running steps writes no steer and wakes no idle `task-session`, while settled runs (with `graph.run.error` included), interactive/end stops, crashed runs, merges, closes, and comments wake once.
- **Files:** `rust/loopflow-execution/src/harness/task_updates.rs`, `rust/loopflow-execution/src/harness/README.md`, `rust/loopflow/src/server/wake.rs` (regression test).
- **What it shows:** Starting and advancing a live Graph across multiple steps leaves an idle `task-session` untouched (0 steers, 0 wakes); ending or blocking at `max_loop_passes` delivers one update containing the exact block reason.

### Slice 4: Builtin skills, prompt goldens, and user docs
Updates `implement.md`, `realign.md`, `kickoff.md`, `loop-or-next.md` (including removing stale `step:` syntax), and `build.yaml` comments for shallow cohesive passes; regenerates `tests/goldens/builtin_implement.md` and `tests/goldens/builtin_realign.md`; updates `docs/orchestration.md` (teaching the outer-review-loop pattern and `max_loop_passes`) and `docs/config.md`.
- **Files:** `rust/loopflow-definitions/src/builtins/task/skill/{implement,realign,kickoff,loop-or-next}.md`, `rust/loopflow-definitions/src/builtins/task/graph/build.yaml`, `tests/goldens/builtin_{implement,realign}.md`, `docs/orchestration.md`, `docs/config.md`.
- **What it shows:** `golden_prompt`, `documented_commands`, `test_instruction_levels.py`, and `test_loopflow_skill_alignment.py` all pass, and `lf --context` on `implement`/`realign` shows the new batching guidance.

## Demo

In an isolated temp repo + `LF_HOME` with a stand-in provider whose decider keeps returning `iterate` and whose `realign` edits `scratch/plan.md`:

```bash
lf --setting max_loop_passes=3 -b graph loop-demo
```

1. Passes 1–3 run; `realign`'s scratch-only edits commit as `lf realign: update scratch/plan.md` with **zero** `Write commit message` sessions.
2. Pass 2's decider receives `Decision pass 2 of 3` + deep-loop warning; pass 3's decider receives `Decision pass 3 of 3` + final-pass notice.
3. Returning `iterate` on pass 3 exits with code `3` (`GraphHeld`), preserves the `Iterate` verdict on the cursor, reports `TaskExecutionState::Blocked` in `lf task status`, and prints:
   `Graph is blocked: loop at realign reached its pass limit (3 of 3; last direction: …). Advance with --from <next>, re-evaluate with --from realign, or raise max_loop_passes (e.g. lf --setting max_loop_passes=7 task run or 0 for unlimited) to continue.`
4. Rerunning with `--setting max_loop_passes=5` continues straight into pass 4's loop body with the saved direction (**zero** re-run of `realign`).
5. A bound idle `task-session` wakes **once** (on the blocked settlement, carrying the block reason), not on each step of passes 1–3.
6. `lf config settings --explain | grep max_loop_passes` displays the `6` builtin default and any overrides.
