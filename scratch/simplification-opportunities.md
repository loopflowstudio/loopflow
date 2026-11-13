# Steer delivery: what is left

Area: steer delivery to Sessions (`steer.rs`, `store/sqlite/steers.rs`,
`server/boundary.rs`, `server/wake.rs`, `harness/task_updates.rs`,
`lf/commands/context.rs`, the OpenCode and Pi plugins).

## Product intent

Write a direction once, for a Session, a Task, a Wave or a repository, and every
running agent it applies to reads it at its next boundary. lf adds to the
conversation and never stands in its way.

## Where the branch stands

Built, uncommitted, focused tests passing (nextest, 4 threads):

- Every launch's hook claims its own steers from the store. One
  `steer_delivered` row per (Session, steer): `claimed_at` before the print,
  `delivered_at` after; an unstamped claim lapses after 15 s. The server
  records turns and wakes idle conversations; its boundary answer carries
  nothing.
- One verb: `lf steer TARGET 'msg'` (Session or Task ID), `lf --wave W steer`,
  `lf --repo . steer`. `lf session steer` and `lf task steer` are gone from the
  tree; `aliases.rs` redirects them and `lf comment`. `lf steer 'msg'` with no
  target, Wave or `--repo` is an error. API kinds stay `session.steer` and
  `task.steer`; `lf steer ISSUE --json` prints the Task thread, as `task.steer`
  answers.
- `--retry` is gone; `lf session steers ID` stays, with `--source`.
- Task updates are steers lf writes (`harness/task_updates.rs`): source
  `update:<writer Session or ->:<key>`, Task-owned, last reading per Task in
  `task_update_readings`. Whoever looks first writes; only interactive
  conversations look and read; a conversation that just started marks earlier
  updates read. Rendered as "Task update …", an observation.

## Finish plan

Scope is the "Left to do" below and nothing more: no redesign, no new
behaviour. Publish by updating draft PR #1923 (title still "Steer running
subagents without a Machine server") and marking it ready. Do not merge.

Order, each with its check (tests always `cargo nextest run -p loopflow
--build-jobs 4 --test-threads 4 -E '<filter>'`, `LF_*` unset, `< /dev/null`):

1. **Query plans for the hook path.** `query_plans.rs`
   `session_lookups_and_steer_reads_plan_by_index` traces `session_steers`
   and `session_steer_source`, not what a hook runs. Add to its traced
   closure: `claim_session_steers` (the `unread()` scope with the
   `update:%` filter), `baseline_task_updates`, and
   `harness::task_updates::write` for a Task Session. Add
   `task_update_readings` to `GROWING` (one row per Task) and a `MEASURED`
   row for its autoindex sized like `tasks`. `source NOT LIKE` is a filter on
   rows already found by `session_id`/`task_id`/`wave_id`/`repo`, so it should
   not create a scan; if it does, fix the filter, not the test.
   Check: `-E 'test(store::query_plans)'` passes.
