# Held terminals and headless work

```sh
lf resume --task LOO-12     # a view of a terminal the holder keeps
lf resume SESSION
lf --machine build resume SESSION
ps -axo command | grep '^lf '
# lf server run
# lf holder run
```

[What runs on a Machine](../../../../docs/dev/index.md#what-runs-on-a-machine)
is the model: commands choose lifetime independently of Task attribution.
This README owns the holder and view protocols, headless launches, exit
settlement and Desktop shells.

## Held terminals

A held conversation has no process of its own. The Machine server asks a
holder to start the agent on a PTY and keeps everything else: the terminal
registry, one libghostty-vt terminal per held terminal, views, viewing
records, queued input, the child's control requests and exit settlement.

`local_session_kind` answers which `LaunchKind` runs in this process;
`delegate` starts a held terminal for the rest and relays its view. Launches
are bare or managed, with [four launch names](../../../../docs/glossary.md#loopflows-words).
Only a work session goes to a held terminal; every other kind runs in its
caller's process.

| Command | Launch | Lifetime |
|---|---|---|
| Direct `lf`, skill or Graph launch in the caller's terminal or pipe, with or without `--task`, `--wave` or `LF_AS` | chat or batch | its caller or terminal |
| `lf -b steer <ID> TEXT` | batch; a Graph's correction turn inside a Task run is a worker | one turn, then exits |
| Resume a chat with no message | chat | its terminal |
| Resume a bound or primary Session, or open a Task, Wave or repository conversation | work session | held interactive terminal, independent of its viewer |
| Open a conversation in a held terminal through a Desktop pane or framed `lf --machine` view | work session | held interactive terminal, independent of its viewer |
| Headless steps of `lf task run` or `lf task auto on`, and the `ci-fix` run landing starts | worker | headless until the step or repair ends |
| A Discord answer for a Wave | worker | returns the answer to the bridge |
| Desktop Task shells | held shell | independent of its viewer, no hooks |

`lf task run` and `lf task auto on` start no agent themselves. `transport::detach_current`
reruns them detached from their terminal so the Graph outlives the caller;
`lf -b task run` waits for it.

To open a work session from the CLI, `lf` calls `session.start` and becomes
a view of the returned terminal. With the server unreachable it fails with
`Loopflow server is not running; run lf server status`. The one launch
nobody views, a Task's background conversation, goes unstarted instead and its
Graph runs on.

In a terminal the view is a small `lf` relay, so a host that shows the
foreground program, such as Rex, shows `lf` there, not the agent. Attaching
never creates: a view of a Session whose terminal is gone says so and offers
resume.

### Launch lifetime and Task membership

A command chooses the launch kind; the checkout, `--task` and `LF_AS` choose which
Task it belongs to. Every launch belonging to a Task counts toward its usage
and receives its steers. Work sessions and workers report turn
boundaries to the Machine server; chats and batches read their steers directly.

A Session reopened in this process runs as the kind `local_session_kind`
returns; any other goes to a held terminal through `delegate`. Direct
launches outside a held terminal or worker step choose between a chat and a
batch. `--batch` selects a batch. Otherwise `--interactive`, a terminal or no
prompt selects a chat. Reopening a Session with no message reads its binding
and primary scope once to choose its kind. Chats reopen as chats, and bound and
primary conversations reopen as work sessions. A Desktop pane or a framed
`lf --machine` viewer is no terminal a chat can own, so a Session it reopens
is a work session the holder keeps.
An unreadable store falls back to a work session and logs a recording gap.

### Chats at a terminal

Chats started with `lf` or a skill run as the terminal's foreground process-group
leader. Closing the terminal ends them. `os_process::foreground` hands over
the terminal before exec and takes it back after; Ctrl+Z stops lf with its
provider and `fg` continues both. `lf resume` reopens a chat in
its terminal too.

### Identity

A held terminal is a `TerminalId`, not an LfProcess: Session → TerminalId →
holder (build, pid) + PTY + child AgentProcess. A holder is one LfProcess
however many terminals it holds. Each Session's attachment records the holder
that holds it. The registry is live state the server rebuilds from holder
inventory, the per-terminal files below and the store; it is not a second
record of Work.

### Holder

`lf holder run` is hidden and started only by the server. It opens no store,
runs no async runtime and knows nothing about Sessions.

- **One per build.** Keyed by the executable's SHA-256: socket
  `<private dir>/holder-<sha12>.sock`, a lock file beside it, state under
  `<Machine home>/holders/<sha12>/<terminal>/`. The server sends starts only
  to the holder of its own build and speaks to every older one it finds.
- **Claim.** The holder takes an exclusive lock for life and only then
  unlinks and binds its socket. A second `lf holder run` of the same build
  fails the lock and exits 0. A holder that is slow to answer still holds the
  lock, so the server waits for it and never starts another in its place.
- **Start.** The server spawns it with `setsid` and reads one ready line.
  launchd (`AbandonProcessGroup`) and systemd (`KillMode=process`) leave it
  running when the server's service stops or is replaced.
- **Lifetime.** It exits 60 seconds after its last child, if no start
  arrives. Logout and reboot end it. Nothing restarts a holder, and a holder
  that dies takes its agents with it; the server's
  [recovery pass](../../../loopflow/src/server/README.md#recovery-pass)
  starts the Task work sessions among them again, in the current holder.
- **Ring.** The newest 2 MiB of raw output per PTY, addressed by absolute
  byte count since the child started. The holder drains every PTY whether or
  not a server is connected, so a stopped server never blocks an agent.
- **Exit.** When a child exits the holder reads what it left on the PTY,
  closes the PTY, and writes `<terminal>/exit.json` (code or signal, final
  byte count) before reporting it. It keeps the ring for readers until the
  server sends `forget`.

Requests and answers are one JSON object per line on a Unix socket
(`terminal_holder/wire.rs`). The protocol is small so that a newer server can speak to
any older holder, and no field has a default:

| Request | Answer |
|---|---|
| `hello {revision}` | `welcome {revision, build, pid}`, or `incompatible {revision}` to an older asker |
| `list` | `terminals [{terminal, request, pid, rows, cols, written, ring_start, exit}]` |
| `start {terminal, request, argv, cwd, env, rows, cols}` | `started {pid}`. `env` is complete; nothing is inherited from the holder. Repeating a `terminal` with the same `request` answers the existing child; with another `request` it is an error |
| `read {terminal, from}` | `reading`, then the connection is that terminal's output as frames |
| `write {terminal}` | `writing`, then the connection is that terminal's input as raw bytes |
| `resize {terminal, rows, cols}` | `done`. The foreground program is signalled even when the size is unchanged |
| `restore {terminal}` | `done`. The PTY returns to the modes it had before its child ran |
| `forget {terminal}` | `done`. An exited terminal's ring is dropped; a running one is an error |

Anything else answers `error {message}` and the connection keeps taking
requests. A `TerminalId` names one PTY and is never reused: a `read` of a
terminal the holder does not hold, or from a position past what that
terminal has written, is an error and never an answer from byte 0.

Revision 2 closes an unfinished bracketed paste when its writer disconnects.
A server reconnecting that writer restores only the paste's delimiters before
forwarding its unsent tail; content is never replayed. Older holders leave the
paste open, so their writers continue without restoring delimiters. Those
holders keep their original behavior until their conversations end.

Output frames are `tag u8 · length u32 BE · payload`:

| Tag | Payload |
|---|---|
| `o` | Raw PTY bytes continuing the reader's position |
| `g` | `{ring_start}`: the ring passed the reader; the stream continues there |
| `m` | `{raw}`: whether the PTY had line editing off when the following bytes were read. The server tells a provider's screen from its exit text by it |
| `x` | `{code, signal, at, written}`: the child's exit, last frame of the stream |

There is no signal request. The server signals a child's process group itself
(same user, pid from `list`).

A dropped holder connection says nothing about its agents. The server reads
again from its last position with backoff; a terminal ends only on an `x`
frame or an `exit.json`. A `start` whose answer was lost is reconciled with
`list` before anything is retried, so one request starts one child. A
terminal missing from `list` with no `exit.json` has an unknown outcome and is
reported as such, never given an invented exit code.

### The child in a held terminal

A Claude launcher hands the server what it would record at exit, then execs
into the provider, so Claude is the holder's direct child and no `lf -i` waits
beside it; the server records the provider's exit, settles its capture and
reports where the conversation moved. A Codex launcher does the same into its client, which talks to its app-server directly. The
server's lifetime worker checks that client's attachment every two seconds
and stops it when the conversation moves; a failed store read leaves it
running, and the byte-handling loop never waits for that read. An OpenCode
launcher becomes `opencode --standalone` the same way. The launcher drops the
pane’s `LF_RELAY_FOCUSED` before requesting a held terminal. The server strips `LF_SERVER` and
`LF_SERVER_STATUS` from what the holder starts, so an agent in a Desktop pane
runs like one opened from a CLI.

The child's own control requests (`status`, `directory`, `exit_record`, …)
enter the server at `held.sock` like a view. `LF_TRANSPORT_HELD` names its
terminal; the lf process that takes up that handoff records it as
`processes.held_terminal_id`. A child's control request that reads another
revision is a recording gap in `logs/lf.log`; the agent and its turn go on.

While a native terminal is open, Loopflow sends its own diagnostics and
background helper output to private `logs/terminal-PROCESS.log` files under
the selected Machine home (`~/.lf` by default). Each launcher prints its
location before handing over the terminal and reports newly recorded
diagnostics after returning. Native provider output keeps its terminal;
OpenCode keeps its separate provider stderr log.

### Views

Every connection to a held terminal enters the server at
`<private dir>/held.sock`: a first line `{"terminal":ID,"revision":N}`, the
answer `{"answer":"hosted","revision":N}`, then the [view
protocol](#view-protocol). That first exchange is the one thing every build
reads the same way; `revision` covers everything after it. A server of another
revision answers and admits nothing. A view in a terminal that reads another
revision becomes the installed `lf` (the relay, same Session and terminal) as
a fresh view, which repaints from the server's screen; no position crosses
builds. A view that already did so, or cannot, ends with one line and the
agent keeps running.

`"waiting"` means the terminal's holder has not answered this server yet: one
that keeps its lock and does not list in time at startup is pending, asked
again with backoff, and taken up when it answers; the view keeps
reconnecting. A refused connection does not end it (a stopped holder refuses
once its socket's queue is full); a holder whose lock is free is gone, and its
terminals and its LfProcess row settle with what its records say, else an
unknown outcome. `"ended"` means no live or pending holder could hold that
terminal: it ended or never started, and the connection is refused as
`not_found`.

The CLI relay (`lf session attach`, run by `lf resume` and never typed) takes
a terminal. When its connection drops it connects again on its own, with
backoff, until a server hosts the terminal or says it has ended. It draws
nothing while it waits; typing is discarded and Ctrl+] closes the view.
Desktop panes set `LF_SERVER=1` for their relay, which passes `--session` and
obtains a `session.attach` ticket for the Session's held terminal; nothing is
started on bridge failure. Clients that speak only the server protocol, such
as the phone, use the same ticket ([server protocol](../../../loopflow/src/server/README.md)).
The [server boundary](../../../../docs/dev/server.md) separates client access
from Machine routing.

A view counts PTY bytes, not bytes sent to it. Its `out` frames are exactly
the PTY's output; a `position` frame follows a snapshot and any stretch it
skipped (output the ring dropped before the server read it, or a native
client's exit text lf replaced); lf's reports and events travel in `aside`
frames, which it writes and does not count. The server keeps no spool.

A view that reconnects attaches with `from`, the position its screen shows, and
`focused`, the focus state it last had:

```json
{"id":1,"kind":"attach","rows":40,"cols":120,"name":null,"from":48213,"focused":false}
```

Sending its last known focus keeps a background terminal that reconnects on its
own from taking the size away from the view a person moved to.

The server answers a new viewer identity, reads the PTY bytes from `from`
out of the holder's ring on a connection of its own, and sends them as `out`
frames until the view reaches the live stream; this works after a server
restart too, since nothing about the view is kept in the server. Then it
sends the current state again (size, owner, theme, directory, title and
program status, a child exit) and a `live` event. A `from` the ring no
longer holds, or past what the PTY printed, gets a snapshot and `position`
instead, then `live`. Until it has caught up the view receives no reports or
events, and the server drops its terminal's answers: the retained screen
answers the agent's live queries meanwhile.

The replayed bytes contain queries the agent asked long ago, and the view's
terminal answers them again. The relay discards everything it reads from
its terminal until the stream is live, then writes a status report query
(`CSI 5 n`) and keeps discarding until the terminal's answer to it: answers
come in order, so every late one has arrived by then (one second at most if
a terminal never answers). Anything the relay itself prints into the screen
(a reconnect notice, an input hint) makes its next
reconnect a fresh attach that repaints instead. The replayed stretch is the
raw PTY output, including a native client's exit text that live views see
replaced.

### Server restart

Before `start` the server writes `<terminal>/meta.json`: the Session, the
start's `request`, and the control key the child's requests are checked
against. That is what lets another server instance answer the same child.
An exit record the launcher hands over is kept beside it in
`exit-record.json` until it is settled, and its native client's announcement
in `native-client.json` until that client ends. A launcher that execs its
provider never announces it again, so a server that takes the terminal up
reads it there; without it a Desktop pane would stay paused waiting for
readiness after a restart.

On startup the server greets every `holder-*.sock`, lists it, and hosts each
running terminal under its existing `TerminalId`. The screen is rebuilt by
replaying the holder's ring from its start, then one size nudge (one column
narrower for 0.4 s, then back to the size the terminal has by then) asks the
agent to repaint what the ring no longer holds. A view that takes the size
during the nudge keeps it. The same nudge follows a gap, when the ring passed output the
server had not read. Scrollback reaches only as far back as the ring, and a
terminal mode set before the ring's start is known only if the agent sets it
again when resized. Replayed output answers none of its queries: they were
answered when it was printed, or went unanswered while no server ran.

### Conversations an earlier build started

Conversations opened before held terminals ran under an `lf session serve`
owner of their own; no current build starts or attaches to one. `lf resume`
of such a Session, and Desktop opening its pane, restart it: the owner is ended
by its verified pid and birth, never by command pattern, and once it has
exited the thread resumes held from provider history. An owner that is already
gone resumes held at once. One that cannot be verified, or does not exit,
starts nothing; the message says it still runs under an earlier build and
names its pid. Nothing sweeps owners nobody resumes; they run until their
agents quit.

## Using a view

Ctrl+] detaches a view. Ctrl+C reaches the running program. A Task step's live
controls (`/status`, `/interrupt`, `/detach`, messages) are shown to each view
as it attaches; headless output never sees them. Closing a terminal leaves the
agent, its provider requests and capture settlement with the holder and
server; `lf session list` shows its Session as `detached` and `lf resume`
reattaches it. A held terminal takes any number of views and every attached
view types: `lf resume` here adds one more view and leaves the others open, and
a draft typed in one shows in all of them. `lf resume SESSION --json` prints
the Session record after a successful return, including detaching. Terminal
traffic does not wait for store reads; the final record is read after relaying
ends. When stdout is no terminal it holds only that record, and the terminal's
output goes to stderr. `lf resume SESSION --prepare --json` returns the one
command Desktop runs, without opening a socket or starting a provider. Desktop
decides from the record's actions whether showing a pane reattaches: a held
Session offers reveal; an ended Session waits for an explicit open.

`lf session rename SESSION 'Resume repair'` updates the attached terminal
title, including Rex and other terminals that support OSC titles. Reattaching
shows the current name. Updates wait for complete provider control sequences;
slow name lookups leave input and output running. cmux tabs and tmux windows
also follow the Session name, while cmux workspaces keep their Task name or a
hand-edited name.

Leaving a relay restores keyboard and drawing modes at the current cursor
position. Returning to the shell must not home the cursor or overwrite the
command's output.

During initial attachment, cooked input waits until the server acknowledges
the view. Input then goes straight to the holder's PTY; only bytes not yet written are
queued, and no store read or write delays forwarding. Returning does not
resubmit input. Unsent input is lost when the conversation is quit and
resumed.

An unfinished bracketed paste ends when its view leaves, and before another
view's keystrokes reach the agent. This releases the native parser's accumulated draft without submitting
it. A reconnect tracks paste delimiters even while dropping queued input and
restores paste framing before forwarding any remaining content. Ctrl+] inside
a paste is content; outside a paste it detaches the view. No paste timeout or
Return is added.
Held-terminal protocol revision 3 pairs this behavior with relays that restore
paste framing after a reconnect; a relay from another revision is replaced by
the selected installation before it attaches.
The entrance handshakes in `tests/transport_tests.rs` and
`tests/machine_commands.rs` pin this revision; update both when advancing it.

A local attachment waits up to five seconds for acknowledgement. Over command
pipes it allows sixty seconds for SSH setup and destination Session
resolution. Timeout or Ctrl+C while waiting closes only the view; execution
remains unknown. A view that is answered but then fails to attach leaves
delivery unknown and starts nothing. Heartbeat intervals and timeouts use
monotonic uptime, so laptop sleep does not expire a local connection.
Completed-Task cleanup counts wall time.

For commands using pipes, the view forwards stdin EOF after all accepted
input. Disconnecting the view alone leaves stdin open. Terminal
views keep the PTY open; the running program interprets terminal EOF keys.
Keystrokes a view sent before it closed reach the program in order, even when
the view quits in the same instant.

### Desktop status

Initial connection attempts keep the pane quiet. **Reconnecting…** follows
a previously acknowledged connection.
Connection feedback overlays the screen without changing the terminal's size;
the first retained screen uses the new pane's dimensions and cursor position.
The relay stays alive when its server connection drops. Its native status row
then shows **Reconnecting…** and pauses input while local selection, copy,
Find and scrolling remain available. Disconnected typing and uncertain writes
are never replayed. Eight attempts with capped backoff lead to **Session unavailable** and a native
**Retry connection** action. A final Close frame ends recovery and disables input;
its reason explains replacement, completion or retirement. Bare EOF remains unknown. Closing the
surface cancels recovery. Ordinary explicit Open remains separate.

The native surface listens on a private Unix socket and passes its path through
`LF_SERVER_STATUS` to its relay. Newline-delimited JSON readings carry
generation, state (`connecting`, `reconnecting`, `connected`, `unavailable`,
`ended`), native readiness, an optional message and the provider transition
for older clients. Desktop accepts input when the state is `connected` and
`native_ready` is true. The one action on the same stream is
`{"kind":"retry"}`, honored only while `unavailable`; lost actions are never
replayed. Closing the surface removes its private directory and cancels recovery. A
broken status stream pauses Desktop input immediately; the relay records the
gap, keeps the terminal running, and reconnects the status stream with its
current reading. Status I/O, including connect, is nonblocking. This is
presentation, not a viewer registry. Plain CLI relays keep their Ctrl+R/Ctrl+]
interaction.

### Screens

The server keeps a libghostty-vt terminal per held terminal (the Ghostty
revision pinned in `rust/loopflow-terminal/ghostty-vt.json`, 8 MiB of scrollback) and
answers the program's cursor, device and mode queries while no view is
attached to answer them. A new view is repainted through Ghostty's VT formatter:
the newest scrollback rows (up to 2,000, under 768 KiB) above its own
history, then the active screen with colors and styles, OSC 8 hyperlinks,
cursor position, Kitty keyboard stack, charsets, modes (alternate screen,
application keys, bracketed paste, mouse, focus), scrolling region, tabstops
and pwd. A restored screen first clears the previous view's input modes and
leaves its old alternate screen; a snapshot with default settings must not
inherit mouse reporting or bracketed paste from the previous program.
Program-defined palette, foreground, background and cursor colors survive
reattachment; default colors follow the view's theme. Detach and failed
resume reset program colors to that configured theme. Images are not
repainted.

When a held terminal ends the server writes
`<Machine home>/screens/<session>.ghostsnp`: a header line naming the Ghostty
revision, then Ghostty's snapshot stream with both screens, scrollback,
styles, hyperlinks, modes and unfinished terminal sequences. It does not
restore the provider's editor or pending requests. `lf resume` paints that
screen while the provider starts; the live terminal then takes over display
and input modes, including a blank screen. Startup failure or cancellation
restores the terminal; resume the same Session again. Snapshot format v1
promises no compatibility across revisions, so another revision's file reads
as absent.

Terminal hosts inherit a split's directory from a report (OSC 7) or the
foreground process's cwd. The server publishes one directory, the connected
provider's, then the Session's selected directory on each later attachment,
and tells each view to enter it with a `directory` event. A view that took
over a caller's terminal sends an empty report when it leaves, returning the
terminal to its shell.

Native providers own their Program Status (OSC 7501) reports, which pass
through unchanged. Claude's native title updates stay enabled because
disabling them also disables its status reports. Loopflow reports activity
for Graph progress and the command's result when it exits. It does not label
an open native conversation Working for the lifetime of its process. Quiet
output alone changes no status. The server answers status-support queries
while no view is attached and keeps the latest 64 records from providers and
lf together. Reattaching replaces the view's records with that current
status, including completed work and pending questions. Clear removes the
named record and its children; a shell prompt retires live claims, and a
terminal reset removes all records. Typing or pasting dismisses done and error
reports for every viewer and the Session. The same validated records are sent as
`program_status` on attachment and after changes, coalesced at complete
output boundaries. Mobile consumes this shape without parsing OSC reports
again. These snapshots confer no control or Task-completion authority; CLI
renderers continue consuming the original terminal sequences.

Desktop terminal placement is validated on the launching view's PTY, then
carried across to the held PTY before native admission. Stale inherited
placement is discarded. The first Session reading can publish reports already
restored on its surface; replacement providers begin with an empty
observation.

## View protocol

Every message is a frame: `tag u8 · len u32 BE · payload`. Stream frames carry
at most 64 KiB; a payload over 4 MiB is a protocol error.

| tag | direction | payload |
|---|---|---|
| `0x01` reply | server → view | JSON answering one request: `answers` (its `id`), `seq`, `kind` |
| `0x02` snapshot | server → view | the terminal snapshot a fresh stream begins with |
| `0x03` out | server → view | PTY output |
| `0x05` position | server → view | the PTY position the stream stands at, u64 BE |
| `0x06` aside | server → view | lf's reports and events; not PTY bytes |
| `0x07` close | server → view | final JSON `{reason: replaced\|finished\|retired}`; not PTY bytes |
| `0x11` request | view → server | JSON with `id` and `kind` |
| `0x12` input | view → server | keystrokes from any attached connection |

```json
{"id":1,"kind":"attach","rows":40,"cols":120,"name":"/dev/ttys004","from":null,"focused":true}
{"answers":1,"seq":0,"kind":"attached","viewer":"…","nonce":"…","terminal":true,"owner":"…"}
{"id":2,"kind":"focus","focused":true}
```

Before attachment, a destination may send a `snapshot` of retained history.
It grants no retention claim; the live snapshot replaces it on attachment. The
attached stream opens with the snapshot, a `resize` event, a `theme` event when
one is known, an `owner` event, and live output.

Events travel in `aside` frames as ``ESC _ lf1;<nonce>;<json> ESC \``, with
the view's own nonce, so a program printing the same bytes passes through as
output. Each carries a gapless `seq` per view, and every
reply carries the `seq` of the last event queued when it was computed. Kinds:
`resize {rows,cols}`, `theme {theme}`, `program_status {status}`,
`owner {viewer,name,held}`, `child_exited {code}`, `detached`, `hint {text}`,
`return {detach}` and `live`.

Every attached view can type. The sizing view supplies the PTY size and
colors. The `attached` reply names it in `owner`, and the `owner` event's
`held` field tells each recipient whether it owns the size. A view without
reported colors supplies no color override.

A focused attach, focus-in or typing takes the size. An unfocused attach
takes it only when there is no owner. When the owner leaves, the most
recently used live view takes over. Older relays omit `focused` on attach,
which counts as a focused attach.

A relay starts focused when stdin is a terminal unless given `--unfocused`
or `LF_RELAY_FOCUSED=0`. Desktop sets the variable in every Session pane.
The relay enables DEC mode 1004 to track focus across reconnects. Ghostty
reports its current focus when the mode turns on, so a focused pane takes
the size before the first key. The agent receives focus reports only when
it enables mode 1004 itself. Focus-out reaches it only from the sizing view.

Only the sizing view's terminal answers the agent's queries. The server drops
the other views' answers (cursor position, device attributes, OSC color
replies, mode reports) so the agent hears one answer.

