# Steer delivery: what is left

Area: steer delivery to Sessions (`steer.rs`, `store/sqlite/steers.rs`,
`server/boundary.rs`, `server/wake.rs`, `harness/task_updates.rs`,
`lf/commands/context.rs`, the OpenCode and Pi plugins).

## Product intent

Write a direction once, for a Session, a Task, a Wave or a repository, and every
running agent it applies to reads it at its next boundary. lf adds to the
conversation and never stands in its way.

## Where the branch stands

Every launch's hook claims its own steers and Task update from the store. One
`steer_delivered` row per (Session, steer): `claimed_at` before the print,
`delivered_at` after; an unstamped claim lapses after 15 s. The server records
turns and wakes idle conversations; its boundary answer carries nothing.

## Left to do

- **Contract migration.** `session_steers`, `steer_batches` and
  `steer_deliveries` are unread. Drop them once `steer_delivered` has been on
  origin/main for `STAGING_DAYS`; their rows were copied into
  `steer_delivered`, so there is nothing to export. Filed as Task 3b0089
  (saved locally, pending Linear sync).
- **Retired wire fields.** `printed` in `session.boundary` and `steers`,
  `update`, `waiting` in its answer stay for OpenCode and Pi conversations
  started before this build, whose in-process plugin still sends and parses
  them. Delete them with `ContextMoment::TaskUpdates` (LOO-838).
- **Conversations started before this build** gate their tool hook on
  `steers/latest` alone, so mid-turn they miss Task and Session steers until a
  prompt or turn end.
- **Burst cost.** Unmeasured under load. One reading, 2026-11-13, quiet moment,
  this Machine's store (839 MB, 2 GB WAL, 319 `lf` processes): installed
  `lf session steers <id>`, which opens the store, 27 ms median of 12 against
  22 ms for `lf --version`. #1879's copied store and 50-Session tool burst is
  the comparison (p99 7.8 s when hooks wrote at every boundary, 1.19 s through
  the server). A held prompt hook now opens the store every time.
- **`task_updates` quiet cache** is per process, so a hook rebuilds Task
  history at each read; only the server's wake keeps the cache.

## Open: Task updates as steers

Not done. A Task update is a reader-side diff (this Session's last reading
against the Task now); a steer is a row someone wrote. Converting needs a
writer at every fact, and three things do not carry over:

- Steers reach every Session in the checkout, Graph steps included. Updates go
  only to interactive conversations; a step would be told about its own run.
- A steer reads as direction with an author. An update says "observation, not a
  request or permission to launch work".
- "An exit no one records stays unreported until something else on the Task
  moves" works because the reader derives it. A writer that never ran writes
  nothing.

What did unify: both ride the same hook read and the same wake, and the
server's boundary answer has neither.

## Open: `Holder`

`Caller`, `Owner`, `Server` still pick the capability variable
(`LF_BARE_STEER` or `LF_PROVIDER_CONTEXT`). One variable would do if a hook
could tell "the server records this conversation" another way. A flag on the
hook command does not work: conversations started before this build run the
new `lf` with their old command lines.

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

Proposed:

1. The save works out its audience once: prints it, touches each reached
   Session's marker, and `lf steer show STEER` lists receipts per Session.
   `steers/latest`, `steers/task/` and `LF_STEER_TASK` would go.
2. One verb, `lf steer TARGET 'msg'`. Today `lf session steer 'msg'` with the
   ID left out steers the whole repository.
3. Drop `--retry`; say it again with another steer.
4. Task updates as steers (above).

Counterexamples: a running Session bound to the Task after the save needs the
bind to touch its marker; "reaches N" is a save-time reading and a later
Session skips what another already read; a Wave or repository save touches one
marker per live Session; a second steer reaches Sessions that followed the
first.
