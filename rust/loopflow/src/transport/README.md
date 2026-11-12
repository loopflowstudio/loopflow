# Retain execution across terminal loss

```sh
lf resume SESSION
lf --machine build resume SESSION
```

Ctrl+] detaches a viewer. Ctrl+C in an attached terminal reaches the running
program. A Task step's live controls (`/status`, `/interrupt`, `/detach`,
messages) are shown to each controlling viewer as it attaches; headless output
and observers never see them. Closing a terminal leaves execution, provider requests and capture
settlement with the detached owner; `lf session list` shows its Session as
`detached` and `lf resume` reattaches it. Attaching takes control: the previous
viewer's input token is revoked and that terminal says where the conversation went.
`lf resume SESSION --json` prints the Session record after a successful return,
including detaching or moving to another viewer. The live relay opens no store;
it reads the final record only after relaying ends.
`lf resume SESSION --prepare --json` returns the one command Desktop runs, without
opening a socket or starting a provider. Desktop decides from the record's `state`
whether showing a pane reattaches: a `detached` Session reattaches, an `active` one
waits for Move here, and an ended one waits for an explicit open.
Preparing asks a running owner whether a viewer holds control, so an owner from an
earlier lf, which cannot record its viewers in a store a newer lf advanced, reads
`detached` once Desktop restarts and its pane reattaches. An owner that is running
but never answers reads `detached` too.

`lf session rename SESSION 'Resume repair'` updates the attached terminal title,
including Rex and other terminals that support OSC titles. Reattaching shows the
current name. Updates wait for complete provider control sequences; slow name
lookups leave input and output running. cmux tabs and tmux windows also follow the
Session name, while cmux workspaces keep their Task name or a hand-edited name.

Desktop shows **Opening… Input paused** until attachment and native input readiness
are acknowledged. Initial retries keep that label; **Reconnecting…** follows a
previously acknowledged connection. Workspace preparation uses the same display
and retains its last available screen.
Connection feedback overlays the screen without changing the terminal's size;
the first retained screen uses the new pane's dimensions and cursor position.
The server relay stays alive when its service connection drops. Its native
status row then shows **Reconnecting…** and pauses input while local selection, copy,
Find and scrolling remain available. Recovery refreshes the same owner's screen;
byte streams resume from their received position or refresh when output expires.
Disconnected typing and uncertain writes are never replayed. If another viewer
took control, recovery observes it without replacing input or geometry. Choose
**Move here** in the read-only pane's connection row to edit the same running
Session. Input stays paused until its owner acknowledges; a lost reply never
repeats that takeover. Eight attempts with capped backoff lead to **Session
unavailable** and a native **Retry connection** action. Only an owner exit event means ended.
Closing the surface cancels recovery. Ordinary explicit Open remains separate.

The native surface listens on a private Unix socket and passes its path through
`LF_SERVER_STATUS` to its relay. Newline-delimited JSON readings carry generation,
state, input capability, native readiness and optional explanation. The same
stream carries `{"kind":"retry"}` or `{"kind":"move","generation":N}`.
A move is consumed once before its replacing attachment; stale generations and
lost actions are never replayed. Closing the surface removes its private directory
and cancels recovery. A broken status stream pauses Desktop input immediately;
the relay records the gap, keeps the terminal running, and reconnects the status
stream with its current reading. Status I/O, including connect, is nonblocking.
This is temporary presentation, not a viewer registry or execution authority.
Plain CLI server relays retain their Ctrl+R/Ctrl+] interaction.

Local terminals reconnect directly to the same execution owner if their socket
closes or stops answering. Recovery never starts another provider or replays
typing; another viewer's input and geometry claims stay with it. If the first
reconnection fails, Ctrl+R retries and Ctrl+] closes the viewer. Initial attachment
failures and command pipes still return their errors.

Leaving a terminal relay restores keyboard and drawing modes at the current
cursor position. Returning to the shell must not home the cursor or overwrite
the command's output.

