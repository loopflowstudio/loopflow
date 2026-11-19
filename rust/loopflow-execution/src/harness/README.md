# Harnesses

Each provider (`claude.rs`, `codex.rs`, `opencode.rs`, `pi.rs`) gets launched,
observed and recorded here. The provider owns the conversation; lf adds context
at turn boundaries, interjects through hooks and records what happened.

## One launch type per harness

```rust
let launch = Launch {
    config: &config,
    harness,            // resolved once from the selected agent
    run,                // Chat, Batch, WorkSession or Worker
    recording: &recording, // store, Session and attachment; required before opening
    start: Start::Resume { thread: &thread },
    context_file: Some(&context),
};
```

Native and headless launches share `Launch` and the same per-harness `Harness`.
`native_session::run` opens and waits for a native client; `drive::run` drives
one headless provider turn. Native results retain the provider's OS exit status;
headless results retain answer text and typed turn failures for retry policy.

The four `LaunchKind` variants match the [glossary's launch names](../../../../docs/glossary.md#loopflows-words).

Work sessions and workers report turns to the Machine server. Chats and
batches read their steers directly and report no turn boundaries.
Transport selects `LaunchKind` once before `delegate` carries it out. Workers run
`lf task run` steps, the `ci-fix` run landing starts, and Discord answers for a
Wave. In-process callers such as commit messages, PR text and replay are
batches. Task attribution never changes this choice. Resume reads the
Session's binding and primary scope before delegation: a chat reopens as a
chat; a bound or primary conversation reopens as a work session.
`admit` creates the capture and claims its attachment before a native launch;
headless admission claims the runtime's attachment before constructing `Launch`.

Each harness file answers the rest:

| method | answers |
|---|---|
| `capabilities()` | who names the thread, initial prompt on resume, failure classification, native skills, turn limit, output schema, respawn after crash, install hint |
| `open(&Launch)` | native interactive client: argv, hook and title flags |
| `driver(&Launch, events, approval)` | headless stream `Driver`: start, input, interrupt, stop |

`HarnessKind::harness()` is the only harness dispatch. Core launch and transcript
code reads capabilities or asks the harness for policy.
That policy includes input recency, live process ownership and saved-turn detail.
A harness whose server may host other conversations validates it before the
shared AgentProcess close path signals the recorded process group.

An interactive client may `exec` into a held terminal.

| harness | interactive client | names the thread |
|---|---|---|
| Claude | native TUI, prompt on stdin | lf (`--session-id`) |
| Pi chat | native TUI | lf (`--session-id`) |
| Pi work session | native TUI with the lf extension | provider |
| Codex chat | `codex`, prompt in argv | provider (start hook) |
| Codex work session | `codex --remote` after lf opens the app-server thread | provider |
| OpenCode | `opencode --standalone`, prompt in `--prompt` | lf (`--session ses_…`) |

`cargo test -p loopflow-execution --lib harness::launch_matrix -- --nocapture`
runs every harness as a chat, work session, batch and worker against stand-in provider scripts, and prints each cell's argv, hook ownership, wait mechanism and recorded thread.
Headless matrix runs wait here; held-terminal exec handoff is covered by the
transport fixtures.

A headless OpenCode turn is one `opencode run --standalone --format json --auto`
process: the prompt on stdin, one JSON line per finished part, and an exit that
settles the turn. Each later turn is another run on the same `--session`. lf
starts no OpenCode server. The context plugin, loaded through
`OPENCODE_CONFIG_CONTENT`, supplies instructions, context, steers between tools
and, for a work session or worker, turn reports to the Machine server.

## What a Task step does on each harness

`lf task run LOO-12 -a <harness>` runs a step the same way on all four. Each
row names the test that shows it; "no" is a gap with its owner.

| a Task step | Claude | Codex | OpenCode | Pi | test |
|---|---|---|---|---|---|
| starts as a worker, settles, records its answer and native thread; the Graph advances | yes | yes | yes | yes | `task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness` |
| records its usage in the Task's history | yes | yes | yes | no: kept under Pi's turn id | same test |
| receives the step's context | yes | yes | yes | yes | same test; `harness::context::tests::` |
| hears a steer sent while it runs, after its next tool | yes | yes | yes | yes | same test; `harness::context::tests::` |
| runs as `yolo` in its Task's checkout | yes | yes | yes | yes | `harness::launch_matrix` |
| is passed over when its CLI is not installed, or named once when it was the only choice | yes | yes | yes | yes | `task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again` |
| fails the turn, with the question as its reason, when it asks one and nobody is there | not shown | not shown | yes | yes | `task_graph_launch_tests::a_question_with_nobody_there_ends_the_step` |
| stops on `lf task interrupt` with its turn recorded interrupted; the next `lf task run` starts the step again | not shown | not shown | yes | yes | `task_graph_launch_tests::a_worker_stops_on_interrupt_and_its_session_resumes` |
| moves to the next agent on a usage limit | yes | yes | no: failures unclassified (LOO-518) | no: failures unclassified | `agent::tests::` fallthrough tests |
| as an idle work session, hears a steer | yes | yes | no: next turn only | no: next turn only | `server::wake::tests::` |

Context and steers reach every harness the same way: it runs
`lf __context-block` when the conversation starts and after each tool. Claude
and Codex run it as a declared hook; OpenCode's plugin and Pi's extension start
it themselves. In the worker test each stand-in makes those two calls around
one tool (`tests/support/tool_turn.py`) while `lf steer` runs in between, and
reads back the block naming its Session and Task, then the steer. The plugin
and the extension themselves run under Node in `harness::context::tests::`.
On real OpenCode 2.2.1 and Pi 1.3.0 workers, a steer sent mid-step changed
the code each wrote. OpenCode keeps no system text between requests, so its
plugin sends every steer it has heard with each later request.

lf reads usage from each provider's own transcript. Pi's session file names a
turn by its entry id and lf's record of the same turn uses its request id, so
a Pi worker's usage lands on a second turn row that `lf history usage --task`
does not count.

`opencode run --auto` cancels a question itself and tells the model to go on
with assumptions; lf fails the turn at that tool call and stops the run. Pi
opens an extension dialog, and lf fails the turn there. What a headless Claude
or Codex does with a question has not been recorded.

An interrupted worker is a Run, and nobody continues a Run: the next
`lf task run` starts its step in a new conversation. The Graph stops any
harness's step the same way (`an_interrupt_ends_the_running_agent_step_and_starts_nothing_further`);
what a Claude or Codex worker records for the turn it was in has not been read.

A "no" for OpenCode or Pi means the step stops with the provider's error (limits)
or the steer waits for the next turn (idle wake); neither loses work.

A skill keeps one `LfProcess` across successive provider `AgentProcess` instances.
Its shared `LfProcessRuntime` in [`process/runtime.rs`](../process/runtime.rs)
holds the current attachment, provider, thread and outcome, with no
separate execution ID. Native launch and headless respawn use that private state;
harness input, shutdown and pending operations retain the exact store and
attachment selected at launch. Claude respawn gets a new attachment through the
runtime without changing those earlier snapshots.

The runtime holds a `SessionRecorder` under the same lock. The recorder publishes
input in [`session_record/capture.rs`](../session_record/capture.rs) and writes
history and receipts in [`session_record.rs`](../session_record.rs); it cannot
launch or stop a provider. Provider identity in memory advances before best-effort
history writes, so a recording failure cannot change which provider or thread a
retry continues. Later observations retry unrecorded identity without duplicating
successful writes.

The runtime retains the store that admitted its attachment for launch, retry and
cleanup. Completion settles the attachment before writing the receipt; final
destruction also settles unfinished runtime state, including after a recording
panic. A failed receipt write remains a headless caller error. Retrying that
write can record the same outcome but cannot reopen the runtime or change its
result.

Native clients own authentication. Saved account rows never select or activate
credentials. Headless classified refusals follow configured model/harness order;
interactive refusals leave the thread open without resending input. Once a spawn
is requested, retry needs an observed exit or confirmed shutdown before replacing
it. Passive usage and refusal facts remain provider-authored and write no account
routing state.

Native exit handles attachment cleanup separately and adopts a plain
`SessionRecorder` from `CaptureHandoff` to finish recording. It does not reconstruct
live runtime state. Recording failures there are logged; the provider's exit
result stands. See [execution](../../../../docs/dev/execution.md#runtime-and-recording)
for the ownership model.

## Session identity

The start and compact context blocks name the Loopflow Session and its Task
identifier and title when it has exactly one. Launch captures those in the
existing delivery file; managed start/compact boundaries refresh that file
through the Machine server. Batches read the launch snapshot without
opening a store or connecting to the server. Fixed provider instructions carry no
Session or Task identity.

## lf never breaks the conversation

lf is a session manager, context adder and hook interjector on top of a harness,
never a middleman that can get in the way (Jack Heart, 2026-11-05). It is never in
the byte path between a person and their provider except as a pass-through that
keeps passing when lf's own state is unavailable: a store that is locked, refused
by its reader floor, missing a column or gone; a hook that fails; a lock that
cannot be read.

Every lf invocation at a turn or tool boundary is best-effort. Every launch's
hook reads the Session's unread steers, Task updates among them, from the store, in
its own process, and at a tool boundary only when a steer may be waiting
([Every launch reads its own steers](../../../loopflow/src/server/README.md#every-launch-reads-its-own-steers)).
A chat or batch
([which launches are managed](../../../loopflow/src/server/README.md#turn-and-tool-boundaries))
does nothing else: it reports no boundary and asks no server. A managed launch's
hook first sends one `session.boundary` request to the Machine server at
`machine.sock` with a two-second deadline, and the server records the turn,
acknowledges the native input and, after a turn ends, records Codex turns,
refreshes planning and starts a pending Session move ([Turn and tool boundaries](../../../loopflow/src/server/README.md#turn-and-tool-boundaries)).
When the server does not answer, the hook still reads its steers, exits 0 and
records the gap once per native conversation in
`logs/lf.log`. The turn or tool completes without it. A recording gap is one
concept, `store::RecordingGap`: one log line per changed failure and one when
lf's state answers again, whether the gap lives in the server's
memory or, for hooks that are a fresh process at every boundary, in a
`.store-gap` marker beside the delivery file. The next hook that reaches
the server sends the marker's gap with its request; the server writes it into
the Session's history as one `store-gap` observation (first and last miss) and
the hook clears the marker, so a turn lf never saw reads as unknown rather than
as still running. Native usage refusals remain with the provider; hooks do not switch logins. The Pi extension and OpenCode plugin live inside the provider's
process, so they send the same request on a `machine.sock` connection of their
own, one per boundary, instead of starting `lf` at each boundary
([OpenCode and Pi report from inside the provider](../../../loopflow/src/server/README.md#opencode-and-pi-report-from-inside-the-provider)).
A server they cannot reach, or an lf they cannot run, goes to their own log
(`logs/pi-extension.log`, `opencode.stderr.log` beside the capture), never to
the conversation. The server
delivers typed input to a held terminal even when it cannot record it, and
holds the Session in memory instead. Quitting returns the provider's own exit within a second; exit
records finish in the background or are lost. Nothing about lf's own state ends a
conversation either: a removed, rebuilt or superseded lf executable leaves its
holder serving until the person quits or resumes elsewhere.

Codex (`codex_connection.rs`) has no relay: the Codex TUI connects straight to
its private app-server. lf sends the first turn there before the client starts;
its hooks reach the Machine server, which reads the thread's turns at each turn
end. An app-server the server cannot read is a recording gap; history fills it
at the next turn end or launch.

## Task updates

Keep several Task conversations open while a Graph finishes and a teammate adds
a comment. Each work session hears what changed on its Tasks: a Graph
run or step that settled, its Graph position, a comment from anyone else. A
comment with direction is a steer (source `comment:<id>`) and arrives only as
one.

A Task update is a steer lf writes (source `update:…`), owned by the Task. No
writer sits at each fact: `task_updates::write` compares the Task with its
last reading (`task_update_readings`) and saves what moved, one steer for its
position and runs and one per comment. Whoever looks first writes it for
everyone: a conversation's hook before it claims its steers, or the Machine
server before it wakes an idle conversation
([Waking an idle conversation](../../../loopflow/src/server/README.md#waking-an-idle-conversation)).
A second reader of the same change finds the reading already moved and writes
nothing. So an exit no one recorded is still reported the next time anything
looks.

Only interactive conversations read updates, never the Session whose comment
it is, and only they look: a worker Run rebuilds no Task history. The Task's
first reading is a baseline, and a conversation that just started marks the
updates written before it as read. The steer is rendered as an observation,
not direction. A run between steps keeps its last report until its next step
or its end. The server caches the revision at which a Task had nothing new; a
hook's process is too short-lived to.

The revision's triggers follow Task membership, Graph and step facts, comments
and delivery receipts. Transcript, usage, availability and ordinary tool-process
writes leave it unchanged, so idle conversations cost one revision read per
store change.

### Regression acceptance

Run `cargo test -p loopflow-execution --lib harness::task_updates::tests::` for what an
update reports, `server::boundary::tests::` and `server::wake::tests::` for its
delivery, and `harness::codex_connection::tests::` for the Codex turn start.
Revision upgrades have `store::migrations::tests::task_update_revision_upgrade_preserves_existing_counters`.
`performance_tests::task_wait_reuses_its_store_and_builds_status_only_on_return`
checks the CLI wait loop; `ops::task::tests::task_wait_condition_releases_terminal_tasks`
checks completion, abandonment and missing Tasks.
Use the environment isolation in [verification commands](../../../../scripts/testing.md).