A view sends `{"id":N,"kind":"controlled"}` every ten seconds on its attached
connection, independent of input and output, and gets `controlled {viewed,
ready}` back; after thirty seconds without one the server drops that view and
its retention claim. Silent network loss needs no EOF. The view also detects
thirty seconds without an answer or output, even when its writes still
succeed, and reconnects. Neither
starts a provider or replays input. Queued replies are read before checking
the deadline so a responsive, quiet provider stays attached after waking.

## Headless work

```sh
lf task run LOO-12 --json < /dev/null
# {"state":"running","graph_process":"…","log":"~/.lf/logs/graph-….log"}
ps -axo ppid,command | grep 'graph run'
# 1 lf --agent claude --task task_… graph run
```

Nothing hosts headless work (`os_process::detached`). Batches and direct
headless Graphs run with their caller. Task Graph workers, landing watchers
and delivery controllers that must outlive the command use `setsid`, stdin
from `/dev/null` and closed inherited descriptors except release locks and
worktree leases. Task steps themselves wait inside their Graph's process tree.

`LF_LAUNCHER_FD` carries the admitted LfProcess ID and a `released` message.
Server launches, landing watchers and run restarts record the ID and append
output to `logs/detached.log`. A pipe closing without an ID leaves delivery
unknown; nothing launches again on that uncertainty. Workers report turns and
read Task steers while running without a PTY, viewer or screen. Their own
process records their exit.

