# Harnesses

Each provider (`claude.rs`, `codex.rs`, `opencode.rs`, `pi.rs`) gets launched,
observed and recorded here. The provider owns the conversation; lf adds context
at turn boundaries, interjects through hooks and records what happened.

A skill keeps one `LfProcess` across successive provider `AgentProcess` instances.
Its shared `LfProcessRuntime` in [`process/runtime.rs`](../process/runtime.rs)
holds the current attachment, provider, account, thread and outcome, with no
separate execution ID. Native launch and headless respawn use that private state;
harness input, shutdown and pending operations retain the exact store and
attachment selected at launch. Claude respawn gets a new attachment through the
runtime without changing those earlier snapshots.

The runtime holds a `SessionRecorder` under the same lock. The recorder publishes
input in [`session_record/capture.rs`](../session_record/capture.rs) and writes
history and receipts in [`session_record.rs`](../session_record.rs); it cannot
launch or stop a provider. Provider identity in memory advances before best-effort
history writes, so a recording failure cannot change which account or thread a
retry continues. Later observations retry unrecorded identity without duplicating
successful writes.

The runtime retains the store that admitted its attachment for launch, retry and
cleanup. Completion settles the attachment before writing the receipt; final
destruction also settles unfinished runtime state, including after a recording
panic. A failed receipt write remains a headless caller error. Retrying that
write can record the same outcome but cannot reopen the runtime or change its
result.

Native exit handles attachment cleanup separately and adopts a plain
`SessionRecorder` from `CaptureHandoff` to finish recording. It does not reconstruct
live runtime state. Recording failures there are logged; the provider's exit
result stands. See [execution](../../../../docs/dev/execution.md#runtime-and-recording)
for the ownership model.

## Session identity

The start and compact context blocks name the Loopflow Session and its Task
identifier and title when it has exactly one. Launch captures those in the
existing delivery file; managed start/compact boundaries refresh that file
through the Machine server. Bare headless runs read the launch snapshot without
opening a store or connecting to the server. Fixed provider instructions carry no
Session or Task identity.

## lf never breaks the conversation

lf is a session manager, context adder and hook interjector on top of a harness,
never a middleman that can get in the way (Jack Heart, 2026-11-05). It is never in
the byte path between a person and their provider except as a pass-through that
keeps passing when lf's own state is unavailable: a store that is locked, refused
by its reader floor, missing a column or gone; a hook that fails; a lock that
cannot be read.

Every lf invocation at a turn or tool boundary is best-effort. A bare headless
run ([which launches are managed](../server/README.md#turn-and-tool-boundaries))
has no such boundaries: it runs `__context-block` only at session start and
after a compaction, to print its context block, and asks no server. Otherwise
`__context-block` from Claude hooks opens no store: it
sends one `session.boundary` request to the Machine server at `machine.sock`
with a two-second deadline, and the server records the turn, acknowledges the
native input, answers pending steers (delivered once the hook's next request
says it printed them) and Task updates and, after a turn ends, records Codex
turns, refreshes planning and starts a pending Session move ([Turn and tool boundaries](../server/README.md#turn-and-tool-boundaries)).
When the server does not answer, the hook prints the context block without
steers, exits 0 and records the gap once per native conversation in
`logs/lf.log`. The turn or tool completes without it. A recording gap is one
concept, `store::RecordingGap`: one log line per changed failure and one when
lf's state answers again, whether the gap lives in an owner's
memory or, for hooks that are a fresh process at every boundary, in a
`.store-gap` marker beside the delivery file. The next hook that reaches
the server sends the marker's gap with its request; the server writes it into
the Session's history as one `store-gap` observation (first and last miss) and
the hook clears the marker, so a turn lf never saw reads as unknown rather than
as still running. Work that can outlast a hook's timeout runs beside it: Claude's
`StopFailure` hook starts `--moment record-limit` in its own OS session and exits,
because a kill between a token reply and the profile write would lose a rotated
grant. The Pi extension and OpenCode plugin live inside the provider's
process, so they send the same request on a `machine.sock` connection of their
own, one per boundary, instead of starting `lf` at each boundary
([OpenCode and Pi ask from inside the provider](../server/README.md#opencode-and-pi-ask-from-inside-the-provider)).
A server they cannot reach, or an lf they cannot run, goes to their own log
(`logs/pi-extension.log`, `opencode.stderr.log` beside the capture), never to
the conversation. The PTY
owner delivers typed input even when it cannot record it, and holds the Session
in memory instead. Quitting returns the provider's own exit within a second; exit
records finish in the background or are lost. Nothing about lf's own state ends a
conversation either: a removed, rebuilt or superseded lf executable leaves its
owner serving until the person quits, resumes elsewhere or an operator rolls over.

Codex (`codex_connection.rs`) has no relay: the Codex TUI connects straight to
its private app-server. lf sends the first turn there before the client starts;
its hooks reach the Machine server, which reads the thread's turns at each turn
end. An app-server the server cannot read is a recording gap; history fills it
at the next turn end or launch.

## Task updates

Keep several Task conversations open while a Graph finishes and a teammate adds
a comment. Each managed conversation hears what changed on its Tasks: a Graph
run or step that settled, its Graph position, a comment from anyone else. A
comment with direction is a steer (source `comment:<id>`) and arrives only as
one.
History from before its first boundary is a baseline, not a report to repeat.

`task_updates::read` compares the Tasks against the last update the Session
received, recorded in its history. It caches the revision at which a Session
had no news, avoiding history rebuilds until that revision changes. It runs
at two places in the Machine server, never in a process of its own: each
`session.boundary` answer carries the update to the hook, which prints it with
the steers, and the server wakes an idle managed conversation when the store's
`task_updates` revision moves
([Waking an idle conversation](../server/README.md#waking-an-idle-conversation)).
Both record the update as `dispatched` before it leaves, so it reaches the
conversation once whichever goes first. A run between steps keeps its last
report until its next step or its end. An exit no one records stays unreported
until something else on the Task moves.

The revision's triggers follow Task membership, Graph and step facts, comments
and delivery receipts. Transcript, usage, availability and ordinary tool-process
writes leave it unchanged, so idle conversations cost one revision read per
store change.

### Regression acceptance

Run `cargo test -p loopflow --lib harness::task_updates::tests::` for what an
update reports, `server::boundary::tests::` and `server::wake::tests::` for its
delivery, and `harness::codex_connection::tests::` for the Codex turn start.
Revision upgrades have `store::migrations::tests::task_update_revision_upgrade_preserves_existing_counters`.
`performance_tests::task_wait_reuses_its_store_and_builds_status_only_on_return`
checks the CLI wait loop; `ops::task::tests::task_wait_condition_releases_terminal_tasks`
checks completion, abandonment and missing Tasks.
Use the environment isolation in [verification commands](../../../../scripts/testing.md).
