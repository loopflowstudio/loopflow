# Create Linear issues reliably; stop draining pending exports every pass

Task 647863. Jack Heart asked on 2026-11-05 why no Task created since ~02:31Z
reached Linear and why Loopflow spends most of its hourly Linear allowance
re-probing stuck exports. The Task description holds the evidence; this is the
plan.

## Cause

`LinearClient::graphql` ran `linear_observe::check_before_write` before every
mutation. For a creation that check sat between `attempt_planning_export`
(attempted=1) and `DeliverTaskCreation`, inside the export's five-second
deadline: it opened a second Store, listed every Wave and all ~990 Tasks,
derived each Task's state, took the Team lock and possibly read a `TeamChanges`
delta. The deadline fired before the create was sent, so every creation ended
"timed out; uncertainty retained", and `lf task sync` refused to repeat an
uncertain create without `--retry-creation`, whose own attempt hit the same wall.

Independently, every pass over pending exports built a fresh repository client
(`ListTeams` to validate the Team claim) and observed each uncertain export
(`FindExportIssue`) before deciding not to retry it: O(N) requests per pass,
times ~11 concurrent lf processes.

## Change

1. **No hidden check inside writes.** `graphql` sends the mutation. Freshness
   runs at work boundaries (`sync_scope`, `team_current`) and explicitly in
   `deliver_task_state`, which already called it. Deleted: the `check_before_write`
   call in `linear/mod.rs`.
2. **Team claim validated once per process.** `resolve_repository_context`
   remembers (team, checkout path) after one successful `ListTeams`; fixtures
   get a fresh key from their temp dir. A long-lived process learns a `lf reteam`
   on restart.
3. **Uncertain creations are observed once per window, then retried with the
   saved id.** New column `export_observed_at` on `task_creation_intents` and
   `project_transitions` (migration `export_observed_at.sql`). Only a look at an
   uncertain (attempted) creation is stamped, so a creation whose first send
   timed out gets one look on the very next pass. The window rule lives in one
   place, `planning_export::export`: a pass refuses an attempted export observed
   within the window before building a client; a look that finds the issue
   absent re-sends the create with its saved id (idempotent) and logs it.
   Window = max(10 min, `delta_fresh_secs()`): longer as the budget tightens,
   until reset inside the reserve. A pass logs "looked Ns ago, next look in Ms"
   inside the window; `lf task sync ISSUE` is the person's look and ignores it.
4. **Timeouts name the step.** "Export timed out while sending the creation;
   creation identity and uncertainty retained."

Pass cost over N pending exports when nothing changed: 0 requests. Documented in
`rust/loopflow/src/ops/README.md`.

## Delete — do not maintain (done)

- `check_before_write` call inside `LinearClient::graphql`. Its one remaining
  caller is `deliver_task_state`.
- `retry_task_export`, then `retry_export` and the `--retry-creation` flag
  (`retry_creation` in `lf/mod.rs`, `bin/lf.rs`, `task_sync`): `lf task sync`
  calls `planning_export::export(…, force=true)` for the Task and its Project.
- `task_sync_does_not_claim_success_for_uncertain_publication` → replaced by
  `uncertain_creation_is_resent_once_per_window_and_on_task_sync`.
- The fake Linear server now refuses a creation whose client id already exists,
  as Linear does; hidden-lookup recoveries in two tests use `export(…, true)`.
- Compress (2026-11-05): the SQL `observed_since` filter on
  `planning_export_owners` (the Rust check is the one window rule), the
  `CASE` clearing the stamp on first attempt, `LinearClient::endpoint()` and the
  `#[cfg(test)]` claim-key suffix, the fixture's dead `team_changes` counter and
  `seed_finished_tasks`'s unused `count`. Second pass: `planning_export_observed_at`
  (the receipt reader `planning_export_attempts` now returns `ExportAttempts`
  with `creation`, `link` and `observed_at` instead of a `(bool, bool)` tuple)
  and `attempt_planning_export`'s `retry_creation` flag with its
  `({column}=0 OR ?4)` clause: the caller holds the object's delivery lock and
  has just read the receipt, so the guard was always true. A `Cell` for the
  timeout step name does not compile; the Graph runner's futures must be `Send`.
  Third pass (after the flag cut): `PreparedExport` carries the receipt's
  `ExportAttempts` instead of copying two of its fields, the one-caller window
  helper is inlined at its check, and the fixture's `PlanningState::sent(op)`
  replaces the per-test `operations.get(..).copied().unwrap_or_default()`.

## Acceptance (from the Task)

- Fixture server answering after a realistic delay, ~1,000 Tasks and 20 uncertain
  exports: `task_create` sends `DeliverTaskCreation` and binds; fails on today's code.
- A pass with 20 uncertain exports inside the window: ≤1 `ListTeams`, 0 `FindExportIssue`.
- `lf task sync ISSUE` sends the create or names the blocked step (the flag
  the Task names was deleted at concept review; the plain command meets it).
