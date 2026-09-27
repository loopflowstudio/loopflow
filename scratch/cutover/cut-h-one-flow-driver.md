# Cut H — one Flow driver, one invocation owner

2026-09-27 · LOO-298 · Brief for the implement → review-slice loop that follows
the [concept review](../concept-review.md). Jack's decisions of 2026-09-27 govern:

1. A Task points at its one managed invocation. Any Flow launched with `--task X`
   names X on its invocation and every Run, reviews included, but only
   `lf task run X [--flow F]` makes an invocation THE one. "You should be able
   to run multiple flows at once on a task, but only one should be THE flow
   invocation for a task."
2. Every launch refuses when its Run row cannot be written. A Run without a row
   does not exist.
3. Bind: the CLI prints the exact target and writes; the app owns confirmation;
   binding a Session again to the Task it already has is a no-op success.

The [complete design](../data-model-one-table-per.md) still governs everything
this brief does not restate; its "One implementation" rules decide every choice
here. A slice is done when the old owner is deleted in the same commit.

## Target

```text
tasks.current_invocation_id  ─►  flow_invocations (id, task_id?, wave_id?, cwd,
                                  message, model, invocation_json, review_json
                                  = cursor, position_version, claim_json,
                                  failure_json, current_run_id, pending_session_id,
                                  state, ended_at)
                                        ▲
                                        │ invocation_id, node, iterations, attempt
                                      runs ──► sessions.current_run_id
```

- `flow_invocations` is the only cursor owner for every Flow. `flows/<id>/`
  holds `driver.lock` and nothing else.
- One `SkillExecutor` drives both a saved Flow (`lf flow`, `lf flow resume`)
  and a Task's Flow (`lf task __worker`). The Task controller adds harness,
  steer, attachment and checkpoint-before-review around it; it does not keep
  its own cursor traversal.
- Fences: `position_version` for the cursor, `claim_json` for the one live
  driver, `current_run_id` for the attempt allowed to act. `LF_FLOW_STEP`
  carries `{invocation, version}`; `lf flow decide|route|blocked` address the
  invocation, never a Task.
- Run ↔ invocation Task equality is strict again (`IS`, both nullable).
- A Session id is opaque (`session_<uuid>`) for every kind; a review's link to
  its invocation is `flow_invocations.pending_session_id` and `runs.invocation_id`.
- A Run's outcome is its row; `terminal.json` remains the settlement receipt
  that is written first. Recovery reads the row.

## Slices, in order

Each slice: write its failing test first, make it pass, delete the replaced
owner in the same commit, append to the ledger below, and stop for review-slice.
Commit with plain git; no publication, install or rebase.

### H1 — Task points at its invocation; strict equality

Draft `point_task_at_invocation` (depends on `name_tasks_on_flow_step_runs`):
`ALTER TABLE tasks ADD COLUMN current_invocation_id TEXT REFERENCES
flow_invocations(id)`; fill it from the `task_current_invocation` index; drop
that index; restore the strict `validate_run_parents_*` triggers (`f.task_id IS
NEW.task_id`); add a trigger that the pointed-at invocation names this Task.

- `flow_position_in(task)` reads through the pointer. `set_flow_position_in`,
  restart and `finish_task_flow_in` move or clear it in their transaction.
- `flow_session::reserve` and `save_flow_in` set `task_id`/`wave_id` from the
  launch's resolved Work (`declared_work`) instead of nulling the Task. Step Runs
  and review Runs of one Flow agree.
- `bind_session`: the Flow-review refusal goes; the generic "already has a Task"
  rule stays, except that the same Task returns the current record unchanged.
  `human_session::bind` prints `Binding <session> to <ISSUE> (<title>). Permanent.`
  before writing.
- Delete: `task_current_invocation`, the relaxed triggers, the review-refusal
  branch in `bind_session_runs_in`, the "carries the Wave and no Task" path.

Proof: `flow_tests::bound_flows_keep_task_context…` asserts the review Run also
names the Task; a new store test drives two invocations naming one Task while
`flow_position` returns only the pointed-at one; `session_cutover_tests` bind
twice to the same Task succeeds twice; the constructor matrix rejects a Run
naming a different Task than its invocation again.

### H2 — the saved Flow runs on the row

Draft `own_flow_launch` (depends on `point_task_at_invocation`): `cwd`, `message`,
`model` columns on `flow_invocations`. No selector strings: the launch resolves
`--task/--wave/--as` once into `task_id`/`wave_id`.

