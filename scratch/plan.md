# LOO-1101: a conversation stays in the directory it runs in

## The report

> in a worktree i started w/o a task initially (manabot.academy-test-drive), i had an agy lf session going. then i wanted to move to fable, so i did `lf resume -a claude:fable` but it sai there was no work in this worktree

## What happened (read from `~/.lf/loopflow.db`, 2026-11-25)

- `session_94d9963b…` (agy, skill `default`) started at 1795575720 in
  `/Users/jack/src/manabot.academy-test-drive`, a linked worktree no Task owns
  (branch `jack-heart/mtg-388-seat-table`). Its first `processes` row has that cwd.
- At 1795579888 it was bound to MTG-388 (`work_source='bound'`, `bound_at` set,
  row in `task_primary_sessions`). MTG-388 already owned
  `/Users/jack/src/manabot.mtg-388-getting-into-a`, so the bind rewrote
  `agent_sessions.cwd` to that path. The agy process kept running in
  `academy-test-drive`, where the uncommitted work is.
- `lf resume -a claude:fable` from `academy-test-drive`: `checkout_scope` finds no
  owner (`task_checkouts` holds the other path), `checkout_conversation` matches
  on `agent_sessions.cwd` and finds nothing, and resume answers
  "This worktree has no associated work conversation."
  (`rust/loopflow-execution/src/ops/human_session.rs:173`).
- Three fresh `claude:fable` conversations followed in that worktree
  (1795595096, …114, …123), none seeded from the agy thread.
- Second symptom, seen live: the agy process ended at 1795595379 (cause not
  read) and the server's recovery pass restarted it at 1795595387 with
  `--conversation 2c51a976…` in `manabot.mtg-388-getting-into-a`, a checkout
  that has none of the conversation's files. `logs/lf.log`:
  `Task conversation started session=session_94d9963b… attempt=1`.

## Cause

Four places assumed a Task's primary conversation always lives at
`task_checkouts.worktree`:

1. **`lf session bind` to a Task with an existing worktree** (fixed in `e1a15bfcf3`).
   `adoption::save(.., continue_in_task = true, ..)` computed `destination`,
   and `promote_task_prepared` ran `UPDATE agent_sessions SET cwd=?2`.
   No local transcript relocation exists, so on directory-keyed providers like
   Claude Code (`~/.claude/projects/<encoded-cwd>/`), rewriting `cwd` also
   broke native `--resume`.
2. **`admit_scope_workspace` / `admit_session_workspace` guarding on leaving
   `work_source='bound'`** (fixed in `e1a15bfcf3`).
   If a conversation already belonged to a Task (`task_id = Some(T)`,
   `work_source = 'declared'`, `bound_at = None`, started from another checkout
   or `main`) and was made primary via `lf session bind`, the next idle
   `lf resume` silently rewrote `agent_sessions.cwd` to `task.worktree`.
3. **Server recovery and Task conversation outlook requiring
   `rtrim(s.cwd, '/') = rtrim(c.worktree, '/')`** (fixed in this pass).
   `task_conversation_here_sql` (`rust/loopflow-storage/src/sqlite/session_recovery.rs:45`)
   previously checked `rtrim(s.cwd, '/') = rtrim(c.worktree, '/')` and populated
   `SessionSummary.in_task_checkout`. When a Task was checked out at `X` and its
   bound primary conversation ran in `Y` (another worktree, `main`, a Wave
   checkout, or a subdirectory of `X`):
   - `task_conversations_here` excluded it, so the server's recovery pass never
     restarted it after a crash, kill, or reboot.
   - `session_ending` mapped `in_task_checkout = false` to `Left::Elsewhere`, so
     `lf task status` and Desktop (`WorkSurfaceView`) falsely printed
     `"its Task is not checked out here"` for a Task that IS checked out on
     this Machine.
4. **Re-selecting an already-assigned Session as primary refusing when another
   Task owns its directory** (fixed in this pass).
   In `ops/task/adoption.rs`, `resolve_checkout` rejected any Session whose
   directory belonged to a different Task (`checkout_task != session_task`) with
   `"The Session and checkout belong to different Tasks; permanent bindings are
   unchanged"`. If `S` was bound to Task `A` in unowned worktree `Y` and Task
   `B` later adopted `Y` (QA scenario 6) — or if `S` was started with
   `lf --task A` inside `B`'s checkout — then repeating
   `lf session bind S --task A` or clicking **Make primary Task conversation**
   in Desktop failed instead of re-selecting `S` as `A`'s primary.