Conversations stay running while their scope is open, whether an owner or a
held terminal hosts them. Completed Tasks end their conversations after the
viewing grace, with or without a checkout; abandonment ends unviewed Task
conversations, even during a turn. Wave and repository conversations run
until their agent quits. Task cleanup signals an owner after its notice; a held
terminal has no process to signal, so the server ends it on the notice.

Bare conversations typed at a terminal (`lf`, `lf <skill>`, or `lf resume` of
an unmanaged Session) start no owner: the provider runs as the terminal's
foreground process-group leader with only the context hooks, and closing the
terminal ends it. `os_process::foreground` hands the terminal over before exec
and takes it back after; a provider stopped with Ctrl+Z stops lf too, and `fg`
continues both. Commands naming Work and reopens of managed Sessions keep the
holder. Headless runs, bare Graphs, and Desktop surfaces waiting for a relay
keep their host.

`lf task run`, `lf ship`, and explicit checkout start an unopened Task conversation
detached beside its Graph; `lf resume` reattaches it. Preparation alone does not
launch it. Background starts require a configured, installed agent; there is no
conversation count cap.

Native Claude prompt/stop hooks preserve silent waits, background tasks and
scheduled wakeups until Claude reports settlement. Terminal status counts only
while a viewer is attached to refresh it; an idle prompt cannot settle an
unfinished native turn. Headless Runs never end for want of viewers.

During initial attachment, cooked input waits until the owner grants control.
Input then goes straight to the provider. The owner queues only bytes not yet written
to its terminal; no store read or write delays forwarding. Returning does not
resubmit input. Unsent input is lost when an explicit rollover or takeover
replaces its provider, just as on quit.

A quit returns the provider's own exit within a second; exit records still
waiting on a locked or refused store finish in the background or are lost, logged.

Nothing else ends an owner on its own. Removing, rebuilding or superseding its
lf executable leaves it serving on the build it started with; the only endings
are the person quitting, `lf resume` taking the conversation over, `lf process
rollover`, Task closure, or an explicit end. Owners record viewing changes and
retry failed writes every two seconds. Once recorded, a conversation waits for
the next change; only shell owners keep polling their explicit Close receipt.
Task cleanup decides automatic closure on its scheduled pass. Repeated unchanged
store errors produce one warning until the condition changes; viewing launches
no helper process.
Cleanup sends a best-effort notice and saves the screen, then signals the verified
owner through the same path as rollover. Old owners that cannot receive the notice
still end by signal.
`lf resume` reopens provider history without starting a turn. Other ended
Sessions stay stopped.

`lf resume` replaces an idle owner that is superseded or unanswering, continuing
its saved provider thread without replaying input. Pending requests, running
turns, Graphs and Runs keep their owner even when its socket does not answer. Resume
attempts attachment; a failed attachment leaves that work intact. Saved history cannot
restore a native editor or pending request. No open Session is unknown, not work: the
owner stays on its own and an answering one is attached, but `lf resume` ends a
superseded one that does not answer. Once protected work settles, another resume
moves it.

`lf process rollover --dry-run` lists the same takeover choices across owners;
omit `--dry-run` to move them. Protected owners stay, answering or not. The sweep
ends idle owners directly; existing viewers may close and need `lf resume` again.
Rollover names the same retention reasons. Resume, rollover and Task retirement
share this judgment; work holds independent of reachability. Only resume asks
whether the owner answers, and only for no open Session.
Graph runners keep running and launch each next step through the entry.

`lf resume` reconnects to a retained native terminal. A structured Run may require
an explicit transition to the provider's native UI. That transition preserves the
Session and saved conversation, but does not preserve its old in-flight turn or
resubmit its input.

An owner that answers but then fails to attach leaves delivery unknown; that
reconnection launches no other provider and replays no Graph. Finished Runs retain
results in Session history.
A restored screen first clears the previous viewer’s input modes and leaves its
old alternate screen, then applies the owner’s screen and modes. A snapshot with
default settings must not inherit mouse reporting or bracketed paste from the
previous program.

