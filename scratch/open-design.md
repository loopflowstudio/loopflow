# Open a conversation — proposed

## Report, 2026-11-10

Jack Heart: “often when i ask a loopflow session to open a task for me, i expect them to open the task session for me but end up with this view instead”

Jack Heart: “Yeah i think lf open needs a product-level redesign and simplification”

Jack Heart, on the status pane's data-format bug: “This should definitely be fixed”

Jack Heart: “hmm i think there should be three modes, but ino rder of priority (1) directly open in this terminal (default for direct invocation in rex or a temrianl at CLI) (2) open as a sibling tab in this terminal (this should be default if an agent does it) (3) just focus (what happens on desktop, or if we can otherwise detect it in the same terminal i.e. rex?)”

Jack Heart, answering whether an agent should focus a Task's existing tab in the same Rex terminal: “Focus the existing tab (Recommended)”

Jack Heart: “weird though that i dont think i want auto focus if its not oen”

Jack Heart: “maybe just error and say already open but allow a --focus param?”

The supplied screenshot shows LOO-820 with unknown state and checkout, no workflow and no conversations. A read during this conversation reported `feature` at `explore` and two closed conversations. The command that produced the opening is not known.

## Proposed experience

`lf open --task LOO-820` reaches the Task's primary conversation. An existing conversation opens with its history; a Task without one gets its first conversation. A failed opening explains what failed and leaves a retry of the same command meaningful. Opening never advances the Task's Graph.

Three presentation modes follow the stated direction:

| Mode | Experience | Default context |
| --- | --- | --- |
| Here | Attach the conversation in the invoking terminal; return to its shell on exit. | Direct CLI invocation, including Rex. |
| Sibling | Open the conversation in a sibling tab of the invoking terminal. | Agent invocation. |
| Focus | Reveal an existing view. | Desktop; explicit `--focus` in a terminal. |

Flag spelling remains proposed: `--here`, `--tab`, `--focus`. The default is determined once at the entry point, then follows the same Session resolution and presentation path as an explicit mode. Calling from an agent changes the presentation default, not which conversation the Task names.

The latest proposal makes terminal focus explicit. Ordinary opening reports “Already open in …; use --focus” when it finds an existing view, without switching focus or creating a duplicate. `--focus` reveals that view and reports “Not open” if no view exists. Focus must identify the actual Session being viewed, not guess from a tab title.

The extent of “already open” remains to be defined: the same Rex terminal, any host on this Machine, or remote hosts too. Desktop's normal navigation was described as focusing; keep that distinct from a terminal command's default.

Task is the work; Session is its conversation; a host presents a view of that Session. Resolve or prepare the primary Session, then present it in the selected mode. Reuse current Session and attachment records; do not introduce a second persistent workspace registry. The agent opening its own Session needs to hand presentation to the host without recursively resuming itself inside its tool call; this cannot be implemented by silently substituting a status pane.

Workspace arrangements, repository/Wave overviews, changes and workflow inspection remain useful. Decide their explicit entry points before removing any current capability. A layout choice must not replace a requested conversation with a status pane.

The proposed internal API is one resolved conversation plus `OpenMode::{Here, Tab, Focus}`. Host adapters implement presentation, rather than selecting another Task/Session or substituting a tool. Ordinary Task opening stops passing through workspace layout rendering. Workspace creation keeps its own explicit action; the command name remains open.

## Proposed usage and agent guidance

```sh
lf open --task LOO-820   # reach the Task's conversation
lf open --session ID    # reach this exact conversation
lf task view LOO-820    # inspect workflow and recorded work
```

Proposed examples, with flag names still undecided:

```sh
lf open --task LOO-820 --here   # attach in this terminal
lf open --task LOO-820 --tab    # open a sibling conversation tab
lf open --task LOO-820 --focus  # reveal its existing view
```

For a request to open a Task, use `lf open --task <id>` and report the actual result. Creating a workflow pane does not fulfill a request to open a conversation. Do not start the Graph to open its conversation.

## Next decisions

1. How far does “already open” look: the same terminal, this Machine, or other Machines?
2. Define repository/Wave opening and explicit workspace arrangements. Trace normal opening, closed-session recovery, unavailable hosts and cross-Machine placement through the same model. A plain terminal without a sibling-tab capability needs a clear result, not a silent change of mode.

The immediate status rendering repair is independent of these proposed opening semantics.

## Verification, 2026-11-10

The isolated terminal regression against `/Applications/Loopflow.app/Contents/MacOS/lf` reproduced unknown task/checkout, “No workflow taken up,” empty conversations and unknown Graph process fields. Against this checkout's build, the same pane retained workflow, checkout, conversations and Graph steps after a conversation-title update.

`cargo test -p loopflow --lib task_view -- --nocapture` with inherited `LF_*` unset and stdin closed: 3 passed. The linker emitted its oversized `__eh_frame` warning. The installed app has not been replaced.

## Repository opening report, 2026-11-10

Jack Heart: “~/src/manabot. it opened what looked like the right set of tasks, but they all failed to open”