- `lf flow <name>` inserts the invocation row and drives it. `CliFlowExecutor`'s
  `checkpoint`, begin-step and finish-step become store transactions fenced by
  `position_version` (reuse `settle_task_worker_in`'s shape without the Task).
  The active boundary is `current_run_id` + the Run's position; `completed` is
  the Run's outcome; `failure` is `failure_json`; `finished` is `state`.
- `lf flow decide|route` write `review_json` verdict/route through one store
  function keyed by invocation id; `lf flow blocked` keys its Ask by
  `flow:<invocation>:<node>:<iterations>`. Both paths (Task and saved) call it.
- `flow_run::recover` reads `runs.outcome` for `current_run_id`; retry clears
  `failure_json` and `current_run_id` under the version fence.
- `lf flow resume` and `launch_driver` read the row. `driver.lock` stays.
- Refuse to start a Flow when the store cannot be written (decision 2).
- From the H1 review: `human_session::bind` prints the `Binding … Permanent.`
  line after the store write succeeds, so a refused bind prints only its refusal.
- Delete: `FlowRun`, `Boundary`, `flow_run::{read,write,update,create,
  checkpoint,capture_membership,bind_run,record_decision,record_route,
  require_active,recover,retry,begin_boundary,finish_boundary}`, `position.json`,
  `position.lock`; `RunFlowStep::of_flow` takes the row. `session_import.rs`
  gains the old `position.json` shape (~15 lines) and stays the only reader.

Proof (the concept review's first test): launch `lf --task X flow <flow with a
review>` against an isolated Home, kill the step's provider stand-in, `lf flow
resume`, complete the review, and assert `flows/<id>/position.json` never
existed, the review Session lists under X, `lf runs --task X` shows the failed
attempt, the retry and the review Run, and `lf flow decide` from a taskless step
lands in `review_json`. Also: a saved Flow with two reviews; `--wave` and `--as`
launches; `lf flow resume --retry` after a failed step.

### H3 — the Task controller drives through the same executor

- `controller/task` builds the shared executor with harness, steer and
  attachment wrapped around `run_skill`; `finish_task_flow_turn`,
  `run_task_flow_op` and the controller's own `cursor.finish` traversal go.
- `claim_task_worker` inserts the worker's Run (`published=0`) in the claim
  transaction; `bind_task_worker_run` publishes it. `task_started` and
  `chapter_task_evidence` drop the `worker_generation>0` clause; the
  `task_chapter_started` trigger and Started event go once `chat/turns.rs`
  renders "Task started" from `tasks.started_at`.
- `lf task run X --flow F` moves the pointer: `ensure_flow_position` today
  returns the existing Flow and ignores `--flow`. Selecting a different Flow
  closes the current tree as `replaced` and points the Task at the new
  invocation in one transaction (H1 review, Done when 4).
- `record_flow_verdict(task_id, …)` and `record_flow_route(task_id, …)` become
  the H2 invocation-keyed functions; `ops/task::task_verdict` resolves the
  Task's pointer then calls them.
- Delete: the controller's parallel traversal, Task-keyed verdict/route store
  functions, the Started event writer.

Proof: the 29 `controller::task` tests; `driver_runs_fresh_slice_turns_until_
the_flow_finishes` unchanged in meaning; a Task step through the real binary
(`flow_tests` or `session_cli_tests`) whose Run row appears at claim with
`published=0` and lists as started before launch.

### H4 — a Run without a row does not exist

- Every launch path (`begin_run_capture`, Ask, saved Flow, Task step, Wave
  runner, PR landing, `lf ops`) fails with the store error before a provider
  starts. The "not recorded and will not list" warnings go.
- `lf runs <prefix>`, `--parent`, `lf replay` and `find_session` resolve a
  prefix by `SELECT id FROM runs WHERE id LIKE ?`; ambiguous prefixes name the
  candidates. `resolve_manifest` and `record_dirs` as lookup go; the record
  directory is `record_dir(home, id)`.
- The `prepared` marker and `run_is_prepared` go: a reserved Run is
  `published=0`; the launcher claims it with `UPDATE runs SET published=1 WHERE
  id=? AND published=0` and starts nothing when that changes no row.
- `recover_task_decision`, `reconcile_reserved_manifest` and `read_run_snapshot`
  for state read `runs.outcome`; `terminal.json` stays the receipt written first.

Proof: with `LF_HOME` pointing at a read-only directory, `lf : "x"`, `lf ask`,
`lf flow <name>` and `lf --task X code` exit nonzero, print the store error and
start no provider stand-in; `lf runs run_ab` resolves by prefix from rows only;
two launchers racing one prepared Run start one provider.

### H5 — opaque Session ids

- Every new Session is `session_<uuid>`. `review_id`, `flow_session::{session_id,
  token}` and every `strip_prefix`/`split` on a Session id go; `owned_target`
  dispatches on `sessions.kind` and joins the invocation through
  `pending_session_id`.
- Draft `opaque_session_ids`: rewrite existing structured ids to
  `session_<uuid>` and record the mapping in an `imported_session_ids` table the
  import module and desktop pane migration read once.
- Swift: `SessionRecord.id` is already an opaque string; only the pane-key
  migration and fixtures change. `SessionTitleSource.unavailable` goes with the
  fixture update (nothing produces it).

Proof: a Task review and a saved Flow review both list, open, rename and
complete through the same `owned_target` path; an old structured id resolves
once through the mapping; the DTO fixtures round-trip in Rust and Swift.

## Done when (whole cut)

1. `rg "position\.json|FlowRun\b|StepToken|resolve_manifest|run_is_prepared|
   prepared_run_id|task_current_invocation|not recorded and will not list"
   rust/loopflow/src` returns only `session_import.rs` and migration SQL.
2. One `impl SkillExecutor` in production; `controller/task/mod.rs` has no
   `cursor.finish` call of its own.
3. `flow_invocations.task_id IS runs.task_id` for every Run with an invocation;
   `tasks.current_invocation_id` names an invocation whose `task_id` is that Task.
4. `lf --task X flow F` lists its step and review Runs under X and leaves
   `tasks.current_invocation_id` unchanged; `lf task run X --flow F` moves it.
5. Unwritable store: every launch refuses; no warning path remains.
6. Same-Task bind is idempotent; the CLI prints the exact target.
7. `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`,
   `scripts/check_migrations.py`, `scripts/check_architecture.py` (the
   `wave_chapters` gap remains the only known miss) pass; the affected suites in
   the proof commands pass once at the end.
8. Net production lines for the cut are reported per file, before the first
   test module, against `e7f3e22fd`.

## Open for Jack, not blocking

Whether `lf session bind` may assign a whole taskless invocation (its row and
every Run) to a Task. Today the strict triggers make it unspellable; the
refusal reads "Run and Invocation nullable Tasks disagree". H1 review.

## Forbidden

Everything in the complete design's list, plus: selector strings on
`flow_invocations`; a `position.json` write anywhere; a second executor; a
Legacy/New branch on Task presence in any Flow operation; a Run that runs
without a row; an id that a reader parses for structure.

## Proof commands

Preflight once per pass: `uv run python scripts/resource_envelope.py`. Clear
`LF_*` and `LOOPFLOW_*` from the environment for every cargo command; `-j 4`,
`nice +10`. Tests select a private `LF_HOME` and clear `LF_CONTROL_HOME` and
`LF_CONTROL_DB_PATH` (TESTING.md).

```sh
cargo nextest run -p loopflow --no-fail-fast --test session_cutover_tests --test session_cli_tests --test dto_fixtures --test flow_tests
cargo nextest run -p loopflow --no-fail-fast --lib -E 'test(ops::) | test(run_record) | test(store::) | test(controller::task) | test(lf::commands)'
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings
uv run python scripts/check_migrations.py
```

Known flaky, outside this Task: `ops::pm::oauth_tests::pm_read_linear_oauth_
sqlite_contention_has_bounded_failure_and_recovers`.

## Ledger

Append one entry per implement or review-slice pass: date, commit, slice,
what passed and failed with the command, what was deleted with line counts,
decisions made on Jack's behalf (also in `../questions.md`), and the next
failing assertion.

### 2026-09-27 · H1 — Task points at its invocation; strict equality

Implement pass. Code commit `4820e1f86`; this entry follows it.

**Owners now.** `tasks.current_invocation_id` is the one selector of a Task's
Flow; every store write that advanced "the current invocation naming Task X"
(`flow_position_in`, claim, reclaim, bind worker Run, block, release, settle,
finish, verdict, route, human completion, restart, reopen, chapter evidence)
selects `id=(SELECT current_invocation_id FROM tasks WHERE id=?1)`
(`durable::TASK_INVOCATION`). `set_flow_position` at version 0 inserts the
invocation and points the Task at it, refusing a Task that already points
somewhere; finish, human completion, restart and reopen clear the pointer in
their transaction. A saved Flow launched with `--task X` writes `task_id=X` on
its invocation (`save_flow_in`, from the launch's `declared_work`) and on its
step and review Runs; `insert_run_in` and the `validate_run_parents_*`
triggers require `f.task_id IS NEW.task_id` again. A review is the Task's own
when its Run's invocation is the Task's pointer (`human_session::managed_review`);
`waiting_flow`, `end_flow`, `replace_session_run` and `complete_session` use the
pointer for the same distinction. Bind: same Task returns the record unchanged;
`lf session bind` prints `Binding <session> to <ISSUE> (<title>). Permanent.`
on stderr before writing.

**Commands and results.** Ambient `LF_*` cleared; `-j 4`; `nice -n 10`;
`scripts/resource_envelope.py` PASS (86.4 GiB free / 64.0 GiB floor).

| Command | Result |
| --- | --- |
| `cargo nextest run -p loopflow --no-fail-fast --lib --test flow_tests --test session_cutover_tests -E 'test(a_task_points_at) \| test(run_constructor_infers) \| test(bound_flows_keep) \| test(binding_an_orphan)'` before production edits | **4 failed**: `a_task_points_at_one_invocation_while_other_flows_name_it` — `UNIQUE constraint failed: flow_invocations.task_id`; `run_constructor_infers_ancestors_and_rejects_conflicts_atomically` — `assertion failed: insert_run_in(&tx, requested).is_err()` (a Task-naming Run in a taskless invocation was accepted); `binding_an_orphan_session_starts_its_task_once` — `Session session_195d… already has Task INF-123; a Run's Task never changes` on the same-Task rebind; `bound_flows_keep_task_context…` — review `work` was `{"kind":"wave"}`, expected `{"kind":"task"}` |
| Same, after | **4 passed** |
| `cargo nextest run -p loopflow --no-fail-fast --test session_cutover_tests --test session_cli_tests --test dto_fixtures --test flow_tests` | 40 passed, **1 failed**: `flow_tests::observing_and_preparing_a_task_are_not_execution` at `starts() == 0` — the review Run of `lf --task INF-123 flow review-first` now names the Task, and the first Run naming a Task sets `started_at`. Rewritten (below); rerun alone: passed. All 41 pass |
| `cargo nextest run -p loopflow --no-fail-fast --lib -E 'test(ops::) \| test(run_record) \| test(store::) \| test(controller::task) \| test(lf::commands)'` (plus the rewritten flow test) | **620 passed, 0 failed** |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `uv run python scripts/check_migrations.py` | 52 shipped migrations unchanged since v0.12.21 |
| `uv run python scripts/check_architecture.py` | only the known `wave_chapters` miss |

The oauth contention test did not fail in this pass.

**Draft.** `point_task_at_invocation` (depends on `name_tasks_on_flow_step_runs`,
42 lines): `tasks.current_invocation_id` filled from the current invocation per
Task, `DROP INDEX task_current_invocation`, `validate_task_invocation` (the
pointed-at invocation names this Task), strict `validate_run_parents_insert` /
`_update`. No released migration changed.

**Deleted.**

| Item | Where |
| --- | --- |
| `task_current_invocation` partial unique index | dropped by the draft |
| Relaxed `validate_run_parents_*` (`f.task_id IS NULL OR …`) | replaced by the draft |
| "A Flow launched directly for a Task names no Task; its Runs may" branch of `insert_run_in` | `store/sqlite/runs.rs` |
| Flow-review refusal in `bind_session_runs_in` | `store/sqlite/runs.rs` |
| "its Runs carry the Work's Wave and no Task" in `flow_session::reserve` | `ops/flow_session.rs` |
| Selector re-resolution and Wave-only Run in `session_import::flow_review` (now `declared_work`) | `ops/session_import.rs` |
| `(FlowReview, Some(task))` kind-and-Task dispatch in `owned_target`; the `(Some, None)` / `(Some, Some)` split in `surface` | `ops/human_session.rs` |
| `task_id IS NULL` in `waiting_flow`, `end_flow`; `r.invocation_id IS NOT NULL AND r.task_id IS NOT NULL` in `replace_session_run`, `complete_session` | `store/sqlite/sessions.rs` |
| `ON CONFLICT DO NOTHING` on the unique index as the "Task already has a Flow" refusal | `set_flow_position_in`, now the pointer update |

**Production lines** (before the first `#[cfg(test)] mod`, against `7eb3046a8`):
`store/sqlite/durable.rs` 1,530 → 1,569 (+39); `ops/human_session.rs` 1,872 →
1,894 (+22); `store/sqlite/children.rs` 2,211 → 2,225 (+14); `store/sqlite/
chapters.rs` 131 → 139 (+8); `store/sqlite/sessions.rs` 598 → 605 (+7);
`lf/commands/flow.rs` 612 → 613 (+1); `store/sessions.rs` 173 → 174 (+1);
`ops/flow_session.rs` 204 → 203 (−1); `store/sqlite/runs.rs` 425 → 419 (−6);
`ops/session_import.rs` 585 → 573 (−12). **Net +73.** The growth is the
`&format!` wrapping of eleven SQL statements in `durable.rs` and the pointer
clears; the draft SQL is not counted. Docs: `docs/architecture/data.md`,
`docs/architecture-reference.md` (invocation-structure row). Swift and
`tests/fixtures/dto` untouched.

**Tests.** New `store::…::a_task_points_at_one_invocation_while_other_flows_name_it`.
Constructor matrix: the taskless-invocation Task-naming Run moved back to
rejected. `flow_tests::bound_flows_keep_task_context…` asserts the review
`work` is the Task and `lf runs --task INF-123` lists the review Run.
`session_cutover_tests::binding_an_orphan…` binds INF-123 twice: the second
returns the same record and prints the target. `flow_tests::observing_and_
preparing_a_task_are_not_execution` asserted that a `--task` Flow's review left
`started_at` NULL (Cut 3's Wave-only review); it now asserts the Task starts at
the review's reservation. The unreadable-capture store test points its broken
Task at the broken invocation.

**Decisions made here** (also in `../questions.md`):

1. No `wave_id` column on `flow_invocations` in H1; H2's `own_flow_launch`
   owns the launch facts. Runs carry the Wave.
2. A review Run naming the Task starts it (store rule "reservation counts"),
   so `lf --task X flow <review-first>` starts X when it parks at the review.
3. The bind line goes to stderr, on the no-op too; `--json` stdout stays clean.
4. `owned_target` refuses a review whose invocation is neither the Task's
   pointer nor a waiting saved Flow with "Session … is no longer waiting".
5. The Rust `Task` struct does not carry `current_invocation_id`; the store is
   its only reader.
6. Import of an old saved-Flow review resolves its Work through
   `declared_work`, so it names the Task too.

**Consequence to keep visible.** Binding a *taskless* Flow's review to a Task is
now refused by the strict trigger (`Run and Invocation nullable Tasks
disagree`) rather than by a bind-specific message: the Run cannot take a Task
its invocation does not name, and the invocation cannot take one its Runs do
not. Whether bind should assign a whole taskless invocation is for Jack.

**Not proven.** `lf session bind` on a review through the binary; a `--wave`
or `--as` launch's review under the strict trigger beyond the existing
`session_cutover_tests` Wave-only saved Flow; the draft against a Home copy.

**Next failing assertion (H2).** Launch `lf --task X flow <flow with a review>`
against an isolated Home and assert `flows/<id>/position.json` never existed:
fails at `flow_run::create` → `write` (`lf/commands/flow.rs`), which writes the
file at launch.

### 2026-09-27 · H1 review-slice

Review of `4820e1f86`/`0fdda5066`; full matrix in [cut-h-review.md](cut-h-review.md).
Every H1 claim passes; the Task-about review, the held pointer under a third
`lf --task INF-123 flow review-first`, the same-Task bind no-op with its
`Binding … Permanent.` line and the taskless-review refusal were driven through
the compiled `lf` against a private Home. One gap fixed: the draft left pre-H1
rows (Cut F step Runs, Cut 3 Wave-only reviews) disagreeing with their taskless
invocation under the strict trigger; `point_task_at_invocation` now backfills a
taskless invocation whose Runs name one Task, proven by
`migrations::tests::pointing_tasks_at_invocations_names_the_task_on_earlier_task_flows`
(failed `1 != 0` before, passes after). Suites: store lib filter plus the H1
flow/cutover tests 159 passed; fmt, clippy, `check_migrations` (52 unchanged)
pass. For Jack: bind line prints before a refused write; whether bind may assign
a taskless invocation; Done-when 4's "`task run --flow` moves the pointer" has
no owner (`ensure_flow_position` ignores `--flow` on a live Flow; restart is the
mover). Verdict: proceed to H2.