A local attachment waits up to five seconds for acknowledgement. Over command
pipes it allows sixty seconds for SSH setup and destination Session resolution. Timeout
or Ctrl+C while waiting closes only the viewer; execution remains unknown. Once
acknowledged, the attachment stays while the owner answers its heartbeats.
Heartbeat intervals and timeouts use monotonic uptime, so laptop sleep does not
expire a local connection. Completed-Task cleanup counts wall time.

For commands using pipes, the controlling viewer forwards stdin EOF after all
accepted input. Disconnecting the viewer alone leaves stdin open. Terminal
attachments keep their PTY open; the running program interprets terminal EOF keys.
Keystrokes a viewer sent before it closed reach the program in order, even when
the viewer quits in the same instant, as long as it still held control.

The owner keeps a libghostty-vt terminal (the Ghostty revision pinned in
`rust/loopflow/ghostty-vt.json`, 8 MiB of scrollback) and answers the program's
cursor, device and mode queries while no viewer's terminal is there to. A new viewer
is repainted through Ghostty's VT formatter: the newest scrollback rows (up to 2,000,
under 768 KiB) above its own history, then the active screen with colors and
styles, OSC 8 hyperlinks, cursor position, Kitty keyboard stack, charsets, modes
(alternate screen, application keys, bracketed paste, mouse, focus), scrolling
region, tabstops and pwd. Reattach while a program uses its alternate screen;
returning to its primary screen keeps the conversation and retained history.
Program-defined palette, foreground, background and cursor colors survive
reattachment; default colors follow the viewer's theme. Detach and failed resume
reset program colors to that configured theme. Images are not repainted.

When an owner ends it writes `<Machine home>/screens/<session>.ghostsnp`: a header
line naming the Ghostty revision, then Ghostty's snapshot stream with both screens,
scrollback, styles, hyperlinks, modes and unfinished terminal sequences. It does
not restore the provider's editor or pending requests. `lf resume` paints that
screen while the provider starts. The live owner takes over its display and input
modes, including a blank screen. Startup failure or cancellation restores the
terminal; resume the same Session again. A fresh owner without a saved preview
leaves the viewer's existing screen in place; mode-only output is still restored.
Snapshot format v1 promises no compatibility
across revisions, so another revision's file reads as absent.
Headless observation retains a bounded output tail and preserves stdout/stderr on
delivery. Slow viewers disconnect; execution continues draining.

## Viewer stream

One Unix socket per owner; one connection per viewer or control client. Every
message is a frame: `tag u8 · len u32 BE · payload`. Stream frames carry at most
64 KiB; a payload over 4 MiB is a protocol error.

| tag | direction | payload |
|---|---|---|
| `0x01` reply | owner → viewer | JSON answering one request: `answers` (its `id`), `seq`, `kind` |
| `0x02` snapshot | owner → viewer | the terminal snapshot a fresh stream begins with |
| `0x03` out | owner → viewer | stream 1: PTY output, or stdout of a piped command |
| `0x04` err | owner → viewer | stream 2: stderr of a piped command |
| `0x05` position | server → viewer | a held terminal's PTY position the stream stands at, u64 BE |
| `0x06` aside | server → viewer | lf's reports and events for a held terminal's view; not PTY bytes |
| `0x11` request | viewer → owner | JSON with `id` and `kind` |
| `0x12` input | viewer → owner | keystrokes from the connection holding input control |

A viewer's stream is the bytes of its snapshot, out and err frames in order. The
viewer counts them; the owner spools the last 1 MiB per viewer.

```json
{"id":1,"kind":"attach","control":true,"replace":true,"own":"take","rows":40,"cols":120,"name":"/dev/ttys004","moved":null}
{"answers":1,"seq":0,"kind":"attached","viewer":"…","reconnect":"…","nonce":"…","token":"…","terminal":true,"grace_ms":60000,"owner":"…"}
```

Before attachment, a destination may send a `snapshot` of retained history. It
grants no input or retention claim; the live snapshot replaces it on attachment.
The attached stream opens with the snapshot (pipes: the retained tail), a `resize`
event, a `theme` event when one is known, an `owner` event, and live output.

