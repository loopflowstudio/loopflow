# Cut H review — H1: Task points at its invocation; strict equality

2026-09-27 · LOO-298 · review-slice of `4820e1f86` (code) and `0fdda5066`
(ledger), diff base `7eb3046a8`. Brief: [Cut H](cut-h-one-flow-driver.md)
(slice H1 and its ledger entry). Decisions: [concept review](../concept-review.md)
and the 2026-09-27 entries in [questions](../questions.md). Later sections of
this file supersede earlier ones; nothing here is superseded yet.

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