## Design: once a conversation has started or been bound, its directory stays put

Three rules align binding, workspace admission, recovery, and checkout ownership:

1. **Binding changes what a conversation belongs to, never where it runs.**
   - `adoption::save`, `store::ownership::promote_task`, and
     `promote_task_prepared` commit Task assignment and primary selection
     without rewriting `cwd`.
   - When the Task already owns a different worktree, `lf session bind` prints
     one factual line noting both paths:
     `Conversation stays in <session_cwd>; <TASK>'s checkout is <task_worktree>.`
   - When `adoption::bind_session` re-selects a Session that already belongs to
     the target Task (`session.task_id == Some(task.id)`), a different Task
     owning `session.cwd`'s worktree does not fail the bind and does not become
     a checkout placement candidate (`checkout = None`). Creating a new Task
     (`lf task create --session` / `--worktree`) still refuses when a Session
     and checkout belong to different Tasks.
2. **Workspace admission places only unstarted, unbound primary placeholders.**
   In `admit_scope_workspace` (`primary.rs`) and `admit_session_workspace`
   (`sessions.rs`), gate placement on `session.captured.is_none() &&
   session.bound_at.is_none()` (SQL: `CASE WHEN current_capture IS NULL AND
   bound_at IS NULL THEN ?3 ELSE cwd END`). Fresh primaries created before
   their checkout exists still land in their scope's worktree before first
   launch. Any conversation that has already launched or been bound keeps its
   directory — unless the directory is gone from disk (`leave_gone_directory`)
   or it moves to another Machine (`reopen_moved_session`).
3. **Recovery and conversation outlook follow whether the Task is checked out
   on this Machine, not whether `session.cwd == task.worktree`.**
   - In `task_conversation_here_sql` (`rust/loopflow-storage/src/sqlite/session_recovery.rs`),
     drop `AND rtrim({s}.cwd,'/')=rtrim({c}.worktree,'/')`. Keep the open-Task,
     primary-Session, unfinished, interactive, `moved_to_machine_id IS NULL`,
     and `task_checkouts` existence checks (`JOIN task_checkouts c ON
     c.task_id=t.id` in `task_conversations_here`; `EXISTS(SELECT 1 FROM
     task_checkouts pc WHERE pc.task_id=t.id)` in `session_summary`).
   - Rename internal `SessionSummary.in_task_checkout` →
     `task_checked_out_here` so the field matches what it measures and what
     `Left::Elsewhere` prints (`"its Task is not checked out here"`).
   - When a dead primary conversation of an open, locally claimed, locally
     checked-out Task is recovered, the pass runs `lf session resume <session>`
     which launches in `session.cwd` (or falls back to the repo's main checkout
     via `leave_gone_directory` if `session.cwd` was removed from disk).

What follows across layers:

- `lf resume -a claude:fable` in the conversation's worktree finds it through
  `checkout_conversation` and starts the new thread there, seeded from the agy
  log (`invocation_intent` → `NewThread` → `new_thread`).
- `lf resume MTG-388` / `--task` from anywhere opens the same conversation in
  the directory it runs in.
- Crash/kill/reboot recovery restarts an open, claimed, locally checked-out
  Task's bound primary conversation in its own `cwd`, and `lf task status` /
  Desktop report `starting again` instead of falsely claiming `"its Task is not
  checked out here"`.
- Re-selecting an already-assigned Task conversation (`lf session bind` /
  **Make primary Task conversation**) keeps working even if another Task owns
  its worktree.
- `agent_sessions.cwd` and `processes.cwd` agree again.
- Two production reads of leaving column `agent_sessions.work_source` disappear
  (`primary.rs`, `sessions.rs`).

Choices made for review (recorded in `scratch/concept-review.html`):

- **Never move on bind (including out of `main` and Wave checkouts).**
  Passed over: *keep the move and match live process cwd on resume* and *move
  only out of shared checkouts*.
- **Print one factual line on `lf session bind` only when the Task already owns
  a different checkout.** Passed over: *say nothing about directories*.