Events ride stream 1 as ``ESC _ lf1;<nonce>;<json> ESC \``, inserted only between
complete terminal sequences. The nonce is the viewer's own, so a program
printing the same bytes passes through as output. An event may span frames.
Pending events share the viewer's 4 MiB output queue limit even while waiting
for a terminal sequence to finish. Each carries a gapless `seq`
per viewer, and every reply carries the `seq` of the last event queued when it
was computed. Kinds: `resize {rows,cols}`, `theme {theme}`,
`owner {viewer,name,held}`, `control {held,notice}` (input control left this
connection), `child_exited {code}`, `detached`, `hint {text}` and
`return {detach}` and `live`. A viewer that receives `control` with `held: false` restores
its terminal, prints the notice and leaves.

A connection that ends without a `leave` request keeps its stream for 60 s.
`{"kind":"resume","reconnect":…,"received":N,…}` continues from byte `N` with no
snapshot; a byte the spool no longer holds answers `reconnect_required`, and the
viewer attaches afresh. Input control and ownership release when the connection
ends; a resume asks for them again. If the viewer's previous socket is still
connected, resume replaces that socket and takes only its own prior claims.
Another viewer's input or geometry claim survives. Closing the stale socket
later cannot revoke the replacement connection's claims.
On resume, either `own` value requests free ownership; taking another viewer's
geometry requires a separate `own` request or fresh explicit attachment.

Ownership arbitrates only size and colors. `own: "take"` replaces the owner,
who receives `owner` with `held: false` and stays attached; `own: "request"`
queues until the owner leaves. A viewer may report its size and colors before
it owns them. Reports are remembered through reconnect grace; only the selected
owner's reports are broadcast as effective appearance. When ownership changes,
its known size and colors follow it. A viewer that has not reported colors
supplies no color override; fresh observers receive only the selected owner's
known theme. Who may type is separate: the `token`
of a controlling attach, `busy` when control is held and `replace` is false,
and a `control` event to the connection that lost it.

An execution command delegates to a detached owner, which holds its PTY or pipes
and records an ordinary Process. Loopflow launches that owner itself; it has no
public command. The owner leaves the launcher's session and drops every
descriptor the launcher inherited (a terminal host's pipes to its other
children), since it outlives that host. Session rows name that Process; viewer
tokens and unwritten input remain in memory. A Claude conversation keeps one lf owner
and one relay per terminal showing it. `lf resume` resolves the Session,
then execs into `lf session attach OWNER`, which
opens no store while relaying and parses no commands. It carries the original
command's receipt through exec and records its result when relaying ends.
A Claude launcher under an owner hands
the owner what it would record at exit, then execs into the provider, so Claude
is the owner's direct child and no `lf -i` waits beside it; the owner records
the provider's exit, settles its capture and reports where the conversation
moved. Graph steps keep their launcher so each step settles before the Graph
continues. A Codex launcher does the same into its client, which talks to its
app-server directly. OpenCode keeps its launcher beside its private server.
The owner sleeps in `poll` on its sockets, its child's output and a wake pipe
that a blocking child wait and explicit shell Close write to. Native clients retain their original provider
channels, including pending request identities. Graph steps share their Graph's
terminal owner through explicit child handoff, retaining their own Process results.

While a native terminal is open, Loopflow sends its own diagnostics and background
helper output to private `logs/terminal-PROCESS.log` files under the selected
Machine home (`~/.lf` by default). Each launcher prints its location before handing
over the terminal and reports newly recorded diagnostics after returning. Native
provider output retains its original terminal; OpenCode keeps its separate
provider stderr log. Ordinary CLI output resumes when the client exits or the
viewer detaches. Read these logs from another terminal while editing a draft.

Desktop Session panes set `LF_SERVER=1` for their terminal relay. The
relay passes `--session` and obtains a `session.attach` bridge for its exact owner from the local client API server before speaking
this same owner protocol. No owner or provider is created on bridge failure.
Ordinary CLI attachments keep their direct socket path. The owner strips `LF_SERVER` and `LF_SERVER_STATUS`
from the program it runs, so an agent in a Desktop pane attaches like any CLI. See
[current server protocol](../server/README.md) for admission and framing.
The [server boundary](../../../../docs/dev/server.md) separates
client access from Machine routing. `lf session serve` remains the execution-owner
entry, distinct from the server serving entry.