## Exit and settlement

When a terminal's `exit` arrives the server records it (`native_exit::settle`
with a record the launcher handed over, and the terminal's end in
`held_terminals`), asks the holder to `forget` the ring and deletes the
terminal's directory. The directory is the receipt: it is deleted only after
the store has the exit, so a store outage leaves it and settlement is tried
again every two seconds. A server that starts after a child exited finds the
exit in the holder's `list`, or in `exit.json` when the holder is gone too,
and records it the same way, settling the kept `exit-record.json` first; an
end nobody saw settles it as killed. A server that takes up a running
terminal takes up its kept record too.

A quit returns the provider's own exit to its views within a second; exit
records still waiting on a locked or refused store finish in the background or
are lost, logged.

Native Claude prompt/stop hooks keep silent waits, background tasks and
scheduled wakeups until Claude reports settlement. Terminal status counts only
while a view is attached to refresh it; an idle prompt cannot settle an
unfinished native turn. Headless runs never end for want of views.

Nothing else ends a conversation on its own. Removing, rebuilding or
replacing its lf executable leaves it running on the build it started with;
the only endings are the person quitting, Task closure, or an explicit end.
Wave and repository conversations run until their agent quits. `lf resume`
reopens provider history without starting a turn.

## Task shells and completed workspaces

