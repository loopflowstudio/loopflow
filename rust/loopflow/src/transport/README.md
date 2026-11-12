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
is the model: a launch runs here, held or detached, and hooks follow Work, not
the place. This README owns the holder and view protocols, headless launches,
exit settlement and Desktop shells.

## Held terminals

A held conversation has no process of its own. The Machine server asks a
holder to start the agent on a PTY and keeps everything else: the terminal
registry, one libghostty-vt terminal per held terminal, views, viewing
records, queued input, the child's control requests and exit settlement.

`transport::delegate` chooses the place once:

| Launch | Runs |
|---|---|
| `lf task run` or an interactive `lf ship` step at a terminal, `lf resume --task`, a Task's primary conversation, Wave session commands | held |
| A repository's conversation (`lf resume` in a checkout with no Task), from a terminal or a Desktop pane | held |
| `lf resume SESSION` of a Session bound to a Task or Wave | held |
| A bare Session opened through a Desktop pane or a framed `lf --machine` view | held, with the two bare hooks |
| Desktop Task shells | held, with no hooks |
| `lf <skill>`, `lf graph …`, `lf -i` at a terminal; a repository conversation recorded before it was managed (no `work_source`) | here, as the terminal's foreground program |
| A bare `-b` or piped run, every headless Graph step | here, as the caller's child |
| Graph launches, landing watchers, `lf ship`, a `-b` or piped Work command | [detached](#headless-work) |

A held launch calls `session.start` and becomes a view of the returned
terminal. With the server unreachable it fails with `Loopflow server is not
running; run lf server status` and starts nothing in its place. The one launch
nobody views, a Task's background conversation, goes unstarted instead and its
Graph runs on.

In a terminal the view is a small `lf` relay, so a host that shows the
foreground program, such as Rex, shows `lf` there, not the agent. Attaching
never creates: a view of a Session whose terminal is gone says so and offers
resume.

### Which launches are managed

A launch is managed when it comes through a Task, Wave or repository
conversation command, including those Desktop opens. `--task` and `--wave` (a
Task or Wave conversation passes them, and `lf task run` passes `--task` to
its Graph's agent steps) bind it as `WorkSource::Declared`, and the launch
carries that as `Holder::Server` (`AgentConfig::holder`). Beside a Session id,
`--task`/`--wave` make the resume managed and the id still selects: Desktop's
Task and Wave workspaces prepare with `lf session resume [ID] --task X
--prepare`, and the `open_argv` it gets back is `lf session resume ID --task
X`. Reopening a recorded conversation by id alone names no Work, so there the
Session row decides: `SqliteStore::is_bare` is false for `declared` or `bound`
Work and for a scope's primary conversation.

A bare `lf` typed inside a Task's checkout stays bare and records the Task as
its ancestry; its usage and time still count toward the Task. So do a child
run started by a Session's agent, which that agent steers, and plain `lf
<skill>`. Desktop adds no mark: an agent started from a Desktop shell is bare.
Design the bare path for subagents started by a Task's agent, not a person at
a prompt: nobody watches, the caller waits on the exit and reads the output,
and the run ends with its parent.

### Bare conversations at a terminal

Bare conversations typed at a terminal (`lf`, `lf <skill>`, or `lf resume` of
an unmanaged Session) run as the terminal's foreground process-group leader
with only the context hooks, and closing the terminal ends them.
`os_process::foreground` hands the terminal over before exec and takes it back
after; a provider stopped with Ctrl+Z stops lf too, and `fg` continues both.
Opening a bare Session somewhere else starts a new process in the same Session
from the provider's history; a process still open elsewhere is not one `lf`
stops.

### Identity

A held terminal is a `TerminalId`, not an LfProcess: Session → TerminalId →
holder (build, pid) + PTY + child AgentProcess. A holder is one LfProcess
however many terminals it holds, and `lf process list` shows it once. Launcher
LfProcess rows and capture attachment keep describing the processes that
really ran. The registry is live state the server rebuilds from holder
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
  that dies takes its agents with it.
- **Ring.** The newest 2 MiB of raw output per PTY, addressed by absolute
  byte count since the child started. The holder drains every PTY whether or
  not a server is connected, so a stopped server never blocks an agent.
- **Exit.** When a child exits the holder reads what it left on the PTY,
  closes the PTY, and writes `<terminal>/exit.json` (code or signal, final
  byte count) before reporting it. It keeps the ring for readers until the
  server sends `forget`.

Requests and answers are one JSON object per line on a Unix socket
(`server/holder/wire.rs`). The protocol is small so that a newer server can speak to
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
reports where the conversation moved. Interactive Graph steps keep their
launcher so each step settles before the Graph continues. A Codex launcher
does the same into its client, which talks to its app-server directly. The
server's lifetime worker checks that client's attachment every two seconds
and stops it when the conversation moves; a failed store read leaves it
running, and the byte-handling loop never waits for that read. OpenCode keeps
its launcher beside its private server. The server strips `LF_SERVER` and
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
as the phone, use the same ticket ([server protocol](../server/README.md)).
The [server boundary](../../../../docs/dev/server.md) separates client access
from Machine routing.

A view counts PTY bytes, not bytes sent to it. Its `out` frames are exactly
the PTY's output; a `position` frame follows a snapshot and any stretch it
skipped (output the ring dropped before the server read it, or a native
client's exit text lf replaced); lf's reports and events travel in `aside`
frames, which it writes and does not count. The server keeps no spool.

A view that reconnects attaches with `from`, the position its screen shows:

```json
{"id":1,"kind":"attach","control":true,"replace":false,"own":"request","rows":40,"cols":120,"name":null,"moved":null,"from":48213}
```

The server answers a new viewer identity, reads the PTY bytes from `from`
out of the holder's ring on a connection of its own, and sends them as `out`
frames until the view reaches the live stream; this works after a server
restart too, since nothing about the view is kept in the server. Then it
sends the current state again (size, owner, theme, directory, title and
program status, a child exit) and a `live` event. A `from` the ring no
longer holds, or past what the PTY printed, gets a snapshot and `position`
instead, then `live`. Until it has caught up the view receives no reports or
events, and its terminal's answers do not count as the controller's: the
retained screen answers the agent's live queries meanwhile.

The replayed bytes contain queries the agent asked long ago, and the view's
terminal answers them again. The relay discards everything it reads from
its terminal until the stream is live, then writes a status report query
(`CSI 5 n`) and keeps discarding until the terminal's answer to it: answers
come in order, so every late one has arrived by then (one second at most if
a terminal never answers). Anything the relay itself prints into the screen
(a reconnect notice, an input hint, an ownership change) makes its next
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
reads it there; without it a Desktop pane would stay **Opening…** after a
restart.

On startup the server greets every `holder-*.sock`, lists it, and hosts each
running terminal under its existing `TerminalId`. The screen is rebuilt by
replaying the holder's ring from its start, then one size nudge (one column
narrower for 0.4 s, and back) asks the agent to repaint what the ring no
longer holds. The same nudge follows a gap, when the ring passed output the
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
controls (`/status`, `/interrupt`, `/detach`, messages) are shown to each
controlling view as it attaches; headless output never sees them. Closing a
terminal leaves the agent, its provider requests and capture settlement with
the holder and server; `lf session list` shows its Session as `detached` and
`lf resume` reattaches it. Attaching takes control: the previous view's input
token is revoked and that terminal says where the conversation went.
`lf resume SESSION --json` prints the Session record after a successful
return, including detaching or moving to another view. Terminal traffic does
not wait for store reads; the final record is read after relaying ends.
`lf resume SESSION --prepare --json` returns the one command Desktop runs,
without opening a socket or starting a provider. Desktop decides from the
record's `state` whether showing a pane reattaches: a `detached` Session
reattaches, an `active` one waits for **Move here**, and an ended one waits for
an explicit open.

`lf session rename SESSION 'Resume repair'` updates the attached terminal
title, including Rex and other terminals that support OSC titles. Reattaching
shows the current name. Updates wait for complete provider control sequences;
slow name lookups leave input and output running. cmux tabs and tmux windows
also follow the Session name, while cmux workspaces keep their Task name or a
hand-edited name.

Leaving a relay restores keyboard and drawing modes at the current cursor
position. Returning to the shell must not home the cursor or overwrite the
command's output.

During initial attachment, cooked input waits until the server grants control.
Input then goes straight to the holder's PTY; only bytes not yet written are
queued, and no store read or write delays forwarding. Returning does not
resubmit input. Unsent input is lost when the conversation is quit and
resumed.

A local attachment waits up to five seconds for acknowledgement. Over command
pipes it allows sixty seconds for SSH setup and destination Session
resolution. Timeout or Ctrl+C while waiting closes only the view; execution
remains unknown. A view that is answered but then fails to attach leaves
delivery unknown and starts nothing. Heartbeat intervals and timeouts use
monotonic uptime, so laptop sleep does not expire a local connection.
Completed-Task cleanup counts wall time.

For commands using pipes, the controlling view forwards stdin EOF after all
accepted input. Disconnecting the view alone leaves stdin open. Terminal
views keep the PTY open; the running program interprets terminal EOF keys.
Keystrokes a view sent before it closed reach the program in order, even when
the view quits in the same instant, as long as it still held control.

### Desktop status

Desktop shows **Opening… Input paused** until attachment and native input
readiness are acknowledged. Initial retries keep that label;
**Reconnecting…** follows a previously acknowledged connection. Workspace
preparation uses the same display and keeps its last available screen.
Connection feedback overlays the screen without changing the terminal's size;
the first retained screen uses the new pane's dimensions and cursor position.
The relay stays alive when its server connection drops. Its native status row
then shows **Reconnecting…** and pauses input while local selection, copy,
Find and scrolling remain available. Disconnected typing and uncertain writes
are never replayed. If another view took control, recovery observes it
without replacing input or geometry. Choose **Move here** in the read-only
pane's connection row to edit the same running Session; input stays paused
until the server acknowledges, and a lost reply never repeats that takeover.
Eight attempts with capped backoff lead to **Session unavailable** and a native
**Retry connection** action. Only a child exit means ended. Closing the
surface cancels recovery. Ordinary explicit Open remains separate.

The native surface listens on a private Unix socket and passes its path through
`LF_SERVER_STATUS` to its relay. Newline-delimited JSON readings carry
generation, state, input capability, native readiness and optional
explanation. The same stream carries `{"kind":"retry"}` or
`{"kind":"move","generation":N}`. A move is consumed once before its
replacing attachment; stale generations and lost actions are never replayed.
Closing the surface removes its private directory and cancels recovery. A
broken status stream pauses Desktop input immediately; the relay records the
gap, keeps the terminal running, and reconnects the status stream with its
current reading. Status I/O, including connect, is nonblocking. This is
presentation, not a viewer registry. Plain CLI relays keep their Ctrl+R/Ctrl+]
interaction.

### Screens

The server keeps a libghostty-vt terminal per held terminal (the Ghostty
revision pinned in `rust/loopflow/ghostty-vt.json`, 8 MiB of scrollback) and
answers the program's cursor, device and mode queries while no view's
terminal is there to. A new view is repainted through Ghostty's VT formatter:
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
terminal reset removes all records. The same validated records are sent as
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
| `0x11` request | view → server | JSON with `id` and `kind` |
| `0x12` input | view → server | keystrokes from the connection holding input control |

```json
{"id":1,"kind":"attach","control":true,"replace":true,"own":"take","rows":40,"cols":120,"name":"/dev/ttys004","moved":null,"from":null}
{"answers":1,"seq":0,"kind":"attached","viewer":"…","nonce":"…","token":"…","terminal":true,"owner":"…"}
```

Before attachment, a destination may send a `snapshot` of retained history.
It grants no input or retention claim; the live snapshot replaces it on
attachment. The attached stream opens with the snapshot, a `resize` event, a
`theme` event when one is known, an `owner` event, and live output.

Events travel in `aside` frames as ``ESC _ lf1;<nonce>;<json> ESC \``, with
the view's own nonce, so a program printing the same bytes passes through as
output. Each carries a gapless `seq` per view, and every
reply carries the `seq` of the last event queued when it was computed. Kinds:
`resize {rows,cols}`, `theme {theme}`, `program_status {status}`,
`owner {viewer,name,held}`, `control {held,notice}` (input control left this
connection), `child_exited {code}`, `detached`, `hint {text}`,
`return {detach}` and `live`. A view that receives `control` with
`held: false` restores its terminal, prints the notice and leaves.

Input control and geometry release when a connection ends; a reconnecting view
asks for them again. Another view's input or geometry claim survives, and
closing a stale socket later cannot revoke a replacement connection's claims.

Geometry ownership arbitrates only size and colors. `own: "take"` replaces the
geometry owner, who receives `owner` with `held: false` and stays attached;
`own: "request"` queues until the owner leaves. A view may report its size and
colors before it owns them; only the selected owner's reports are broadcast as
effective appearance, and its known size and colors follow ownership. A view
that has not reported colors supplies no color override. Who may type is
separate: the `token` of a controlling attach, `busy` when control is held and
`replace` is false, and a `control` event to the connection that lost it.

A view sends `{"id":N,"kind":"controlled"}` every ten seconds on its attached
connection, independent of input and output; after thirty seconds without one
the server releases that connection's retention and input claim. Silent
network loss needs no EOF. The view also detects thirty seconds without an
answer or output, even when its writes still succeed, and reconnects. Neither
starts a provider or replays input. Queued replies are read before checking
the deadline so a responsive, quiet provider stays attached after waking.

## Headless work

```sh
lf task run LOO-12 --json < /dev/null
# {"state":"running","graph_process":"…","log":"~/.lf/logs/graph-….log"}
ps -axo ppid,command | grep 'graph run'
# 1 lf --agent claude --task task_… graph run
```

Nothing hosts headless work (`os_process::detached`). A bare `-b` or piped
execution, and every headless Graph step, runs in the caller's process and its
agent is that process's child, so it ends with its caller and its stdout and
stderr are the caller's. Work that must outlive its caller (Graph launches,
landing watchers, `lf ship`, a `-b` or piped Work command) is spawned with
`setsid`, stdin from `/dev/null` and every inherited descriptor closed except
release locks and worktree leases. It gets one more pipe, named by
`LF_LAUNCHER_FD`: the child writes its LfProcess ID there once admitted, and
`released` when it lets its launcher go. Server launches, landing watchers and
ship restarts read the ID and append the child's output to
`logs/detached.log`; a pipe that closes without an ID leaves delivery unknown,
and nothing is launched again. A `-b` or piped Work command prints the child's
stdout and stderr as they grow and exits with its status, or with success
once a Task run or ship lets it go after naming its Graph log. Killing that
caller leaves the work running.

No PTY, view protocol or screen is involved. Hooks, turn recording and
steers work as they do for any headless run; the child's own process records
its exit.

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
`lf` typed in it is bare. Its private runtime in `desktop::shells` owns
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
views count, passive ones too; input control is separate. Closing the last
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

Desktop's automatic opening uses a non-replacing input claim: working Tasks use
Start, which permits first-thread creation; other Work uses reveal, which
requires existing history. Their prepared commands keep that distinction
through launch. If another view wins before attachment, the server returns
Busy and keeps that view's input. Only Take back uses ordinary resume to take
control; Try again on a consumed first launch uses it on a Session no client
holds. The live relay announces native input readiness to Desktop after the
server acknowledges the native client; saved screen bytes never provide that
announcement. A successful automatic attach records its Desktop terminal
association after the handshake, keeping current provider status without
claiming another view's input during preparation.