Terminal hosts inherit a split's directory from a report (OSC 7) or the foreground
process's cwd. The owner publishes one directory, the connected provider's, then
the Session's selected directory on each later attachment. It reports that
through the stream and tells each viewer to enter it with a `directory` event.
Viewer and owner share a Machine, so both sources agree even when a person launched
from elsewhere. A viewer that took over a caller's terminal sends an empty report
when it leaves, returning the terminal to its shell.

Native providers own their Program Status (OSC 7501) reports, which pass through
unchanged. Claude's native title updates stay enabled because disabling them also
disables its status reports. Loopflow reports activity for structured Runs and Graph progress, and
the command's result when it exits. It does not label an open native conversation
Working for the lifetime of its process. Quiet output alone changes no status.
The owner answers status-support queries while detached and retains the latest
64 records from providers and lf together. Reattaching replaces the viewer's
records with that current status, including completed work and pending questions.
Clear removes the named record and its children; a shell prompt retires live
claims, and a terminal reset removes all records.

Desktop terminal placement is validated on the launching viewer’s PTY, then carried
across the owner’s private PTY before native admission. Stale inherited placement
is discarded. The first Session reading can publish reports already restored on
its surface; replacement providers begin with an empty observation.

## Held terminals

```sh
lf resume --task LOO-12     # a view of a terminal the holder keeps
ps -axo command | grep '^lf '
# lf server run
# lf holder run
```

A managed launch has no owner process. The Machine server asks a holder to
start the agent on a PTY and keeps everything else: the terminal registry,
one libghostty-vt terminal per held terminal, viewers, viewing records, queued
input, the child's control requests and exit settlement. Everything above
about viewers, control, ownership, lifetime and repainting holds for a held
terminal with "the server" in place of "the owner", except where this section
says otherwise. Owners remain for bare launches, piped and `-b` executions
and Desktop Task shells.

`transport::managed` is the one place that decides:

| Launch | Managed |
|---|---|
| `lf task run`, an interactive `lf ship` step, `lf resume --task`, a Task's primary conversation, Wave session commands | yes |
| A repository's conversation (`lf resume` in a checkout with no Task), from a terminal or a Desktop pane | yes |
| `lf resume SESSION` of a Session bound to a Task or Wave | yes |
| A repository conversation recorded before it was managed (no `work_source`) | no |
| `lf <skill>`, `lf graph …`, `lf -i`, any `-b` or piped execution | no |
| Desktop Task shells | no: they run without a server on headless Linux |

A managed launch calls `session.start` and becomes a view of the returned
terminal. With the server unreachable it fails with `Loopflow server is not
running; run lf server status`. It never falls back to a local owner. The
one launch nobody views, a Task's background conversation, goes unstarted
instead and its Graph runs on.

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
| `m` | `{raw}`: whether the PTY had line editing off when the following bytes were read. The server tells a provider's screen from its exit text by it, as an owner does with `tcgetattr` |
| `x` | `{code, signal, at, written}`: the child's exit, last frame of the stream |

There is no signal request. The server signals a child's process group itself
(same user, pid from `list`).

A dropped holder connection says nothing about its agents. The server reads
again from its last position with backoff; a terminal ends only on an `x`
frame or an `exit.json`. A `start` whose answer was lost is reconciled with
`list` before anything is retried, so one request starts one child. A
terminal missing from `list` with no `exit.json` has an unknown outcome and is
reported as such, never given an invented exit code.

### Views