Desktop creates a shell once, on the Task's recorded Machine, in a terminal
that Machine's holder keeps. The shell is plain: no hooks, no steers, and an
`lf` typed in it starts a chat. Its private runtime in `desktop::shells` owns
execution receipts, startup, reconnect and Close. A Linux Machine runs the
same server and holder under systemd; no Desktop app is needed there. Machine
routing carries the same view protocol across SSH. There is no Task shell
command or external-host shell registry.

Attachment, Hide and Desktop exit leave the shell running. Creation consumes
initial argv and Desktop's supplied integration once; reconnect never replays
startup. Close and Task cleanup end a shell through the server: it sends the
held terminal a close notice and, off its loop, kills what the shell left in
its OS session (ordinary foreground, background and stopped jobs, even in
separate process groups; stragglers get three seconds), then records
`jobs_settled`. Deliberately daemonized processes that created a different OS
session are outside the shell's ownership. Ended shells keep their identity
and saved screen. Undo restores that history; New Shell deliberately starts
different execution. An unavailable destination never opens a local successor
prompt.

Keep a Task Session view open to retain its worktree. Hidden or unfocused
views count. Closing the last
view starts a completed Task's cleanup grace; requested work delays cleanup
without restarting that clock. A shell does not retain the worktree.
Loopflow cleans up its Desktop shells; Rex shells remain yours to manage in
Rex. Task and conversation history remain after cleanup.

