# Steer delivery: what is left

Area: steer delivery to Sessions (`steer.rs`, `store/sqlite/steers.rs`,
`server/boundary.rs`, `server/wake.rs`, `harness/task_updates.rs`,
`lf/commands/context.rs`, the OpenCode and Pi plugins).

## Product intent

Write a direction once, for a Session, a Task, a Wave or a repository, and every
running agent it applies to reads it at its next boundary. lf adds to the
conversation and never stands in its way.

## Where the branch stands

Built and committed; focused checks below:

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

## Checks, 2026-11-13

All `cargo nextest run -p loopflow --build-jobs 4 --test-threads 4`, `LF_*` unset:

- `test(store::query_plans) | test(store::migrations)`: the hook test found
  `task_updates::write` scanning `agent_sessions` through `task_overview`'s
  Session list; it now reads `task_progress` (Graphs and position only). Pass.
- `test(harness::task_updates)`, `test(session_steers::)`,
  `test(wave_resolution_matrix::) | test(dto_fixtures::)`,
  `test(/^lf::tests::/) | test(documented_commands::)`: pass.
- `binary(served_reads) | binary(global_commands)`: `lf steer ISSUE --explain
  --json` printed text (its `steer_json` id was not read); fixed, pass.
  `open_delivers_exact_task_to_desktop_without_preparing_its_execution` fails
  here with "No agent selected" (Machine PATH has several harnesses); CI.
- `cargo clippy --all-targets -- -D warnings`, `check_migrations.py`: clean.
- Swift (`RegistryQuery.swift` argv): CI.

## Left to do

- **LOO-841** (progress delivery confirmation) described a Task update lost by
  a failed send. Updates are steers now and stay pending until sent. Whether it
  can close is a person's call.
- **Contract migration.** `session_steers`, `steer_batches` and
  `steer_deliveries` are unread. Drop them once `steer_delivered` has been on
  origin/main for `STAGING_DAYS`: Task 3b0089.
- **Retired wire fields.** `printed` in `session.boundary` and `steers`,
  `update`, `waiting` in its answer stay for OpenCode and Pi conversations
  started before this build. Delete them with `ContextMoment::TaskUpdates`
  (LOO-838).
- The removed integration test (a Task comment reaching the next boundary once)
  is not restored: `task_updates::tests::comments_after_launch…` and
  `wake::tests::an_idle_claude_conversation…` cover it, `session_steers.rs`
  the hook print.

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
