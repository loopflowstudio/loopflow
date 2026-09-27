# Cut H review — H1: Task points at its invocation; strict equality

2026-09-27 · LOO-298 · review-slice of `4820e1f86` (code) and `0fdda5066`
(ledger), diff base `7eb3046a8`. Brief: [Cut H](cut-h-one-flow-driver.md)
(slice H1 and its ledger entry). Decisions: [concept review](../concept-review.md)
and the 2026-09-27 entries in [questions](../questions.md). Later sections of
this file supersede earlier ones; the H1 section stands and the [H2 section](#cut-h-review--h2-the-saved-flow-runs-on-the-row) below is current.

## Takeaway

H1 holds. Every H1 claim passes through the compiled `lf` against a private
Home except the one the probe Home cannot reach (`lf task run` wants a Linear
team); the pointer is proven there through the store on the same Home. One
gap was found and fixed in this pass: the draft left a pre-H1 `--task` Flow's
invocation disagreeing with its Runs. Two items need Jack, neither blocks H2.

## Evidence matrix

Probe Home: `LF_HOME` under the session scratchpad, `LF_CONTROL_HOME` and
`LF_CONTROL_DB_PATH` cleared, every `LF_*`/`LOOPFLOW_*` cleared, `nice -n 10`,
`-j 4`; Tasks `INF-123` (`task_c098bf09…`) and sibling `INF-124` registered
through the `session_cutover_tests` fixture code; `review-first` is a Flow whose
only step is a human review, so nothing needs a provider stand-in.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Task pointer | `tasks.current_invocation_id` is the one selector of a Task's Flow; `flow_position` reads through it | `durable::TASK_INVOCATION` in every Task-keyed invocation write; `set_flow_position` at version 0 inserts and points (guarded `current_invocation_id IS NULL`); finish, restart, reopen and human completion clear it in their transaction | Live: pointer set through the store to `9d7c2744…`; then `lf --task INF-123 flow review-first` a third time → `SELECT current_invocation_id FROM tasks` unchanged; `flow_invocations` shows four current rows naming INF-123 and `count(tasks pointing)=1` for exactly one. Store test `a_task_points_at_one_invocation_while_other_flows_name_it` | pass |
| Strict equality | `runs.task_id IS flow_invocations.task_id`, both nullable | Trigger text in the probe schema: `f.task_id IS NEW.task_id`; `insert_run_in` fills or refuses | Live: `sqlite_master` shows the strict `validate_run_parents_insert`; binding a taskless Flow's review to INF-123 aborts with `Run and Invocation nullable Tasks disagree`. Lib: constructor matrix rejects a Task-naming Run in a taskless invocation | pass |
| Pre-H1 rows agree after the draft | Done-when 3: every Run with an invocation agrees with it | **Gap.** The draft filled the pointer and dropped the index but left Cut F's step Runs (`task_id=X`, invocation `NULL`) and Cut 3's Wave-only review Runs as they were | New `migrations::tests::pointing_tasks_at_invocations_names_the_task_on_earlier_task_flows` seeded a managed, an about and a taskless invocation at the pre-draft schema: failed `left: "1" right: "0"` on the disagreement count; passes after the backfill below | fixed |
| Draft fill never picks arbitrarily | Pointer fill from the current invocation per Task | `UPDATE tasks SET current_invocation_id=(SELECT id … WHERE task_id=tasks.id AND state='current')` runs while `task_current_invocation` (`UNIQUE(task_id) WHERE task_id IS NOT NULL AND state='current'`) is still live; `DROP INDEX` follows | Source: draft order; the same test asserts the pointer is `managed` with an `about` row present | pass |
| `lf --task X flow F` names X on step and review Runs, leaves the pointer | Decision 1 | `save_flow_in` writes `task_id` from `declared_work`; `flow_session::reserve` names the Task on the review Run | Live: after `lf --task INF-123 flow review-first -b --no-loopflow` the invocation has `task_id=task_c098…`, the review Run has that Task, its Wave and `work_source=declared`; `lf session list --json` `work` = `{"kind":"task"}` with `flow_membership.kind=step`; `lf runs --task INF-123 --json` lists the Run with `task:INF-123`; pointer `NULL` before and after (three launches). Step Runs: `flow_tests::bound_flows_keep_task_context…` (six Runs, all `task:INF-123`) | pass |
| `lf task run X --flow F` moves the pointer | Done-when 4 | `ensure_flow_position` returns the existing position when the Task has one and ignores `--flow`; only a Task with no Flow gets a new invocation and pointer. Restart (`restart_task_flow`) is the mover | Live: `lf task run INF-123 --flow review-first` refused in the probe Home (`.lf/config.yaml has no repository pm.linear_team`), so the setter was proven through `store.set_flow_position` on the same Home. Source: `controller/task/mod.rs::ensure_flow_position` | pass for H1's scope (set and clear); **gap for the cut**: "moves" is restart's, not `task run --flow`'s |
| Same-Task bind idempotent, exact target printed | Decision 3 | `bind_session_runs_in` returns `Ok` when the bound Task is the requested one; `human_session::bind` prints on stderr before writing | Live: second `lf session bind <review> --task INF-123 --json` exits 0 with byte-identical JSON; stderr `Binding flow:732e… to INF-123 (Prove Task PR transitions). Permanent.`; `--task INF-124` exits 1 with `already has Task INF-123; a Run's Task never changes`. `session_cutover_tests::binding_an_orphan_session_starts_its_task_once` | pass |
| Flow-review refusal gone | A Task-about review binds like any Session | Refusal branch deleted; the strict trigger is the only guard | Live: the Task-about review bound (no-op) without the old `is a Flow review` message; taskless review reached the trigger. `rg "is a Flow review; its Runs keep" rust/loopflow/src` empty | pass |
| Four deletions absent | Brief "Delete" list | — | `rg` over `rust/loopflow/src`: `task_current_invocation` only in the creating and dropping drafts; `IS NULL OR f.task_id` only in the `name_tasks_on_flow_step_runs` draft this one supersedes; "carries the Wave and no Task" none; review-refusal string none. Probe schema: index count 0 | pass |

## Negative architectural proof

- No Legacy/New adapter and no branch on Task presence in a Flow write:
  every Task-keyed invocation statement goes through `TASK_INVOCATION`; the
  saved-Flow statements key by invocation id. The remaining dispatch is the
  designed one between a Task's own review (`managed_review`, pointer) and a
  waiting saved Flow (`waiting_flow`) in `owned_target` and `surface`; that is
  the two-driver split H2/H3 delete, not a new adapter.
- No selector string on `flow_invocations`: the table has no `cwd`, `message`
  or `model` yet; `as_work` lives only in `FlowRun`/`position.json`, H2's target.
- No dual write of the pointer: one setter (`set_flow_position_in`, version 0,
  `WHERE current_invocation_id IS NULL`) and four clearers, each in the
  transaction that ends the invocation (`finish_task_flow_in`, `restart_task_flow`,
  `reopen_work_in`, `complete_human_task_boundary`). `end_flow` and
  `waiting_flow` exclude pointed-at rows. A version-0 set on a Task that already
  points elsewhere inserts, fails the pointer update, returns
  `InvalidAuthority` and rolls the insert back with the transaction.
- Latent, not wrong today: `task_started`, `chapter_task_evidence`,
  `retire_chapter_backlog` and `require_current_task_chapter` select
  `flow_invocations WHERE task_id=? AND worker_generation>0`, which now spans
  about-Flows too. Safe because `save_flow_in` writes `worker_generation=0` and
  only the pointed-at invocation is ever claimed. H3 deletes the clause.
- Test-only stale selectors `WHERE state='current' AND task_id=?1`
  (`durable.rs` 2776, 2829) and `WHERE task_id=?1` (`controller/task/mod.rs`
  2835, 2870) still address "the Task's invocation" by Task; correct while
  those fixtures hold one invocation per Task.
- Verdict: the slice advances the full design and leaves no dead end for H2.

## Findings

1. **Fixed here.** Draft `point_task_at_invocation` now backfills, after the
   pointer fill and index drop and before the strict triggers: a taskless
   invocation whose Runs name exactly one Task gives that Task to its Task-less
   Runs (Wave filled when null, left alone when it already matches) and then
   takes the Task itself. An invocation whose Runs never named a Task stays
   taskless; one whose Runs disagree is left as it was. Proof: the new
   migration test; `scripts/check_migrations.py` unchanged (52 shipped).
2. `human_session::bind` prints `Binding … Permanent.` before the store write,
   so a refused bind reads "Binding X to INF-124 (…). Permanent." followed by
   the refusal. This is what the brief specified; whether the line should
   follow the write instead is UX for Jack.
3. Binding a taskless Flow's review surfaces the trigger text with
   `sqlite error: … Caused by: … Error code 1811: constraint failed`. Honest,
   not a user message. The pair `validate_run_parents_update` and
   `validate_invocation_run_parents` makes "assign the whole invocation" un-
   spellable in either statement order without a relaxed trigger, so it is a
   design choice, not a fix.
4. `lf task run X --flow F` on a Task with a live Flow ignores `--flow`
   (`ensure_flow_position`). Done-when 4 needs an owner: either restart is the
   mover and the wording changes, or H3 gives `task run --flow` replace
   semantics.
5. `lf task run` refuses without `pm.linear_team` in the repo config, so the
   worker path cannot be driven in a fixture Home. Nothing about H1; it caps
   what a review can prove through the binary until a stand-in is possible.

## The two decisions made for Jack

- **A review Run naming the Task starts it.** Consistent with Cut E's
  "reservation counts" and decision 1's "names X on every Run". The only
  alternative is a Started writer that asks whether the Run's invocation is the
  pointer, which is a new branch on Task-flow identity in the one place the
  cut is removing them. Keep; no decision needed. Visible consequence:
  `lf --task X flow review-first` starts X the moment it parks.
- **Taskless review bind refused by the trigger message.** Correct under strict
  equality; the refusal itself needs no decision. Jack's call is only whether
  bind should ever assign a whole taskless invocation and every Run in it
  (finding 3). Until then the raw message stands.

## Commands and results

| Command | Result |
| --- | --- |
| `uv run python scripts/resource_envelope.py` | ok |
| `cargo build -p loopflow --bin lf` | ok; the probe drives `target/debug/lf` |
| `cargo nextest run -p loopflow --lib -E 'test(pointing_tasks_at_invocations_names_the_task_on_earlier_task_flows)'` before the draft change | FAIL `every Run agrees with its invocation on the Task: left "1" right "0"` |
| Same, after | PASS; `session_ownership_import_preserves…` PASS |
| `uv run python scripts/check_migrations.py` | 52 shipped migrations unchanged since v0.12.21 |
| `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` | pass (one fmt diff in the new test, formatted) |
| `cargo nextest run -p loopflow --no-fail-fast --lib --test flow_tests --test session_cutover_tests -E 'test(store::) \| test(a_task_points_at) \| test(run_constructor_infers) \| test(bound_flows_keep) \| test(binding_an_orphan) \| test(observing_and_preparing)'` | 159 passed, 0 failed (393 s) |

## Not proven

- `lf task run` and the worker through the binary (finding 5); the pointer's
  setter is proven through the store on the same Home and its stability through
  the binary.
- Step Runs of `lf --task X flow F` live; `flow_tests` covers them.
- The draft against a copy of an installed Home; the synthetic pre-H1 rows in
  the migration test are the closest proof.
- A `--wave` or `--as` launch's review under the strict trigger.

## Recommended next action

Proceed to H2; no further H1 implement pass. Carry findings 2–4 to Jack in the
next loop-decide, and let the H2/H3 ledger name the owner of Done-when 4. H2's
first failing assertion is unchanged from the ledger: `flows/<id>/position.json`
never exists after `lf --task X flow <flow with a review>`.

# Cut H review — H2: the saved Flow runs on the row

2026-09-27 · LOO-298 · review-slice of `4bed6f562` (code and ledger), diff base
`d0a5469f5`. Brief: [Cut H](cut-h-one-flow-driver.md) (slice H2 and its ledger
entry). Decisions: [concept review](../concept-review.md) and the "Cut H2" entry
in [questions](../questions.md). Supersedes nothing in the H1 section.

## Takeaway

H2 holds on the row: every H2 claim passes through the compiled `lf` against a
private Home, including the two paths the implement pass listed as not proven
(a saved Flow with two reviews; a `--wave` launch's review). One decision made
for Jack was wrong and is fixed here: an operation interrupted mid-way replayed
on resume. It now records a Run like every other step, and recovery blocks it
for inspection until `--retry`, which is what `docs/lf.md` already promised.
The +387 is a second fence spelled next to the Task's; H3 has to delete the
Task twin by name or H2 is a dead end. Proceed to H3 with that list.

## Evidence matrix

Probe Home: `LF_HOME`=`LF_CONTROL_HOME` under the session scratchpad,
`LF_DB_PATH`=`LF_CONTROL_DB_PATH` inside it, every `LF_*`/`LOOPFLOW_*` cleared,
`nice -n 10`, `-j 4`; the compiled `target/debug/lf` carrying the fix below.
Wave `probe-wave`, one Project, Task `INF-123` with an open PR seeded by SQL
in the shapes the store reads; provider stand-in `opencode` exits 7 while a
`fail-once` marker exists and otherwise runs `lf flow decide advance` when
`LF_FLOW_STEP` is set. Flows: `work-then-review`, `work-then-decide`,
`op-then-review` (`- op: rebase --plan`), `two-reviews`.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| `position.json` never written; the row is the only cursor owner | `flow_invocations` owns cursor, attempt, failure; `flows/<id>/` holds `driver.lock` only | `FlowInvocation` read by `store/sqlite/flows.rs::FLOW_SELECT`; cursor written only by `write_cursor_in` and the two verdict/route writers; `flow_run.rs` is 88 lines of lock + `ActiveStep` | Live: after four invocations `find flows -type f` lists four `driver.lock` files and nothing else; `rg 'position\.json\|FlowRun\b\|StepToken' rust/loopflow/src` returns `ops/session_import.rs` lines 89 and 199 only | pass |
| `LF_FLOW_STEP` is `{invocation, version}` | Token names the invocation and the cursor version it was launched at | `flow_run::ActiveStep { invocation, version }`; `capture_membership` refuses a stale version | Live: the stand-in's nested `lf flow decide` at the work step is refused (`this Flow step does not own a decision`, twice) and at the deciding step records once; source `ops/flow_run.rs:15` | pass |
| decide/route/blocked key by invocation | One store function keyed by invocation id, fenced by `current_run_id` | `record_flow_decision` / `record_flow_path` / `flow_blocker_key` take `step.invocation`, `step.version`, the Run; the Task-keyed `record_flow_verdict`/`record_flow_route` resolve the pointer then call the same `record_verdict_in`/`record_route_in` | Live: `work-then-decide` completes with `review_json.index=2` and state `completed` after the in-step `Decision recorded`; store test `a_decision_belongs_to_the_current_attempt…` (wrong Run, wrong version, conflicting decision, moved position all refused). Route and blocked: source only | pass (route/blocked source-level) |
| `recover` reads `runs.outcome` | The row's outcome settles the attempt; `terminal.json` stays the receipt | `FLOW_SELECT` subselects `runs.outcome` for `current_run_id`; `settle_attempt_in` blocks on a failed Run, waits on an unsettled one, passes a completed one | Live: the exit-7 attempt is `runs.outcome='failed'` and `failure_json.reason` = `work-proof Run failed: agent exited with code 7…`; `lf flow resume <id>` without `--retry` refuses with that reason and the retry hint | pass |
| A Flow refuses to start without a writable store | Decision 2: the store error is the refusal; no provider starts | `execute` runs `store.create_flow` before `drive_saved`; no warning path | Live: `LF_DB_PATH` in a `0555` directory → exit 1, `unable to open database file`, stand-in launch count unchanged (4 → 4); test `a_flow_refuses_to_start_without_its_row` | pass |
| The review Session lists under the Task | Decision 1: every Run of a `--task` Flow names the Task; the review is the Task's about-Flow, not its pointer | `reserve` writes `task_id`/`wave_id` from the row; `insert_run_in` fills and validates | Live: `lf session list --all --json` → `kind=flow`, `work={"kind":"task","id":task_…}`, `flow_membership.step=review-proof node=1`; `lf runs --task INF-123 --json` lists exactly the failed attempt, the retry and the review Run; `tasks.current_invocation_id` stays NULL through launch, retry and completion; `started_at` set | pass |
| `lf flow resume --retry` after a failed step | Retry clears the failure and the attempt under the version fence; the next attempt is `attempt=2` | `retry_flow` → `write_cursor_in(clear_attempt=true)` | Live: attempt 2 `completed`, Flow parks at the review (`position_version` 2 → 4, `pending_session_id=session_…`); after `session ready` inside the review and `session complete`, `lf flow resume` exits 0 with state `completed`, three Runs | pass |
| A saved Flow with two reviews (implementer: not proven) | Each review is its own Session; the row's `pending_session_id` moves | `reserve` mints `session_<uuid>` per waiting step | Live: `two-reviews` parks twice with distinct `session_` ids; complete → resume → second review; complete → resume → `completed`, `review_json.index=2`, two Runs, no open Session of its own | pass |
| A `--wave` review launch (implementer: not proven) | The Flow's Wave survives on the row; its Runs and review name the Wave, no Task | `execute` writes `wave_id` from the binding; `reserve` copies it | Live: `lf --wave probe-wave … flow op-then-review` → row `task_id NULL, wave_id=<wave>, cwd set`; review Run `wave declared`; Session `work={"kind":"wave"}`; `lf runs --wave probe-wave` lists the Op Run and the review Run | pass |
| An interrupted operation is a recorded failure needing `--retry` | Design: a cursor is never exactly-once external effects; `docs/lf.md:443` "inspect any operation's effects before choosing `--retry`" | **Gap at `4bed6f562`**: `run_op` ran `execute_flow_ops` with no attempt on the row, so a driver killed mid-`lf pr land` replayed it on the next resume (ledger decision 3). Fixed below | Store test `an_interrupted_operation_blocks_for_inspection_instead_of_replaying`: with the old arm, `recover_flow` returned `Flow … is waiting for Run …; its completion is not recorded` forever; with the fix it blocks with `op: pr land Run interrupted before its completion receipt…`, the Run row reads `interrupted`, `--retry` opens attempt 2. Live: the `op: rebase --plan` step is Run `node 0 attempt 1 outcome completed provider loopflow skill NULL` | fixed |
| Bind line prints after the write (H1 review finding 2) | A refused bind prints only its refusal | `human_session::bind` moved the `eprintln!` after `bind_session` | Source: `ops/human_session.rs` diff | pass (source) |

## Negative architectural proof

- Reachable writers of a saved Flow's cursor: `write_cursor_in` (checkpoint,
  fail, retry, settle) and `record_verdict_in`/`record_route_in` (review_json
  only, no version bump; `checkpoint_flow` merges). No file writer exists;
  `flow_run.rs` has no serialization of a cursor. No dual write of failure: the
  driver (`fail_flow`) and recovery (`settle_attempt_in`) are two triggers into
  one function.
- Fallback readers: `position.json` is read by `session_import.rs` only. No
  `FlowRun`, `StepToken`, `position.lock`, `save_flow` remain.
- Selector strings on `flow_invocations`: none in the draft SQL or the
  writers; `as_work` appears once, as `launch.as_work = None` in `drive_saved`,
  clearing the CLI selector before the row's Task/Wave are applied.
- Executors: one production `impl SkillExecutor` (`CliFlowExecutor`) plus the
  engine's test `RecordingExecutor`.
- Legacy/New branches on Task presence inside Flow operations. Found five;
  only the first is the fallback H3 owns:
  1. `lf/commands/flow.rs::control` `decide`/`route`/`blocked`: the `None`
     token arm resolves the checkout's Task and calls the Task-keyed store
     functions or builds `task:<task>:<inv>:<boundary>` (lines 191–192,
     209–214, 233–260). H3.
  2. `decode_flow` refuses `cwd IS NULL` with "a Task's own Flow runs through
     `lf task run`" (`flows.rs:105–110`). The launch fact `cwd` lives on
     `tasks.worktree` for a Task invocation and on the row for a saved one:
     "where does cwd live" depends on Task presence. H3 must fill the column
     for Task invocations (validated copy) or make the one executor read the
     Task's worktree through the pointer; today `lf flow resume` on a Task's
     invocation is unspellable by this refusal.
  3. `run_record::record_row` returns early when `RunFlowStep.task_id` is
     `Some` ("managed Task step, stored at claim", 1683–1689) and
     `RunFlowStep::of_flow` sets it `None` (decision 6). The Task id doubles as
     a flag. Goes when H3's claim inserts the worker Run and the launcher only
     publishes it.
  4. `commit_skill_work` runs between steps only when `cli.task` and
     `cli.wave` are both `None` (`flow.rs:613`). Pre-existing (the old line
     also tested `as_work`); reads the row through `launch` now. A behavior
     choice on Work presence, not a persistence adapter; keep visible.
  5. `flow_session::launch` renders `--task` or `--wave` for the review's
     child process from the row (a selector rendering, not a store branch).
- Verdict: the slice advances the full design. It is a dead end only if H3
  deletes less than the duplicate list below.

## Duplicate fence and settle logic (flows.rs against durable.rs)

The +387 is not one fence spelled out; it is the saved-Flow fence spelled next
to the Task's. Both key `flow_invocations.position_version`; the Task's adds
`claim_json` equality and `TASK_INVOCATION`. Concrete pairs, current line
numbers:

| Operation | `store/sqlite/flows.rs` | Task twin | H3 |
|---|---|---|---|
| Row decode into a Rust type | `decode_flow` 73–138 → `FlowInvocation` | `read_flow_position_row`/`flow_position_in` `durable.rs:958–994` → `FlowPosition`; both through `decode_flow_cursor` 1336 | one type, one decoder |
| Insert at version 0 | `insert_flow_in` 189–215 | `set_flow_position_in` INSERT branch 996–1030 | one insert |
| Cursor write under the version fence | `write_cursor_in` 217–246 (`position_version=?2 AND state='current'`) | `settle_task_worker_in` 1150–1195 (`… AND claim_json=?7`), `set_flow_position_in` UPDATE branch ~1049–1056 | one writer, claim as an optional fence |
| Block with a failure | `fail_flow` 504–520 | `block_task_flow_in` 1078–1113 | one |
| Release the attempt / clear failure | `retry_flow` 432–452 (`clear_attempt`) | `release_task_worker_in` 1115–1148, restart | one |
| Which Run may act | `require_attempt_authority` 303–323 (version + `current_run_id` + no failure) | `claimed_position_in` 911–931 (`claim.worker_run_id`); both then `runs::require_attempt_in` 286 | one |
| Settle the attempt from its outcome | `settle_attempt_in` 263–301 reads `runs.outcome` | `controller/task/mod.rs::recover_task_decision` 583–640 reads `terminal.json` via `read_run_snapshot` | the row (H4 deletes the file reader) |
| Loop blocker key | `flow_blocker_key` 558–577 `flow:<inv>:<node>:<iterations>` | `control("blocked")` `None` arm `flow.rs:233–260` `task:<task>:<inv>:<boundary_key>` | one key |
| Operation step as a Run | `CliFlowExecutor::run_op` (this review) | `controller/task/mod.rs::run_task_op_boundary` 511–580 + `run_task_flow_op` 867 | one |

The brief's H3 deletion list names only the controller's traversal, the
Task-keyed verdict/route functions and the Started event writer. It must also
name `settle_task_worker_in`, `block_task_flow_in`, `release_task_worker_in`,
`set_flow_position_in`'s UPDATE branch, `FlowPosition`/`read_flow_position_row`
(folded into `FlowInvocation`), `recover_task_decision`, `run_task_op_boundary`
and the `task:` blocker key. Otherwise two fences remain on one row and rule 2
("one code path per operation") is not met.

## Findings

1. **Fixed here — an interrupted operation replayed.** Decision 3's premise
   ("an Op step has no Run") is false for the one implementation H3 converges
   on: the Task path already records an operation as a Run (`run_task_op_boundary`:
   `task_run_spec(…, "loopflow", None, "operation", …)`, `skill: None`, then
   `fill_run_provider(run, "loopflow", None)`), so `Run.skill`/`provider` being
   `Option` was never a DTO question. The old file guard ("interrupted before a
   completion receipt … require inspection rather than replaying it") was the
   design's "a cursor is not exactly-once external effects", and `docs/lf.md:443`
   still promises it. Ops are `lf pr …` and `lf rebase …`; replaying a
   half-finished rebase or `pr land` on resume is the 2 a.m. failure. Fix:
   `run_op` records the operation as a Run (`harness loopflow`, `surface
   operation`, `skill None`, membership `Step`, work from the row) and settles
   it `completed`/`failed`; a completed attempt at the same position is the
   step's completion, so a crash between `end_run` and the checkpoint does not
   replay either. `settle_attempt_in` ends an operation Run without an outcome
   as `interrupted` and blocks with `op: <name> Run interrupted before its
   completion receipt; inspect its effect before retrying`; the driver holds
   `driver.lock`, so no other process can be running that operation (the
   precondition is stated on `recover_flow`). A skill Run without an outcome
   still keeps the Flow waiting. Production lines: `lf/commands/flow.rs`
   717 → 753 (+36), `store/sqlite/flows.rs` 582 → 597 (+15); cut net +438.
2. `cwd` on the row vs `tasks.worktree` (negative-proof item 2). H3 owner.
3. `RunFlowStep.task_id` as a flag in `record_row` (item 3). H3 owner.
4. `record_verdict_in`/`record_route_in` update `review_json` without bumping
   `position_version` (`flows.rs:362–365`, `395–398`); `checkpoint_flow` merges
   the recorded leaf. Correct today; the fence's meaning is "position", so a
   reader that caches `review_json` by version is stale by design. Keep visible.
5. A skill Run whose driver died leaves the Flow at `Flow … is waiting for Run
   …; its completion is not recorded` with no operation to settle it except
   H4's liveness reconciliation. Unchanged from the file era; not an H2 gap.
6. `message` on the row is the *bound* message: the rendered `<lf:work>` block
   with the Task's title, description, PR and base commit frozen at launch.
   A launch fact by the brief's definition, so a resume re-reads the stale
   rendering deliberately. Note only.
7. Cosmetic: the step launch line for a `--task` Flow prints
   `lf work-proof --wave <wave id> …`; the Run row names the Task. `drive_saved`
   sets both `launch.task` and `launch.wave`; the printer shows the Wave.
8. `flows/<id>/driver.lock` stays after completion (an empty lock file). The
   design allows it ("may be deleted at rest with nothing lost").

## The decisions made for Jack, judged

1. **`wave_id` on `flow_invocations` — keep.** For a Task invocation it is a
   copy of `tasks → projects.wave_id`, validated by
   `validate_invocation_wave_insert/_update` (rule 1: a copy exists only with a
   validator); for a taskless Flow it is the owner of "launched for this Wave".
   Deriving it at read would put "depends on whether there is a Task" back into
   `FLOW_SELECT`. The update trigger fires on `task_id`/`wave_id` only; a Task
   never changes Wave (Project = (Wave, Chapter)), so that suffices.
   `insert_run_in` fills a Run's Wave from the invocation and the Task check
   after it validates; the probe Home ends with 0 Task and 0 Wave disagreements.
2. **Opaque saved-review ids — keep.** H5's rule for one kind early; the
   import keeps old ids. Two id shapes now coexist in `sessions` (Task reviews
   still `task:<inv>:…`) until H5.
3. **Interrupted Op replays — reversed and fixed** (finding 1).
4. **Driver as the one failure writer — keep.** Two reasons, one shape:
   `<skill> Run failed: <error>` from the launch, `<step> Run <outcome>` from
   recovery; the operation case adds its inspection hint.
5. **One `failure_json` shape — keep.**
6. **`RunFlowStep.task_id` `None` for a saved step — works, flag it.** Live D
   and J show every Run naming the Task through declared Work. The field's
   second meaning (finding 3) is H3's to delete.
7. **Dev `lf` resolves its own Home — observed, unchanged.** The probe
   stand-in also had to pass `LF_HOME=$LF_CONTROL_HOME` to the nested decide.
8. **Pre-H2 rows without `cwd` refused on resume — acceptable for dev Homes;**
   the same refusal is what hides finding 2 for Task invocations.

## Commands and results

| Command | Result |
| --- | --- |
| `uv run python scripts/resource_envelope.py` | ok |
| `cargo build -p loopflow --bin lf` (after the fix) | ok; the probe drives `target/debug/lf` |
| `cargo nextest run -p loopflow --lib -E 'test(an_interrupted_operation_blocks)'` with the old `None` arm | FAIL: `called Result::unwrap() on an Err value: InvalidAuthority("Flow … is waiting for Run …; its completion is not recorded")` |
| Same, with the fix; plus `a_decision_belongs_to_the_current_attempt…` | 2 passed |
| Live probe (steps A–J above) | all pass; log kept in the session scratchpad |
| `cargo nextest run -p loopflow --no-fail-fast --test session_cutover_tests --test session_cli_tests --test dto_fixtures --test flow_tests` | 44 passed, 0 failed (1 slow) |
| `cargo nextest run -p loopflow --no-fail-fast --lib -E 'test(ops::) \| test(run_record) \| test(store::) \| test(controller::task) \| test(lf::commands)'` | 617 passed, 0 failed |
| `cargo fmt --all --check` | pass after formatting the new test |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `uv run python scripts/check_migrations.py` | 52 shipped migrations unchanged since v0.12.21 |
| `uv run python scripts/check_architecture.py` | only the known `wave_chapters` miss |

Note for the next pass: this machine has no `timeout`; the brief's bound is
`perl -e 'alarm shift; exec @ARGV' 900 …`. The oauth contention test did not
fail.

## Not proven

- An `--as` launch's review through the binary.
- A real driver kill mid-skill (finding 5) and mid-operation through the
  binary; the operation case is proven at the store and the Op Run row live.
- `own_flow_launch` against a copy of an installed Home.
- `lf flow route` and `lf flow blocked` through the binary (source-level only).

## Recommended next action

Proceed to H3; no further H2 implement pass. H3's brief must extend its
deletion list with the Task twins named in the duplicate table, resolve `cwd`
for Task invocations (finding 2) and drop `RunFlowStep.task_id`'s flag meaning
(finding 3). Its first failing assertion stays as the H2 ledger states it
(`claim_task_worker` inserts the worker's Run; `cursor.finish` gone from
`controller/task/mod.rs`).