2. **Lint and schema.** `cargo clippy --all-targets -- -D warnings`;
   `uv run python scripts/check_migrations.py` (`steer_delivered.sql` is the
   branch's only migration and is additive: two new tables, an index, a copy).
   Check: both clean.
3. **Focused suites not yet run.** `test(store::migrations)`,
   `test(served_reads)`, `test(global_commands)`,
   `test(wave_resolution_matrix)`, `test(dto_fixtures)` (the server schema
   fixture keeps the retired `printed`/`steers`/`update`/`waiting` fields; a
   failure there means the fixture, not the wire, moved). Two filters per run.
   Swift (`RegistryQuery.swift` argv) is left to CI.
4. **Docs.** `docs/dev/data.md` gains `task_update_readings` beside
   `steer_delivered`; `docs/dev/execution.md` ~234 and
   `docs/dev/server-first.md` say the hook claims its own steers and the
   server only records turns and wakes; `docs/sessions.md` "Keep several
   conversations open" and `docs/troubleshooting.md` name `lf steer` and
   `lf session steers`, never `--retry`. Then `website/dev.py sync-docs`.
   Check: `grep -rn 'session steer \|task steer \|--retry' docs README.md
   rust/loopflow/src/builtins` finds nothing outside alias notes.
5. **Removed integration test.** Not restored: `task_updates::tests::
   comments_after_launch_reach_the_conversation_but_its_own_do_not` and
   `wake::tests::an_idle_claude_conversation…` cover the behaviour; the
   hook print path is covered by `session_steers.rs`. Noted in the PR body.
6. **Publish.** Commit, PR title "One steer verb; every launch reads its own
   steers; Task updates as steers", body lists the retired wire fields
   (LOO-838 deletes them), the contract migration Task 3b0089, LOO-841 as a
   person's call, and the Known edges below.

Done when: `lf steer TASK 'msg'` saved from one shell is printed once at the
next hook of every running conversation in that Task, bare or held; the
second hook at the same moment prints nothing; a Task comment from another
Session reaches an interactive conversation as "Task update …"; PR #1923 is
ready with CI started.

## Left to do (source list)

- **Not yet run since the last edits:** `cargo clippy --all-targets -- -D
  warnings`, `uv run python scripts/check_migrations.py`,
  `test(served_reads)`, `test(global_commands)`, `test(wave_resolution_matrix)`,
  `test(store::migrations)`, `test(store::query_plans)`, the Swift tests that
  spell steer argv (`swift/Loopflow/Services/RegistryQuery.swift` changed), and
  `dto_fixtures` (the server schema fixture may need the retired fields).
  Machine load is high: `cargo nextest run -p loopflow --build-jobs 4
  --test-threads 4 -E '<one or two modules>'`, never the broad suite; CI runs
  that.
- **`task_update_readings` and update steers have no query-plan row** in
  `store/query_plans.rs` (`GROWING`, `MEASURED`). The scope query gained
  `p.source NOT LIKE 'update:%'`; check it with `EXPLAIN QUERY PLAN` at the
  measured size.
- **Docs to reread for the new model:** `docs/dev/data.md` (new table),
  `docs/dev/execution.md` line ~234, `docs/dev/server-first.md`,
  `docs/troubleshooting.md`, `docs/sessions.md` "Keep several conversations
  open". `website/dev.py sync-docs` regenerates the command tree.
- **A removed test** (a Task comment reaching the next boundary once) sits in
  the scratchpad of the conversation that removed it; the same behaviour is now
  covered by `task_updates::tests::comments_after_launch…` and
  `wake::tests::an_idle_claude_conversation…`. Decide whether an integration
  test through the hook is still wanted.
- **LOO-841** (progress delivery confirmation) described a Task update lost by
  a failed send. Updates are steers now and stay pending until sent; its docs
  references are removed. Whether it can close is a person's call.
- **Contract migration.** `session_steers`, `steer_batches` and
  `steer_deliveries` are unread. Drop them once `steer_delivered` has been on
  origin/main for `STAGING_DAYS`. Filed as Task 3b0089 (saved locally, pending
  Linear sync).
- **Retired wire fields.** `printed` in `session.boundary` and `steers`,
  `update`, `waiting` in its answer stay for OpenCode and Pi conversations
  started before this build. Delete them with `ContextMoment::TaskUpdates`
  (LOO-838).

## Known edges

- Conversations started before this build gate their tool hook on
  `steers/latest` alone, so mid-turn they miss Task and Session steers until a
  prompt or turn end.
- A bare interactive conversation with no server learns of a Task change only
  when its hook next starts for another reason: nothing touches a marker when
  a Task fact changes.
- A Task steer or update is excerpted at 400 or 600 bytes in the conversation;
  the complete source is the file the block names.
- Burst cost is unmeasured. One reading, 2026-11-13, quiet moment, this
  Machine's store (839 MB, 2 GB WAL, 319 `lf` processes): installed
  `lf session steers <id>`, which opens the store, 27 ms median of 12 against
  22 ms for `lf --version`. #1879's copied store and 50-Session tool burst is
  the comparison (p99 7.8 s when hooks wrote at every boundary, 1.19 s through
  the server). An interactive conversation's hook now rebuilds its Tasks'
  history at each read; only the server keeps the quiet cache.
- `lf steer TARGET --explain` previews a Task steer even when the target is a
  Session.

## Open: `Holder`

`Caller`, `Owner`, `Server` still pick the capability variable
(`LF_BARE_STEER` or `LF_PROVIDER_CONTEXT`). They hold the same value; the name
says whether a server records the conversation. A flag on the hook command
does not work: conversations started before this build run the new `lf` with
their old command lines.

## Concept review, 2026-11-13

Page: `scratch/concept-review.html`. Nothing below is built.

Said so far, Jack Heart, this conversation:

- On an idle bare conversation hearing a steer only at its next prompt: "this ok"
- "nested worktrees not something i hink we should support"
- On the receipt: "i would just think there is like just a steers table (or
  maybe session_steers, task_steers, wave_steeers, repo_steers?) and then a
  steer_delivered table which is keyed by (session, steer) so 1 per session
  rather than 1 per steer"
- On the batch tables: "yeha, soudns like we dont needt that"

Jack Heart on the four questions: "yeah lets do the proposal", then
"1 dont know or care 2a 3a  4b".

Not built, by that answer to question 1: the save printing who it reaches,
`lf steer show STEER`, and collapsing the markers to one per Session.