Viewing is dated on the held terminal (`held_terminals.viewer_hold`,
`viewer_hold_at`). Only Task Session views retain a checkout; Desktop shells
already accompany those Sessions and add no hold. Losing the last connection
starts the absence clock immediately. A resumed view cancels cleanup.

Local and remote views use the same protocol. SSH carries the frames through
the destination's relay, which forwards stdin/stdout to its server's
`held.sock`. The bridge creates no PTY, Session or heartbeat. An ordinary
external SSH connection uses its host's connection-loss detection; lf's
heartbeat applies to its own remote path.

Leaving Desktop's warm set, window close and orderly Quit send Ctrl+] to
release their relays. Quit waits up to two seconds for relay exit;
unavailable delivery falls back to connection-loss cleanup. A dated viewing
observation expires if the server crashes, so a stale row cannot retain
a completed workspace indefinitely. The Machine's scheduled cleanup pass
reconciles viewed Tasks with that same absence window.

A done Task ends its eligible conversations after fifteen minutes without
viewing, independently of checkout retirement. The grace starts no earlier than
completion; a newly resumed, never-viewed conversation starts it at launch. A
returning view cancels closure; leaving again starts a new grace. The cleanup
pass checks current Task membership, completion and viewing, then ends each
held terminal through the server on its notice. A view returning between that
decision and the end sees “Task completed” and can resume from history.
Abandonment ends unviewed Task conversations, even during a turn. Requested
active work, pending requests and running Graphs still hold execution; their
completion does not restart a deadline. Store failures cannot authorize
closure. Returning after cleanup continues saved provider history without
reopening the Task, and the same closure rule applies even after its checkout
has been retired.