Every connection to a held terminal enters the server at
`<private dir>/held.sock`: a first line `{"terminal":ID,"revision":N}`, the
answer `{"answer":"hosted","revision":N}`, then the viewer protocol above,
unchanged. That first exchange is the one thing every build reads the same
way; `revision` covers everything after it. A server of another revision
answers and admits nothing. A view in a terminal that reads another revision
becomes the installed `lf` (`lf session attach --terminal`, the same Session)
as a fresh view, which repaints from the server's screen; no position crosses
builds. A view that already did so, or cannot, ends with one line and the
agent keeps running. A child's control request that reads another revision
is a recording gap in `logs/lf.log`; the agent and its turn go on.
`"waiting"` means the
terminal's holder has not answered this server yet: one that keeps its lock
and does not list in time at startup is pending, asked again with backoff, and
taken up when it answers; the view keeps reconnecting. A refused connection
does not end it (a stopped holder refuses once its socket's queue is full);
a holder whose lock is free is gone, and its terminals and its LfProcess row
settle with what its records say, else an unknown outcome. `"ended"` means no
live or pending holder could hold that terminal: it ended or never started,
and the connection is refused as `not_found`. Attaching never starts anything. The
child's own control requests (`status`, `directory`, `exit_record`, …) use the
same entrance; `LF_TRANSPORT_HELD` names its terminal the way
`LF_TRANSPORT_OWNER` names an owner. A terminal is no process, so the lf
process that takes up that handoff records it as `processes.held_terminal_id`.
Rules scoped to an owner's descendants start from those processes instead:
Task retirement excuses the work beneath it.

`lf session attach --terminal TERMINAL` is the relay for a held terminal.
When its connection drops it connects again on its own, with backoff, until a
server hosts the terminal or says it has ended. It draws nothing while it
waits; typing is discarded and Ctrl+] closes the view. Clients that speak
only the server protocol reach the same terminals with a `session.attach`
ticket (`server/README.md`).

A held terminal's view counts PTY bytes, not bytes sent to it. Its `out`
frames are exactly the PTY's output; a `position` frame follows a snapshot
and any stretch it skipped (output the ring dropped before the server read
it, or a native client's exit text lf replaced); lf's reports and events
travel in `aside` frames, which it writes and does not count. The server
keeps no spool for it.

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

### Exit and settlement

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

## Task shells and completed workspaces

Desktop creates a shell once, on the Task's recorded Machine. Its private
runtime in `desktop::shells` owns execution receipts, startup, reconnect and
Close. It runs on headless Linux too, without an app server. Machine routing
carries the same viewer protocol across SSH; the transport owns the PTY.
There is no Task shell command or external-host shell registry.

Attachment, Hide and Desktop exit leave execution running. Creation consumes
initial argv and Desktop's supplied integration once; reconnect never replays
startup. Close ends ordinary foreground, background and stopped jobs, even when
job control assigned them separate process groups. Deliberately daemonized
processes that created a different OS session are outside the shell's ownership.
Ended shells retain their identity and saved screen. Undo restores that history;
New Shell deliberately starts different execution. An unavailable destination
never opens a local successor prompt.

Keep a Task Session viewer open to retain its worktree. Hidden or unfocused
viewers count, passive ones too; input control is separate. Closing the last
viewer starts a completed Task's cleanup grace; requested work delays cleanup
without restarting that clock. An ordinary shell does not retain the worktree.
Loopflow cleans up its Desktop shells; Rex shells remain yours to manage in Rex.
Task and conversation history remain after cleanup.

The serving owner dates its own viewer evidence on its Process (`viewer_hold`,
`viewer_hold_at`). Only Task Session viewers retain a checkout; Desktop shells
already accompany those Sessions and add no hold. Losing the last connection
starts the absence clock immediately, independently of the sixty-second stream
replay grace. A resumed viewer cancels cleanup. Ending an unviewed owner preserves
that absence time; its exit adds no new workspace grace.

Local and remote viewers use this same protocol. SSH carries the frames through
`lf session attach -- <ssh argv>`; the destination attachment forwards stdin/stdout
to the owner's socket. The bridge creates no PTY, Session or heartbeat. Only the
execution owner holds a PTY. The viewing endpoint sends the existing
`{"id":1,"kind":"controlled"}` query every ten seconds on its attached connection,
independent of input and output; after thirty seconds without
one the owner releases that connection's retention and input claim. Silent network
loss needs no EOF. Other connected viewers still hold. Attach/Resume restarts the
heartbeat deadline, and the same stream carries input, resize, ownership and Leave.
The viewer also detects thirty seconds without an answer or output, even when
its writes still succeed. A server-backed viewer reconnects to the same owner;
other viewers report an unknown outcome and close. Neither starts a provider or
replays input. Wall time includes sleep; queued replies are read before checking
the deadline so a responsive, quiet provider stays attached after waking.
An ordinary external SSH connection uses its host's connection-loss detection; LF's bounded heartbeat applies to its own remote path.

Leaving Desktop’s warm set, window close and orderly Quit send Ctrl+] to release their
relays. Quit waits up to two seconds for relay exit; unavailable delivery falls
back to connection-loss cleanup. Only actual Session viewing retains the checkout.
An owner's dated observation expires too if it crashes, so a stale Process row
cannot retain a completed workspace indefinitely. The Machine's scheduled cleanup
pass also reconciles viewed Tasks and uses that same absence window after owner loss.

