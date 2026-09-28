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

Jack's Linear comment `c5cd2dd2` (2026-09-27) keeps three items in LOO-298:
repository Chapters and Project-owned Flows (H7), the saved Flow cursor on the
row (H2, done), and every remaining redundant pair (H3–H6). The same day he
decided to delete `lf wave serve` and every notion of a Wave listener or
resident, lfd included, and to add that deletion to this branch aggressively:
[Cut I](cut-i-delete-listeners.md). Order after H2: H3, I, H4, H5, H6, H7.
`cut-h-handoff.md` and `cut-h-wip.patch` were the stopped worker's partial
Cut H; H2 supersedes them and they are deleted.

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
- One row type and one fence set. `FlowPosition`, `read_flow_position_row`,
  `flow_position_in` and `decode_flow` become one `FlowInvocation` read;
  `set_flow_position_in`'s UPDATE, `settle_task_worker_in`, `block_task_flow_in`
  and `release_task_worker_in` become the H2 `write_cursor_in`, `fail_flow` and
  `retry_flow` with the claim as an extra predicate; `recover_task_decision`
  (terminal.json) becomes `settle_attempt_in` (runs.outcome);
  `run_task_op_boundary` becomes the shared `run_op`; the `task:` blocker key
  becomes the `flow:` key. The H2 review lists the pairs with line refs.
- `cwd`: a Task invocation stores NULL and the one SELECT reads
  `COALESCE(f.cwd, t.worktree)`; a trigger refuses a stored cwd on a Task
  invocation. `lf flow resume <id>` then works on a Task's invocation too.
- `RunFlowStep.task_id` loses its "stored at claim" meaning; the row says so.
- Delete: the controller's parallel traversal, the Task-keyed store functions
  above, the `None`-token Task fallback in `lf flow decide|route|blocked`, the
  Started event writer.

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

### H6 — remaining pairs and the wire

- `WorkCatalog` (`lf/commands/work_catalog.rs`) goes; `lf activity` filters PR,
  Steer and creation entries with the same SQL Work filter Runs use.