Checkout and Desktop shell cleanup use the same fifteen-minute viewing grace.
Renewed viewing resets the checkout deadline before the next sweep. Completion
and PR landing schedule one retirement pinned to the completion, checkout and
branch; reopen or changed placement revokes it. Installation provisions a
separate launchd/cron job running `lf task retire` once a minute. Each pass
finds closed Tasks with running conversations as well as the local retirement
queue, reconciles views, attempts eligible cleanup and exits. No Task owns a
waiting cleanup process. Eligibility normally reaches a pass in 15–16 minutes
while awake; wall-clock deadlines survive sleep and reboot. Destination
servers confirm shell job settlement before deletion; a failed cleanup is
retried on a later pass. Abandoned local checkouts also remain in the same
retry queue after their last conversation exits, until cleanup succeeds or
the Task reopens. Completed-Task cleanup discards uncommitted files but keeps
primary/persistent checkouts and changed committed refs. Abandoned checkout
retries keep dirty files; an earlier `--force` applies only to that
invocation. Failures record a reason and retry after one minute, doubling up
to an hour independently of the absence deadline. One failed Task does not stop
other queued Tasks. Overlapping passes serialize, and interrupted effects can be
retried. `lf task retire <task>` runs one pass for a specific Task. Reopening a
retired Task never silently recreates a checkout.

Desktop's private [shell runtime](../desktop/README.md) carries startup
resources on creation; the view transport does not select terminal
integration.

Desktop keeps views open for visible Tasks and a bounded recent set of
unfinished conversations, so those hold their checkouts; leaving the set,
Close and window close detach them.

Desktop opens a conversation the way `lf resume` does: it attaches one more
view beside any already open, resumes an ended provider from history, and
takes over a Session whose provider runs outside a held terminal, in a native
client lf cannot share. Try again on a consumed first launch gives its request
another attempt. The live relay announces native input readiness to Desktop after
the server acknowledges the native client; saved screen bytes never provide
that announcement. A successful automatic attach records its Desktop terminal
association after the handshake.

Terminal parsing and repainting live in
[`loopflow-terminal`](../../../loopflow-terminal/README.md). The application
owns screen-file persistence and viewer lifetime; the terminal library owns
snapshot bytes, native query replies and display state.