A done Task ends its eligible conversations after fifteen minutes without
viewing, independently of checkout retirement. The grace starts no earlier than
completion; a newly resumed, never-viewed owner starts it at launch. A returning
viewer cancels closure; leaving again starts a new grace. The cleanup pass checks
current Task membership, completion and viewing, then signals each verified
owner directly. Owners record viewing; they do not judge Task closure or poll
automatic requests. A viewer returning between that decision and the signal sees
“Task completed” and can resume from history. Requested active work,
pending requests and running Graphs still hold execution; their completion does
not restart a deadline. Store failures cannot authorize closure. Returning after
cleanup continues saved provider history without reopening the Task, and the same
closure rule applies even after its checkout has been retired.

Checkout and Desktop shell cleanup use the same fifteen-minute viewing grace.
Renewed viewing resets the checkout deadline before the next sweep. Automatic
shell closes are signalled in the pass that decides cleanup; explicit Close stays
on the shell receipt and waits for requested work to settle. Completion and PR
landing schedule one retirement pinned to the completion, checkout and branch;
reopen or changed placement revokes it.
Installation provisions a separate launchd/cron job running `lf task retire` once
a minute. Each pass finds closed Tasks with running conversations as well as the local
retirement queue, reconciles viewers, attempts
eligible cleanup and exits. No Task owns a waiting cleanup process. Eligibility
normally reaches a pass in 15–16 minutes while awake; wall-clock deadlines survive
sleep and reboot. Destination owners confirm shell job settlement before deletion;
a failed cleanup is retried on a later pass. Abandoned local checkouts also remain
in the same retry queue after their last conversation exits, until cleanup
succeeds or the Task reopens. Completed-Task cleanup discards uncommitted files but
preserves primary/persistent checkouts and changed committed refs. Abandoned
checkout retries retain dirty files; an earlier `--force` applies only to that
invocation. Failures record a reason and retry after one minute, doubling up
to an hour independently of the absence deadline. One failed Task does not stop
other queued Tasks. Overlapping passes serialize, and interrupted effects can be
retried. `lf task retire <task>` runs one pass for a specific Task. Reopening a
retired Task never silently recreates a checkout.

Desktop's private [shell runtime](../desktop/README.md) carries startup resources
on creation; the viewer transport does not select terminal integration.

Desktop keeps viewers open for visible Tasks and a bounded recent set of
unfinished conversations, so those hold their checkouts; leaving the set,
Close and window close detach them.

Desktop's automatic opening uses a non-replacing input claim: working Tasks use
Start, which permits first-thread creation; other Work uses reveal, which requires
existing history. Their prepared commands retain that distinction through launch.
If another viewer wins before attachment, the owner returns Busy and keeps that
viewer's input.
Only Take back uses ordinary resume to replace a holder; Try again on a consumed
first launch uses it on a Session no client holds. The live relay announces
native input readiness to Desktop after its owner acknowledges the native client;
saved screen bytes never provide that announcement. A successful automatic attach
records its Desktop terminal association after the handshake, preserving current
provider status without claiming another viewer's input during preparation.