- String subjects go: `RunSpec.subjects`, `RunManifest.subjects`,
  `PromptBuild.subjects`, `WorkBinding.subjects`, `human_session::work_selector`,
  and `ActiveRun.subjects` on the wire. DTOs carry typed `task_id`/`wave_id`
  (plus names from the row's join); Swift and `tests/fixtures/dto` move in the
  same commit. The prompt's Work context renders from the typed Work.
- `lf session list --task <issue>` exists (the demo names it).
- The `task_flow_positions` migration test keeps its historical subject; note
  it as history in the ledger, do not delete a released migration's proof.
- `historical_session_run_id` / `historical_ready_summary`: Jack's open
  decision; leave until he answers.

Proof: DTO fixture round-trips in Rust and Swift (`swift test`); `lf activity
--task X` lists a PR entry and a Run through one filter; `rg subjects
rust/loopflow/src swift` returns only the import.

### H7 — repository Chapters and Project-owned Flows

Target per [docs/waves.md](../../docs/waves.md#the-planning-model) and the
complete design's "Repository Chapter rotation": a Chapter is the repository's
clock; `chapters(id, repository, status, activated_at, …)` with one current per
repository; `projects.chapter_id` and `UNIQUE(wave_id, chapter_id)`; a Project
owns its default Flow template (`projects.flow`). `wave_chapters` and the
per-Wave `Chapter.wave_id` go; `ops/chapter.rs` keeps classification, provider
reconciliation and frozen evidence but rotates every Wave in one transaction
(`lf wave new-chapter --plan chapter.json`, preview, retry by Chapter id).
`lf task run` defaults to the Project's Flow; `--flow` overrides and moves the
pointer (H3). `check_architecture.py` stops reporting the `wave_chapters` gap.

Proof: the complete design's Done when 4 (two Waves incl. an empty plan,
concurrent start vs retirement, lost provider response, retry after
activation, one current repo Chapter, frozen predecessor evidence); `lf wave
history --wave X` and `lf status X --chapter <id>` through the binary.

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

This branch carries drafts (LOO-321 incident): never run its binary against
the installed Home and never promote it before it lands. Every proof uses its
own `LF_HOME` with `LF_CONTROL_HOME`/`LF_CONTROL_DB_PATH` unset or pointed at
that same private Home.

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

### 2026-09-27 · H2 — the saved Flow runs on the row

Implement pass. Code and this entry in one commit.

**Owners now.** `flow_invocations` is the only cursor owner of a saved Flow.
`lf flow <name>` inserts the row (`SqliteStore::create_flow`, draft
`own_flow_launch`: `cwd`, `message`, `model`, `wave_id`; selectors resolve at
launch into `task_id`/`wave_id`) and refuses to start when the store cannot be
written. `CliFlowExecutor` reads the row before every step (`recover_flow`),
checkpoints under the `position_version` fence (`checkpoint_flow`: a decision
or route recorded on the same position survives, a moved position releases
`current_run_id` and `pending_session_id`), and the driver writes a failure
once (`fail_flow`). The active boundary is `current_run_id` plus the Run's
position; a completed attempt is the step's completion; a failed or interrupted
Run becomes `failure_json` (`TaskFlowBlocker` shape, same as a Task's); a live
Run keeps the Flow waiting. `lf flow resume --retry` clears the failure and the
attempt under the fence (`retry_flow`). `lf flow decide|route` write
`review_json` through `record_verdict_in`/`record_route_in`, keyed by
invocation id and fenced by `current_run_id`; the Task-keyed
`record_flow_verdict`/`record_flow_route` now check the claim and call the
same writer. `lf flow blocked` keys its Ask by
`flow:<invocation>:<node>:<iterations>`. `LF_FLOW_STEP` carries
`ActiveStep { invocation, version }`. A saved review's Session id is
`session_<uuid>`; the row links it through `pending_session_id`, and
`LF_HUMAN_SESSION` carries `{"kind":"standalone_flow","id":<session>}`.
`flows/<id>/` holds `driver.lock` and nothing else. `human_session::bind`
prints `Binding … Permanent.` after the store write.

**Commands and results.** Ambient `LF_*`/`LOOPFLOW_*` cleared; `-j 4`;
`nice -n 10`; `scripts/resource_envelope.py` PASS (86.2 GiB free / 64.0 GiB
floor).

| Command | Result |
| --- | --- |
| `cargo nextest run -p loopflow --test session_cutover_tests -E 'test(a_task_flow_runs_on_its_row) \| test(a_taskless_step_records)'` before production edits | **2 failed**: `a_task_flow_runs_on_its_row_through_failure_retry_and_review` at `flows/<id>/position.json` exists ("the row is the only cursor owner"); `a_taskless_step_records_its_decision_on_the_invocation` at `Flow … blocked: repeat at step 1 requires a decision` (the stand-in's `lf flow decide` reached no store) |
| Same, after | **2 passed** |
| `cargo nextest run -p loopflow --no-fail-fast --test session_cutover_tests --test session_cli_tests --test dto_fixtures --test flow_tests` | **43 passed, 0 failed** (1 slow) |
| `cargo nextest run -p loopflow --no-fail-fast --lib -E 'test(ops::) \| test(run_record) \| test(store::) \| test(controller::task) \| test(lf::commands)'` | 616 run: 615 passed, **1 failed** — `review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors` seeded a Task invocation with no Wave by raw SQL; the new `validate_invocation_wave_insert` trigger refused it. Fixture given its Wave; rerun with the flows and pointer tests: 3 passed |
| `cargo nextest run -p loopflow --test session_cutover_tests -E 'test(a_flow_refuses_to_start)'` | 1 passed (added after the suites; the locked-store refusal names `unable to open database file` and starts no provider) |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `uv run python scripts/check_migrations.py` | 52 shipped migrations unchanged since v0.12.21 |
| `uv run python scripts/check_architecture.py` | only the known `wave_chapters` miss |

The oauth contention test did not fail in this pass.

**Draft.** `own_flow_launch` (depends on `point_task_at_invocation`, 35 lines):
`cwd`, `message`, `model`, `wave_id` on `flow_invocations`; Wave filled from the
Task for Task invocations and from a taskless Flow's Runs where they agree;
`validate_invocation_wave_insert`/`_update` refuse an invocation whose Task and
Wave disagree. `set_flow_position_in` fills `wave_id` from the Task;
`insert_run_in` fills a Run's Wave from its invocation and refuses a different
one. No released migration changed.

**Deleted.**

| Item | Where |
| --- | --- |
| `FlowRun`, `Boundary`, `StepToken`, `directory`, `lock`, `read`, `write`, `update`, `create`, `checkpoint`, `bind_run`, `record_decision`, `record_route`, `require_active`, `recover`, `retry`, `begin_boundary`, `finish_boundary`, `position.json`, `position.lock`, five file-semantics tests | `ops/flow_run.rs` |
| `session_id`, `token`, `waiting` (file reader), selector-resolving `declared_work` | `ops/flow_session.rs` |
| `save_flow`, `save_flow_in` (the UPSERT copy of the cursor) | `store/sqlite/sessions.rs`, `store/sessions.rs` |
| Task-path duplicates of the repeat, conflict and router checks in `record_flow_verdict`/`record_flow_route` | `store/sqlite/durable.rs` |
| `flow_run::bind_run` call after capture | `lf/commands/run.rs` |
| "this review's Session is not recorded" warning; `save_flow` before each step; `LOOPFLOW_FLOW_NAME`-era `record.wave/task/as_work` relaunch | `lf/commands/flow.rs` |
| `HumanSessionToken::StandaloneFlow { token: StepToken }` | `ops/human_session.rs` |

**Production lines** (before the first `#[cfg(test)] mod`, against `d0a5469f5`):
`ops/flow_run.rs` 465 → 88 (−377); `ops/flow_session.rs` 203 → 161 (−42);
`store/sqlite/sessions.rs` 605 → 564 (−41); `store/sqlite/durable.rs` 1,569 →
1,528 (−41); `store/sessions.rs` 174 → 157 (−17); `ops/human_session.rs`
1,894 → 1,885 (−9); `store/sqlite/runs.rs` 419 → 417 (−2); `lf/commands/run.rs`
1,224 → 1,223 (−1); `lf/commands/flow.rs` 613 → 717 (+104); `ops/session_import.rs`
573 → 623 (+50, the old `position.json` shape and its selector resolution);
`run_record.rs` 2,255 → 2,260 (+5); `durable.rs` 359 → 433 (+74,
`FlowInvocation`/`FlowAttempt`); `store/sqlite/flows.rs` new 582;
`store/flows.rs` new 100; `store/mod.rs`, `store/sqlite.rs` +1 each.
**Net +387**, above the concept review's estimate: the store gained the
sixteen-column row decode, the checkpoint merge, recover, retry, the two
fenced writers and the blocker key, each with its fence spelled out. Docs:
`docs/architecture/data.md`, `docs/architecture/planning.md`,
`docs/architecture-reference.md` (`flow_invocations` row), `TESTING.md`.
Swift and `tests/fixtures/dto` untouched.

**Tests.** New `session_cutover_tests::a_task_flow_runs_on_its_row_through_
failure_retry_and_review` (the concept review's first test: provider stand-in
exits 7 on the first launch, `--retry`, review under INF-123, `lf runs --task`
lists the failed attempt, the retry and the review Run, `position.json` never
exists, state `completed`), `a_taskless_step_records_its_decision_on_the_
invocation` (the stand-in runs `lf flow decide advance` inside the deciding
step; the Flow takes the edge and completes), `a_flow_refuses_to_start_without_
its_row`, `store::sqlite::flows::tests::a_decision_belongs_to_the_current_
attempt_and_recovery_reads_its_outcome` (fences: wrong Run, wrong version,
conflicting decision, unsettled Run, failed Run → blocker, retry, second
attempt, moved position). `flow_session::nested_review_is_one_session…`
rewritten on the row. `flow_tests::bound_flows_keep…` finds the review by
`kind`; the cutover `inside` helper carries the Session id.

**Decisions made here** (also in `../questions.md`):

1. `own_flow_launch` adds `wave_id` with the three launch columns; a `--wave`
   Flow's Wave has to survive resume, and the H1 ledger reserved it for H2.
2. Saved review Sessions are `session_<uuid>` now; the old `flow:<inv>:<boundary>`
   shape lives only in the import. The `LF_HUMAN_SESSION` token carries the
   Session id.
3. An Op step has no Run, so an Op interrupted mid-way replays on resume; the
   old "interrupted before a completion receipt" guard (also for a skill step
   that died before its Run row existed) is gone.
4. The driver is the one failure writer: `"<skill> Run failed: <error>"` from
   the launch, `"<step> Run <outcome>"` from recovery.
5. `failure_json` has one shape (`TaskFlowBlocker`) for every invocation.
6. `RunFlowStep.task_id` stays `None` for a saved Flow's step: `record_row`
   reads a Task there as "managed Task step, stored at claim".
7. A development `lf` inside an agent resolves its own worktree Home; the test
   stand-in passes the fixture's Home to its nested `lf flow decide`.
8. Pre-H2 saved Flows on a dev Home (rows without `cwd`) are refused by
   `lf flow resume` with "has no launch record on its row".

**Not proven.** A saved Flow with two reviews; an `--as` launch's review
through the binary beyond `flow_tests`; a real driver crash (the failed attempt
is a provider exit, not a kill); `own_flow_launch` against a Home copy; a
`--wave` review launch (`flow_session::launch` passes the Wave id, which
`resolve_explicit_wave` accepts by id or name).

**Next failing assertion (H3).** `claim_task_worker` inserts the worker's Run
in the claim transaction: after `store.claim_task_worker(...)`,
`SELECT count(*) FROM runs WHERE invocation_id=?1 AND published=0` is 0; and
`rg "cursor.finish" rust/loopflow/src/controller/task/mod.rs` still returns
`finish_task_flow_turn`.

### 2026-09-27 · H2 review-slice

Review of `4bed6f562`; full matrix in [cut-h-review.md](cut-h-review.md#cut-h-review--h2-the-saved-flow-runs-on-the-row).
Every H2 claim passes through the compiled `lf` against a private Home, plus
the two paths the implement pass left unproven (a saved Flow with two reviews;
a `--wave` launch's review). One decision reversed and fixed: an operation
interrupted mid-way replayed on resume. `run_op` now records the operation as
a Run the way the Task path already does (`harness loopflow`, `surface
operation`, `skill None`), and `settle_attempt_in` ends a receipt-less
operation Run as `interrupted` and blocks with `op: <name> Run interrupted
before its completion receipt; inspect its effect before retrying`; `--retry`
is the explicit replay. Proof: `store::sqlite::flows::tests::an_interrupted_
operation_blocks_for_inspection_instead_of_replaying` (failed with the old arm:
`waiting for Run …; its completion is not recorded`; passes), the Op Run row
live on a `--wave op-then-review` launch. Production: `lf/commands/flow.rs`
717 → 753, `store/sqlite/flows.rs` 582 → 597; cut net +438. Suites: integration
44 passed, lib 617 passed; fmt, clippy, `check_migrations` (52 unchanged),
`check_architecture` (only `wave_chapters`) pass. Duplicate-logic finding: the
saved fence is spelled next to the Task's (decode, insert, cursor write, block,
release, attempt authority, settle, blocker key, Op Run); H3's deletion list
must name `settle_task_worker_in`, `block_task_flow_in`,
`release_task_worker_in`, `set_flow_position_in`'s UPDATE branch,
`FlowPosition`/`read_flow_position_row`, `recover_task_decision`,
`run_task_op_boundary` and the `task:` blocker key. Two Task-presence branches
for H3 beyond the decide/route fallback: `cwd` lives on `tasks.worktree` for a
Task invocation and on the row for a saved one (`decode_flow` refuses `cwd IS
NULL`); `RunFlowStep.task_id` doubles as the "stored at claim" flag in
`record_row`. Verdict: proceed to H3.

### 2026-09-27 · Rebase onto main 90a232aaf

Reconciliation pass after rebasing the 62 commits onto `origin/main`
(`90a232aaf`, #1301 "Keep Task agent choices and make stalled work visible",
plus #1297 and the v0.12.22 release). Conflicts were resolved by keeping our
side per hunk; the 19 dropped main hunks (controller/task, human_session,
lf/commands/flow, flow_session) were then re-implemented on the row model. No
Legacy/New branch; `prepare_flow_run` does not return.

**Main's behaviors, where they live now.**

| #1301 behavior | On main | Here |
| --- | --- | --- |
| Task keeps `-m` (`tasks.agent`) and it overrides each step's frontmatter | `resolve_task_agent`, `select_task_agent`, `prepare_task_flow_step`, `drive_task` | merged clean; unchanged |
| The review Run takes the Task's agent | `prepare_flow_run` writing a prepared manifest | `human_session::select_review_agent`: resolves the agent and writes `provider`/`model` onto the review's reserved Run (`runs.published=0`) through `Store::retarget_unpublished_run`; called by `prepare` before `launch_flow`, and by `serve_flow_locked` after `reserve_review_run`, which launches `lf --tui --model <agent>`. `publish_review_run` still records what launched |
| A changed Task agent re-targets an unstarted review | `retarget_prepared_task_review`: new prepared Run, `carry_session_name`, manifest read | `retarget_prepared_task_review` = `select_review_agent` on the Task's human position; the same Run row changes in place, a published Run keeps its provider. No manifest, no Session rename |
| A decision Run without a verdict blocks with its Run and opens a keyed unblock Ask | `task_unblock` writing an `AskSessionRecord` file keyed `task:<task>:<inv>:<boundary>` | `task_unblock` stores an Ask Session row (`store_ask`, shared with `reserve_ask`) whose Run names the Task, its Wave, the worktree, `skill=unblock`, the Task's agent and `caller_run_id` = the failed Run; keyed `ask_once_<sha256(flow:<invocation>:<node>:<iterations>)>` — the H2 `flow_blocker_key`, now `QueuedInvocation::blocker_key` / `FlowPosition::blocker_key`, used by `lf flow blocked` on both paths and by the store's `flow_blocker_key`. `launch_keyed_ask` is the one reuse-or-store-then-launch path for `ask_once` and `task_unblock`; `launch_ask` takes the caller from the Run row |
| Blocked shown while the decision Run waits on its Ask | `task_waiting_unblock` reading the record file | `task_waiting_unblock(store, position)` reads the keyed Session row: open and `caller_run_id` = the claimed worker Run |
| `TaskFlowBlocker.run_id` | `block_task_flow_in` fills it | merged; `TaskFlowBlocker::now` carries `run_id: None` and the saved Flow's `settle_attempt_in` fills the failed attempt's Run |
| Stalled (5 min without event or CPU progress) | `run_record/activity.rs`, worker `activity_tick`, `task_execution` | merged clean; unchanged |
| `ensure_flow_position` reopens the unblock Ask of a blocked decision | hunk dropped | restored |
| DTO fixtures `task_execution_stalled`, `task_flow_stalled`; Swift `stalled` | | merged clean |

**Commands and results.** Ambient `LF_*`/`LOOPFLOW_*` cleared; `-j 4`;
`nice -n 10`; `scripts/resource_envelope.py` PASS (77.8 GiB free / 64.0 GiB
floor).

| Command | Result |
| --- | --- |
| `cargo check -p loopflow --all-targets` before | 24 errors (`task_unblock`, `task_unblock_key`, `task_waiting_unblock`, `retarget_prepared_task_review`, `prepare_flow_run`, fixtures, `RunSpec.work`, `TaskFlowBlocker.run_id`) |
| `cargo check -p loopflow --all-targets` after | pass |
| `cargo nextest run … --test session_cutover_tests --test session_cli_tests --test dto_fixtures --test flow_tests --test status_tests --test task_github_cache_tests --test task_initialization_tests` first pass | 61 run: 60 passed, **1 failed** — `session_cli_tests::boundary_names_follow_run_ids_and_replacement_runs` read the fixture provider's evidence file between the shell creating it and writing the id (passes alone); the test now waits for the id |
| Same, second pass | **61 passed, 0 failed** (1 slow) |
| `cargo nextest run … --lib -E 'test(ops::) \| test(run_record) \| test(store::) \| test(controller::task) \| test(lf::commands)'` first pass | 628 run: 627 passed, **1 failed** — `task_decision_driver_failures_open_one_unblock_and_reassess_feedback` expected main's "review evidence cannot be empty"; the H2 shared writer said "decision requires evidence or direction". One message now, main's |
| Same, second pass | **628 passed, 0 failed** |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `uv run python scripts/check_migrations.py` | 53 shipped migrations unchanged since v0.12.22; main's `task_agent` draft joins ours with no chain change |
| `uv run python scripts/check_architecture.py` | only the known `wave_chapters` miss |

The oauth contention test did not fail in this pass.

**Tests kept, adapted, deleted.** Main's controller tests all kept:
`task_decision_recovery_requires_the_original_successful_run`,
`task_decision_live_unblock_returns_feedback_without_navigation`,
`task_decision_driver_failures_open_one_unblock_and_reassess_feedback`,
`task_decision_public_resume_preserves_feedback_after_adoption_refusal`,
`task_agent_driver_runs_fresh_slice_turns_until_the_flow_finishes` run
unchanged on rows (their `RunSpec` gains our `work: None`; the fixture is
`human_task_fixture_at(database)` with our `_with_database` wrapper).
`task_agent_changed_during_worker_applies_to_next_review` asserts the review
Run row's provider/model instead of a prepared manifest.
`task_agent_survives_refresh_and_selects_autonomous_and_human_steps` drives
`select_review_agent` and `retarget_prepared_task_review` on the row: same Run
id, `published=0`, provider changes; a published Run keeps its provider.
`task_initialization_tests::task_live_unblock_status_and_desktop_share_exact_
boundary_and_recovery` writes Ask Session rows (stale caller → running;
`replace_session_run` onto the deciding Run → blocked in `lf task status` and
`roadmap`; ready+complete → running; other boundaries → running) instead of
`human-sessions/*.json`. Nothing deleted: main's
`ordinary_flow_parks_at_the_same_durable_review_after_recovery` and the
`flow_session` `WaitingExecutor` tests were already replaced by H2's row tests
in the rebase itself (they drove `flow_run::{create,read,update}` and
`position.json`).

**Production lines** (before the first test module, against `f74dac0d8`):
`ops/human_session.rs` 1,883 → 2,064 (+181); `engine/invocation.rs` 182 → 198
(+16); `store/sqlite/sessions.rs` 564 → 580 (+16); `store/sessions.rs` 157 →
172 (+15); `controller/task/mod.rs` 1,129 → 1,137 (+8); `durable.rs` 209 → 214
(+5); `ops/task_execution.rs` 185 → 182 (−3); `lf/commands/flow.rs` 753 → 748
(−5); `store/sqlite/flows.rs` 597 → 591 (−6). **Net +227**, the Task unblock
Session and the review agent selection. No Swift, DTO fixture or migration
change beyond what main brought.

**Decisions made here** (also in `../questions.md`):

1. A Task decision's unblock Ask is keyed by the H2 `flow:` key, not main's
   `task:` key, so `lf flow blocked` from inside the Run and the driver's
   recovery open one Session.
2. `launch_ask` takes the asking Run from the Ask Run's `caller_run_id`, not
   from the launching process's `LF_RUN_ID`; the Run directory is passed only
   when it resolves.
3. The review agent is chosen where the review Run is reserved and read back
   at launch (`--model`); a published Run is never re-targeted.
4. `settle_attempt_in` fills `failure.run_id` for a saved Flow too; a saved
   Flow's failed decision does not open an unblock Ask (not in #1301 either).

**Not proven.** A Task review launched through the real `lf session serve-flow`
with `--model`; the unblock Ask through tmux (the unit stub records launches).

## 2026-09-27 · H3 · one Task/saved Flow executor

H3 is implemented locally for the supervisor's rebase, then review-slice. Jack
requested Codex take over the stopped Claude worker and finish only H3. No
publication, rebase, installation, installed-Home access by a branch binary,
Cut I or H4 was performed. H7 remains governed by `../chapters.md`.

Before editing, Codex saved all 30 stopped-worker paths, their hashes and the
binary diff in ignored `.lf/tmp/h3-codex-recovery/` (`stopped-worker.tar.gz`,
`source-hashes.json`, `working.patch`). H3's base is `097fc32a99a284eb2ef3f9efc7343df101424709`.
The original worker's transcript and logs remain unchanged at the supplied
`9cfe1e56-e5e1-4bde-957b-2f6476ba61e1` session / `agent-af8f21e89d66e5c3c` paths.

### Owners and behavior

- `CliFlowExecutor` is the one production `SkillExecutor`. The Task launcher
  supplies its harness, selected agent/account, steer polling, attachment,
  stall observation and review checkpoint policy. The saved launcher supplies
  ordinary CLI launches. `StepLauncher` has these two production implementations;
  neither owns a cursor or settlement fence. It is launch policy, not an adapter
  retaining the former Task driver.
- `FlowInvocation` replaces `FlowPosition`; one SELECT and decoder read both
  kinds. Cursor writes, failure/retry, current-attempt authority and operation
  Runs use `store/sqlite/flows.rs`, with the Task claim as an optional predicate.
  The shared driver verifies its claim before outcome recovery can write.
- Claiming a Task reserves its unpublished Run in the same transaction. The
  launcher publishes that exact Run. `current_run_id` replaces the claim's
  duplicate `worker_run_id`. Outcomes recover from Run rows.
- A Task invocation stores NULL cwd; the SELECT joins `tasks.worktree` and a
  trigger rejects a competing stored cwd. Invocation-addressed resume of the
  Task's managed Flow uses Task resume, preserving agent and unblock policy.
  `--retry` cannot bypass a restart-only Task failure or required feedback.
- Choosing a different Flow replaces the unclaimed managed invocation and
  changes the Task pointer atomically. Choosing the same name resumes its
  capture. A held worker claim requires stopping the worker first. Other Flows
  attributed to that Task stay intact. There is no runtime-child invocation
  tree yet; this cut replaces the extant root, not an unimplemented tree.
- Started reads `tasks.started_at`, including a reservation. Wave chat projects
  the column through its existing renderer, keeping historical event IDs (zero
  identifies a new column-derived observation). Rendering is idempotent and
  does not queue governance work, live or after replay. The Started trigger is
  gone. Historical Started events remain retirement evidence only; no Run is
  invented for them and they cannot turn the current Started column on.

### Deletions checked against H3's base

All 15 names below existed in the base and have zero references in current
Rust source. No production file was deleted; moving code is counted on both
sides of the line measurement.

| Removed | Surviving operation |
| --- | --- |
| `FlowPosition`, `read_flow_position_row`, `flow_position_in` | `FlowInvocation`, one Flow SELECT/decoder |
| `set_flow_position_in` | `insert_flow_in`, `write_cursor_in` |
| `settle_task_worker_in`, `block_task_flow_in`, `release_task_worker_in` | shared checkpoint, failure and release transactions |
| `claimed_position_in` | shared version/claim/current-attempt checks |
| `record_flow_verdict`, `record_flow_route` | invocation-keyed decision/path writes |
| `bind_task_worker_run` | claim reservation plus `publish_attempt` |
| `recover_task_decision` | `settle_attempt_in`, reading Run outcome |
| `run_task_op_boundary`, `run_task_flow_op` | shared executor `run_op` |
| `finish_task_flow_turn` | engine traversal and shared driver |

Also removed: the controller's production `cursor.finish` traversal, the
None-token Task fallbacks for decide/route/blocked, the claim's Run copy, the
`RunFlowStep.task_id` "stored at claim" flag behavior, and the Started event
trigger. Historical Started decoding remains. `record_task_start` had already
been removed before H3 and is not counted as this cut's deletion.

Measurement uses the retained `review-lines.py` method: Rust/Swift non-test
prefixes, excluding test files and trailing test modules, retaining inline test
helpers. Against H3's base: **+2,151 / −2,714 = −563**, 25 production files.
The new SQL draft adds **21 lines** (net −542 including SQL). `human_session.rs`
has **2,068** lines before its trailing test module. Whole branch against
`90a232aaf`: **+6,461 / −5,068 = +1,393**, 47 production files. These are not
whole-file or semantic-path counts. Exact file counts are retained in ignored
`line-counts.json` beside the backup.

### Counterexamples and repairs

The prior transcript records 24/25 store tests passing with the replacement
case failing, then 24/34 Task tests passing with ten failures; later retries
show seven failing review/launch cases and three failing decision/agent cases.
Those failures preceded its final green receipts; they have not been erased or
claimed as fresh Codex runs. The original final Clippy failed on a large
SessionTarget variant, an eight-argument cursor writer, and two `Ok(...).unwrap()`
assertions. Codex boxed the large enum payloads, grouped the existing id/version
fence as a tuple, and simplified the assertions without lint suppressions.

Codex's new populated migration regression failed with `claim_json = NULL`:
the stopped draft unconditionally released existing claims. It now removes only
`worker_run_id` from their JSON; process identity, generation, version, claim
time and current attempt survive. The regression compares the serialized claim
exactly, because compare-and-set uses those bytes.

The new CLI proof first queried the wrong field and then exposed the existing
published-only `runs` inventory. It now checks Task Started through `roadmap`
before launch, and lists the Run after publication. Its next setup failures
were a missing Chapter and a registered but nonexistent branch; the fixture
now creates both. Production inventory was not broadened to make the test pass.
The initial disposable-source copy also stopped at the Ghostty Gitlink; it was
recreated completely, with that non-Rust submodule represented by its directory.
None of these failed setups counts as proof. The documentation check then
found stale generated architecture HTML. A first generation attempt used the
root Python environment and failed on missing `fasthtml`; generation under
`uv run --project website` succeeded, and both documentation checks passed.

Review caught that the new Started projection initially used the observation
wake path. Live delivery and replay now render it without pending work; a
focused test proves the distinction. Review also caught internal Task IDs being
passed to the issue-selector resume API; the invocation route now resolves the
Task's issue identifier and the real CLI regression proves restart policy holds.

### Proof and boundaries

Resource preflight passed before verification (74.7 GiB free initially, 75.0
GiB at the later preflight; 64 GiB floor). Every build/test clears inherited
`LF_*` and `LOOPFLOW_*`, runs at nice +10 with four workers and a 900-second
phase bound. All fixtures use disposable Homes.

| Evidence | Result and scope |
| --- | --- |
| Prior worker `lib.log` | 637 passed, 1,054 skipped; retained receipt, not rerun. Covers selected ops/Run/store/Task/CLI/chat library tests. `cargo.sh` inspected: clears Loopflow authority; command used `-j 4`. |
| Prior worker `integration.log` | 87 passed, zero skipped across Session cutover/CLI, DTO, Flow, status, Task GitHub/initialization and PR tests. Same retained-receipt boundary. |
| Fresh focused library/CLI proofs | 40 distinct tests passed across the recorded runs: all 34 existing Task-controller tests; migration preservation; new Flow selection; historical retirement; real CLI claim/worker/resume; existing Task-observation replay; new Started replay. Earlier failed fixture runs remain logged. |
| Real CLI Task proof | Claim reserves `published=0`; roadmap shows Started; `task __worker` executes the captured operation after the template is removed, publishes and completes the same Run; `runs --task` returns it. A later invocation's restart-only failure survives `flow resume --retry`. The operation is `rebase --plan` in a disposable repo: no real provider or remote mutation is claimed. |
| Canonical materialization | Disposable source snapshot of 2,079 inputs; 14 drafts materialized as a hypothetical 0.12.23 batch. Populated claim preservation and the two-operation Task consumer both pass (two repeats). This is neither a release frontier nor installed migration acceptance. The later chat-only correction and Task resume selector fix do not alter that migration/operation proof. |
| Formatting / Clippy | `cargo fmt --all`; `cargo clippy --all-targets -j 4 -- -D warnings` pass. |
| Documentation | Portable architecture and README/index synchronization: two passed. Generated `docs/architecture.html` refreshed. |
| Whitespace | `git diff --check` passes. |
| Architecture | Seven inventories pass; SQLite remains 32/33 with the known `wave_chapters` gap. H7 owns it. |
| Migration history checker | Fails because `0.12.23.001_release.sql` exists on `origin/main` but not this branch. No release file was copied or rewritten to bypass the required rebase. |

Logs, runner, canonical source snapshot and counts are under ignored
`.lf/tmp/h3-codex-recovery/`. The 40-test set is incremental proof, not a new
all-green 637/87 run on the final tree. Task providers, PM and process failures
in library proofs are simulated. Real configured provider recovery, a real
five-minute stall, installed Home migration, desktop agreement and the full
LOO-298 acceptance remain unproven.

### Post-rebase review · 2026-09-27

Reviewed `77e44a6cce89cc9c601ac548fb32202485abeb09` on released main
`5bdcef6b65419d5db2583ee791cede2f1a95b3df`, at Jack's request. **H3 holds after
two bounded repairs.** This is local source acceptance of the common driver,
not whole-Task acceptance or publication. The [integration receipt](../h3-upstream-integration.md)
retains its earlier failures and proof boundaries.

- **Fixed: stopping an advanced worker.** The claim retains its original
  `position_version` across checkpoints. Stop passed that old value to release,
  so a dead worker could prevent restart after progress. The extended real-process
  regression failed with `Flow … changed under its driver`. Stop now supplies
  the observed current version and the same exact claim; replacement, unknown
  identity and still-live-process refusals remain intact.
- **Fixed: recovery could consume a replacement claim.** The driver's separate
  read/check did not fence the recovery transaction, and later step recovery
  made no comparison. A deterministic takeover regression showed recovery
  accepting the superseded caller and settling the replacement's interrupted
  attempt. Recovery now compares the supplied claim inside its transaction;
  rejected recovery leaves the row, attempt and Task events unchanged. The
  replacement can still recover. The separate pre-read is deleted.
- **Review handoff preserved.** Checkpoint returns the updated invocation,
  including its released claim when parking. The executor observes that result
  before entering the review; unchanged checkpoints also validate the claim.
  A new shared-driver test executes an operation, parks at the review and
  verifies its unpublished Session Run and retained Task/invocation ancestry.
- **Documentation repaired.** Removed duplicated Task create/restart/edit prose
  introduced by integration and documented same-Flow resume versus replacement
  of an unclaimed managed invocation by a different selection.

Ownership inspection follows launch, claim/reservation, publication, checkpoint,
recovery, review completion and restart. One production `SkillExecutor`, one
Flow SELECT/decoder and one cursor writer remain. All 15 predecessor names in
H3's deletion table are absent from Rust source; `cursor.finish` in the Task
controller is test-only, and `position.json` is import-only. Launchers supply
provider/review policy, not competing persistence. H2 replaced the saved-Flow
consumer; H3 replaces the Task consumer, so the two-pass no-replacement blocker
does not apply. Main's deletion admission reads typed Run attribution; its
stop-to-restart expected-observation transaction remains intact.

Fresh proof uses private Homes, scrubbed `LF_*`/`LOOPFLOW_*`, nice +10, four
Cargo workers and 900-second phase limits. Resource preflight passed at 70.4 GiB
free. Both defect regressions failed before production repairs (an earlier
fixture field typo failed compilation and was corrected before those assertions).
Results are retained under `.lf/tmp/h3-codex-recovery/review-*.log`:

- `review-final-focus.log`: 45 passed, one explicitly ignored configured-Codex
  test; Task controller, Flow recovery/fences and worker-stop selection. The
  additional parking test below brings fresh library proof to 46 passes.
- `review-parking.log`: one passed; operation-to-review driver handoff.
- `review-cli.log`: four passed through the real binary: claim-time reservation
  and captured Task operation after source removal, managed resume policy,
  attributed-Flow isolation, failed/retried step plus review, and taskless
  decision persistence. Providers/Linear are stand-ins, not configured services.
- Formatting, all-target Clippy and whitespace pass. Schema and architecture
  bytes are unchanged, so the integration's seven canonical repeats, released
  migration preservation and architecture/doc receipts retain their stated scope;
  they were not rerun or promoted to full-gate evidence.

Retained non-test-prefix measurement, excluding test files/trailing test modules,
docs and generated files but **retaining inline test helpers**: H3 plus integration
and this review versus rebased H3 parent `f47581d37`: **+2,213 / −2,751 = −538**
across 26 Rust/Swift files, plus its 21-line SQL draft. Review alone versus
`77e44a6cc`: **+36 / −25 = +11**, four production prefixes; the added lines carry
transactional recovery authority and the checkpoint's returned state. Whole branch
versus main `5bdcef6b6`: **+6,495 / −5,086 = +1,409**, 47 files; SQL **+461**.
`human_session.rs` remains **2,081** prefix lines. No production file was deleted;
named owner removals, not these counts, establish the reduction.

Remaining scope is unchanged: H4 owns missing Run/end writes and dead-skill
recovery; H5/H6 own Session/DTO reductions; status-based Chapters use
`../chapters.md`; runtime loop children and installed/desktop acceptance remain
unproved. The first-Home initialization race and prior OAuth contention flake
are unresolved. The architecture inventory still lacks `wave_chapters` (H7).
Next action belongs to the supervisor: Cut I is next in the recorded order,
followed by H4's killed-step/failed-end-write proof. No product decision, new Ask,
Flow edge, Task completion, push, PR mutation or installed-Home action occurred.
