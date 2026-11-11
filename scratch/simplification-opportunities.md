# Simplification Opportunities

Read on branch `c3932878a`, after idle ending and the native-input ledger were
deleted. Area: conversation lifetime (transport owner, Task retirement, resume,
warm starts).

## Product intent

A conversation lives as long as what it belongs to: an open Task, Wave or
repository keeps it running; a closed Task ends it. Switching between
conversations should cost at most a slower start, never lost work.

## Opportunity 1: closing a conversation is a request, not a judgment each owner makes

**Misalignment**: Closure is a Task event (done after its grace, abandoned),
but every owner still decides it for itself, as it did when idleness was
per-owner.

**Symptom**: `transport/lifetime.rs` (658 lines) runs a thread per owner that
reads the store every 2 s: `shell_close_request`, `owner_task_abandoned`,
`owner_task_completed` (which itself walks completed Tasks, viewer holds and
`unsettled_work`), then `owner_retention`. `Ending::confirm` repeats the shell,
abandonment, retention and completion checks a second time before exit.
Desktop shells already work the other way: retirement sets
`desktop_shells.close_requested`, and the owner obeys (`close_allowed`).

**Realignment**: Retirement and abandonment write one close request per owner
(the shell mechanism, extended to conversation owners). The owner's policy is:
record viewing, and end when a close request is present and it is unviewed with
no queued input.

**Cascade**: `owner_task_completed`, `owner_completed_tasks` and
`owner_task_abandoned` leave the owner's poll; the double check in `confirm`
goes; the `output` and `viewers` counters in `Activity` (race guards for idle
ending) go, leaving `viewed`. About 85 owners stop polling Task state every 2 s.
Shells and conversations share one close path, so `close_allowed` stops being a
special case.

## Not taken: resume attaches, only rollover replaces

Jack Heart answered "1a" to the review's question 1: resume keeps replacing an
idle owner on another build, and an unsent draft is lost. The superseded
branches in `human_session::attaches` and `OwnerRetention::NoOpenSession` stay.

## Opportunity 3: one resident limit instead of three

**Misalignment**: "How many conversations stay warm" is one concept with three
owners, none of which bounds running agents any more.

**Symptom**:
- `BACKGROUND_TASK_CONVERSATIONS = 16` (`human_session/primary.rs`) skips
  starting new Tasks' conversations once 16 run, so the newest start cold.
- `SessionsWorkspaceRegistry.warmConversationLimit` with
  `RepositoryWorkspace.reconcileWarmth` evicts the stalest Desktop panes
  across repositories. Before this branch an evicted pane's agent idle-ended two
  minutes later, so this was the effective least-recently-viewed cache. Now it
  evicts views only, and the agents keep running.
- The per-Machine agent count ("6a") is deferred until LOO-824's emulator can
  tell an editor is empty.

**Realignment**: one Machine resident count in config, read by the server.
Background start always runs; Desktop asks the server which conversations are
warm instead of keeping its own bound; suspension fills in later behind the
same number.

**Cascade**: the Rust cap and its skip path go (question 2b); Desktop's
cross-repository warmth reconciliation becomes a server answer, and its
retention tests follow.

## Leftovers already tracked

- `session.end_detached_after` shim in `config.rs`: removal Task
  `task_c118380f31f14d07ba28669499efa395`.
- `processes.native_input` column: unread and unwritten; dropping it is a
  staged contract migration. No Task filed yet.

## Aligned areas

- `transport/input.rs`: 37 lines, a byte queue forwarded as written. Nothing
  left to remove.
- Viewer holds recorded by the owner and read by retirement for the
  fifteen-minute grace: one record, one reader.
- `RecordingGap` per operation in the owner's poll: failures stay visible
  without ending the conversation.