- After `lf install`: the 19 listed Tasks get identifiers (demo step; record in PR).

## Checks

`cargo test -p loopflow --lib -- export uncertain task_sync timed_out_creation
task_create_reaches_linear rate_limit_retries_creation store::sqlite::task_history
store::sqlite::linear_team_checks` 52 passed (259 s, after the second compress pass);
`cargo clippy --all-targets -D warnings` clean; `scripts/check_migrations.py` ok (export_observed_at unmerged). Gate filter:
`test(ops::linear::task_planning_tests::) | test(store::sqlite::planning_export::) |
test(store::sqlite::linear_team_checks::) | binary(lf)` minus the two long
`ops::linear` fixtures named in scripts/testing.md; website suite for docs/work.md.
Sync (2026-11-05): merged main `384505b120` (upgrade-docs audit; it curated
Wave memory and touched `lf/mod.rs`) without conflicts; `cargo check -p loopflow
--all-targets` passes on the merge. Realign found code, README, docs and plan in
agreement; the Wave memory lesson survived main's curation.

## Concept review (2026-11-05, headless)

Accepted experience, now implemented: creating a Task reaches Linear inside its
own deadline; a creation that timed out is looked for on the next pass and
re-sent with its saved id when absent; passes spend nothing on exports they
looked at inside the window. Observe-before-create and the saved id are kept.

**Design change: `lf task sync ISSUE` is the explicit look; `--retry-creation`
goes.** Evidence: `lf task create` already answers "Saved locally; pending Linear
sync. Retry with `lf task sync <id>`" (`bin/lf.rs`), and docs/work.md says
"`lf task sync LOO-12` retries one Task". On this branch that same command can
answer "looked 40s ago, next look in 560s (`--retry-creation` looks now)": the
product sends the person to retype its own hint with a flag. Once a look that
finds nothing re-sends the creation itself, "retry creation" is no longer an
action a person takes; the flag's only residue is skipping a wait the person
never chose. No skill, Graph or Desktop path runs `lf task sync`
(grep over builtins, swift, .lf/graphs), so forcing the look there cannot
restart the drain; the window stays on passes, which are the automatic callers.
The Task's acceptance line for `--retry-creation` ("sends the create or names
the blocked step") is met by `lf task sync` itself.

Implemented (2026-11-05): `planning_export::export(store, repo, work, force)`
wraps the urgent budget; `sync_export` is the pass (false) and `task_sync` calls
`export(…, true)` for the Task and its Project. `retry_export`, the
`retry_creation` field and the `--retry-creation` option are gone; `--retry-updates`
stays because uncertain field writes are still not retried automatically. The
in-window refusal reads "looked Ns ago, next look in Ms" and names no flag; a pass
logs it at debug. docs/work.md, `ops/README.md` and the renamed test
`uncertain_creation_is_resent_once_per_window_and_on_task_sync` follow, with the
in-window wait asserted on `sync_export` and the look on `task_sync`.
Check (realign, 2026-11-05, on 70d5470eed): `cargo test -p loopflow --lib --
uncertain_creation task_sync` 6 passed (21 s); `grep -rn retry_creation
rust/loopflow/src docs` empty; clippy and fmt clean on this same tree from the
third compress pass (13 tests, 108 s).

Kept as recorded assumptions, not blocking: the ten-minute healthy window and
auto-retry for Projects (`scratch/questions.md`). Vocabulary split between
`observe`/`export_observed_at` in code and "look" in user text is noted, not
worth a loop.

Second pass (2026-11-05, on b89dee04fd, after implement/compress/realign): the
code, README and docs/work.md say the same three things as the accepted design
(a pass waits out the window, `lf task sync` looks now, a look that finds nothing
re-sends the saved id). Recovery checked against the fixture: a re-send that
Linear refuses as a duplicate returns that refusal from `lf task sync`, keeps the
creation attempted, and the next look binds the existing issue; nothing is
created twice (`uncertain_creation_is_resent_once_per_window_and_on_task_sync`).
The refusal reaches the person as Linear's own "already exists" text, which is
truthful and self-resolving; rewording it is not worth a loop. `--retry-updates`
stays asymmetric with creations because field writes were outside this Task's
scope, not because of a new decision. No design change and no open product
choice; the step advances to the demo.

## Remaining (demo step)

- `lf install` the branch, then `lf task sync <id>` for the 19
  listed Tasks (12 loopflow, 6 etude-studio, 1 manabot) or let the drain run;
  record each identifier in the PR. Installed lf 0.15.1 is still draining: the
  request log for the hour before 2026-11-05 07:55Z shows ~7,000 sent, ~1,200
  deferred and ~4,300 paused requests, half of them `ListTeams`; the drain stops
  spending once the branch is installed.
- Jack decides whether ten minutes is the right healthy-budget window
  (`scratch/questions.md`).