A replay of the installed `lf` opening plan with Rex replaced by a recording stub selected MTG-242, MTG-250, b015d5, MTG-267 and MTG-269. All five recorded checkouts existed locally. Each planned pane was a Conversation running `lf resume` with its actual interactive Session ID, rather than Task & workflow. Preparing MTG-242's selected Session succeeded without starting its provider. The stub recorded no real Rex changes; its launch arguments and receipts are under `/tmp/lf-open-diagnostic.v67wcm/`.

The original failed tabs were no longer present in the inspected Rex inventory. Their error text has not been recovered, and the cause of their failure is still unknown. Nearby Desktop logs reported an older binary rejecting the `agent_limit_recoveries` migration, but nothing connects that separate error to these Rex launches.

Opening currently reports `opening` and “Created a new Task tab” when the host accepts the layout. The redesign must distinguish that from a conversation becoming usable and carry a child launch failure back to the opening result. Decide how to retain a failed pane's error long enough to read it, rather than losing it when its command exits.

## Attach and input failure, 2026-11-10

Jack Heart: “can you open those 5 tabs for me” and “and figure out why the conversations failed to attach”.

Opening all five as Conversation tabs in Rex `amber-willow` exposed these errors:

```
Error: invalid data: Session e30ecfae91b84e97b2049b9d74318439 dispatch is still busy; retry after the current send finishes
Error: invalid data: Session 881aafe7262040e486a79a1c429dfe16 dispatch is still busy; retry after the current send finishes
```

Jack Heart: “other three worked i think”. Later, after `lf session new` in MTG-269: “i found that the session wasnt receving input” and “when i tried to start ti new this happened”, with the same dispatch-busy error for `session_c73fe3a8e9e94b56b1ffe915b575a9c0`. A visible prompt had not established usable input.

`lsof` found `lf __codex-observe` holders for MTG-242 (PID 18360, born Nov 8 15:30), b015d5 (90401, Nov 9 08:19), MTG-250 (56414, Nov 8 15:29) and MTG-267 (95391, Nov 9 11:51). One-second stack samples of 18360 and 90401 both showed `Recorder::watch → TaskUpdates::pending → task_overview → graphs_of_task → graphs_in → sqlite3_step` while each held its Session fence file. Samples remain at `/tmp/lf-open-lock-18360.txt` and `/tmp/lf-open-lock-90401.txt`. MTG-269's reported lock was no longer held when inspected; its provider was replaced during the concurrent manual recovery.

Recovery sent SIGTERM only to those four verified observer PIDs. Their Codex providers remained alive. MTG-242 and b015d5 then reconnected; the two exited duplicate tabs were removed. The manually added split panes were retained. A nonblocking exclusive-lock probe acquired and immediately released all five Session fences. No prompt was submitted to the conversations as a test.

The code still wraps `TaskUpdates::pending` in `with_session_attachment` in `harness/codex_connection.rs::pending_update`. Task-history reads can therefore block attachment and input with the misleading “current send” error. This needs a separate repair: keep observation outside the dispatch fence, retaining the check and short fence at actual delivery. Recovery alone does not remove that code defect.

## Sleep and wake disconnect, 2026-11-10

Jack Heart: “hmm all my lf sessions quit with Error: Execution owner disconnected; outcome is unknown” and “after i closed my laptop and reopened in a new place (where i briefly didnt have wifi)”.

`pmset` recorded sleep at 17:45:23, a maintenance wake at 18:01:04, sleep again at 18:01:06 and lid wake at 18:10:49 Pacific. At 18:14, the four original Codex PIDs (15870, 53186, 89312, 93487) still existed. MTG-242's Rex pane retained “Execution owner stopped answering; outcome is unknown, no provider was restarted and no input was replayed”. No further process signals or reconnection attempts were made during this investigation; manual restarts were visible concurrently.

The transport has a sleep-sensitive timeout on both sides: `Viewer::expire_heartbeat` uses 30 seconds of `SystemTime` and runs before reading queued requests; the relay likewise checks wall time since its last response and exits for local connections. Sleep can therefore cause either side to close a healthy local connection on wake. These connections use Unix sockets, independent of Wi-Fi. This matches both observed errors and surviving providers; no per-connection timeout event was logged to establish which side fired first for every Session. The corrective change is to measure heartbeat silence in awake time, and make local connection recovery retain the same execution without replaying input. No transport change has been installed from this checkout.

## Sleep/wake repair in this diff

Jack Heart: “can yo add that fix to this diff”.

Heartbeat deadlines on both sides now use monotonic uptime. An acknowledged local terminal can reconnect directly to its existing owner; it shares the existing recovery protocol, discards outage input and requests only free claims. A failed automatic reconnect leaves Ctrl+R retry / Ctrl+] close in the terminal. Command pipes and initial attachment retain bounded failure. Detached-session retirement still uses wall time. This does not repair the separate observer/dispatch lock scope described above.

Verification is headless: the new regression pauses only a fixture relay, closes its connection and checks the same owner and one provider launch, exact delivered input, another viewer's claims and explicit retry after temporary unavailability. It does not suspend the actual laptop. The first automatic-reconnect/newer-viewer run passed; final focused tests and lint are pending. Nothing from this checkout has been installed into the running app.