- **Auto-recover a dead bound primary when its Task is checked out on this
  Machine (in the conversation's own `cwd`), and leave checkout-less Tasks
  stopped with `"its Task is not checked out here"`.** Passed over: *require
  `cwd == task.worktree`* (silently breaks recovery on every split checkout and
  prints a false status message), and *recover even checkout-less Tasks* (would
  race `lf task create --run` setup and auto-start un-checked-out backlog Tasks).

The trade-off: a Task whose checkout is `X` can have its primary conversation
in `Y`. Graph workers, `lf task changes`, PR delivery, and `lf task move` still
operate on `X`; `lf task status`, recovery, and `lf session bind` act on the
real paths.

## User-visible outcome

In a worktree where a conversation runs, `lf resume` and `lf resume -a <agent>`
open that conversation whether or not it was bound to a Task that has another
worktree. Binding reports the Task it bound to, and when the Task already has a
different checkout, notes both paths without moving the conversation. If that
bound conversation dies while its Task is open and checked out here, the server
restarts it in its own directory and `lf task status` reports that accurately.
Repeating `lf session bind` or **Make primary Task conversation** keeps working
even if another Task adopts the conversation's worktree.

## Demo

```bash
cd ../repo.research && lf -a agy            # a conversation in a Taskless worktree
lf session bind <session> --task LOO-12     # LOO-12 already has ../repo.loo-12
lf resume -a claude:fable                   # same Session, new thread, still in ../repo.research
# Kill provider → server recovery restarts it in ../repo.research; `lf task status` shows coming_back
```

## Source of truth

- `agent_sessions.cwd` is where the conversation runs.
- `task_checkouts.worktree` is where the Task's branch, PR, and Graph workers live.
- Binding writes `task_id`, `work_source`, `bound_at`, and `task_primary_sessions`;
  it stops writing `cwd`.
- `current_capture IS NULL AND bound_at IS NULL` identifies an unplaced primary
  placeholder that `admit_scope_workspace` / `admit_session_workspace` may
  place before first launch.
- `task_checkouts` (on this Machine) + `task_primary_sessions` + open Task +
  unfinished interactive local Session identifies a Task conversation this
  Machine's server recovers (launching it in `agent_sessions.cwd`).

## Affected surfaces

### Already built and committed (through `57ed33d102`)

- **Storage (`rust/loopflow-storage/src/sqlite/`):**
  - `children.rs`: removed `destination` from `promote_task_prepared`, its
    `UPDATE agent_sessions SET cwd=?2`, and its stale-workspace check.
  - `sessions.rs`: `admit_session_workspace` guards with
    `CASE WHEN current_capture IS NULL AND bound_at IS NULL THEN ?3 ELSE cwd END`;
    `checkout_conversation` accepts `for_task: Option<&TaskId>` so adoption
    skips Sessions bound to another Task while resume still sees them.
- **Execution (`rust/loopflow-execution/src/`):**
  - `store/ownership.rs`, `store/children.rs`: dropped `destination` and paired
    `(NewTask, Option<WaveId>)`.
  - `ops/task/adoption.rs`: dropped `continue_in_task` and `destination`;
    emits `Conversation stays in <session_cwd>; <TASK>'s checkout is <task_worktree>.`
    when the Task owns a different checkout.
  - `ops/human_session.rs`: prints `Binding … Permanent.` before running
    `bind_session`; passes `None` to `checkout_conversation`.
  - `ops/human_session/primary.rs`: `admit_scope_workspace` returns early when
    `session.captured.is_some() || session.bound_at.is_some()`.
- **CLI, docs, Desktop:**
  - `rust/loopflow/src/lf/mod.rs`: `lf session bind` `after_help` updated.
  - `docs/work.md`, `docs/sessions.md`, `docs/dev/reference-models.md` updated.
  - `swift/LoopflowMac/Views/SessionActionSheet.swift` copy and
    `swift/LoopflowTests/SessionControlsTests.swift` updated.
  - Native resume (`session_lifecycle_tests.rs`) and provider-switch
    (`session_launch_contract.rs`) fixtures added/updated.

### Built in post-concept-review pass

1. **Recovery & outlook decoupled from `s.cwd == c.worktree`:**
   - `rust/loopflow-storage/src/sqlite/session_recovery.rs`: dropped
     `AND rtrim({s}.cwd,'/')=rtrim({c}.worktree,'/')` and the `c` parameter from
     `task_conversation_here_sql`; added unit test
     `task_conversations_here_follows_task_checkout_not_session_cwd` covering
     split-checkout inclusion, local-checkout removal, and `query_plans` index
     check on the summary predicate.
   - `rust/loopflow-storage/src/session.rs`, `rust/loopflow-storage/src/sqlite/sessions.rs`,
     `rust/loopflow-execution/src/ops/human_session.rs`, `rust/loopflow-execution/src/ops/recovery.rs`:
     renamed internal `SessionSummary.in_task_checkout` → `task_checked_out_here`
     and updated comments on `DownConversation.here` / `Left::Elsewhere`.
   - `rust/loopflow-execution/src/ops/human_session/tests.rs`: updated
     `the_pass_starts_what_the_outlook_says_the_server_starts` so `cwd='/elsewhere'`
     with `task_checkouts` present yields `ComingBack` + due recovery, and
     deleting `task_checkouts` yields `WaitingForYou` (`"not checked out here"`).
   - `rust/loopflow/src/server/recovery.rs`: added
     `a_dead_task_conversation_in_another_directory_restarts_in_its_own_directory`
     verifying `recover_conversations` starts a split-checkout primary and
     `service::continuing` launches it in `session.cwd`, while deleting
     `task_checkouts` leaves it down.
2. **Re-selecting an already-assigned Session in another Task's checkout:**
   - `rust/loopflow-execution/src/ops/task/adoption.rs`: passed `target: Option<&TaskId>`
     into `resolve_checkout` (`None` from `resolve_source`, `Some(&task.id)` from
     `bind_session`). When `worktree.is_none()` and `target == Some(session_task)`,
     a different Task owning the directory clears `checkout` / `checkout_task`
     instead of refusing, so `bind_session` re-selects the Session as `task`'s
     primary without adopting the other Task's checkout.
   - `rust/loopflow-execution/src/ops/task/adoption/tests.rs`: added
     `reselecting_an_assigned_session_in_another_tasks_checkout_keeps_both_checkouts`
     covering QA scenario 6 follow-up, `lf --task` started inside another Task's
     checkout, and refusal to reassign across Tasks.
3. **Docs checked:** `rust/loopflow/src/server/README.md`, `docs/work.md`,
   `docs/sessions.md`, and `docs/dev/reference-models.md` already match.

### Remaining

None for implementation; full native fixtures and Desktop `SessionControlsTests`
run at gate/CI.

## Absent and error states

- A worktree with no conversation still answers `"This worktree has no
  associated work conversation."` Unchanged; see `scratch/questions.md`.
- The conversation's worktree was removed from disk (`!session.cwd.is_dir()`):
  `leave_gone_directory` (`primary.rs:285`) falls back to the repository's main
  checkout when a repository is recorded on the Task, Wave, or Session; with no
  repository on record, launch reports the missing directory.
- A Task with no checkout on this Machine (remote Machine, or never checked out
  locally) has no `task_checkouts` row, so recovery leaves its conversation
  down with `"its Task is not checked out here; \`lf resume --task …\` opens it"`.
- Sessions already moved by a past bind keep their recorded `cwd`. No repair
  code: `lf resume <session>` reaches them from anywhere.

## Exclusions

- Giving a Task a second checkout, or moving files between worktrees.
- `lf task checkout`, Machine moves (`lf session move`, `lf task move` — note
  that `moves::send_task` still checkpoints `task.worktree`, separate follow-up
  if split-checkout moves need warning/refusal).
- Changing `lf resume` in an empty unowned linked worktree.
- Removing `agent_sessions.work_source` completely (`LOO-836`) or merging
  test-only `bind_session_prepared`.
- Why the agy process ended at 00:29.

## Verification

- Run focused Rust tests covering both new slices:
  - `cargo test -p loopflow-storage --lib sqlite::session_recovery::`
  - `cargo test -p loopflow-execution --lib ops::human_session::tests::`
  - `cargo test -p loopflow-execution --lib ops::task::adoption::tests::`
  - `cargo test -p loopflow --lib server::recovery::`
- Verify SQLite query plans for `task_conversations_here` and `session_summary`
  unchanged or improved (`cargo test -p loopflow-storage --lib query_plans::`).
- Native resume, provider-switch fixtures, and Desktop `SessionControlsTests`
  run at gate/CI.
