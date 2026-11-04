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
but never answers reads `detached` too; reattaching stops its provider and continues
the same provider thread in a new owner, without replaying input.

A Session nobody views ends after `session.end_detached_after` (default `2m`,
`off` disables it) of quiet. The grace counts wall time, including laptop sleep,
from the last viewer change, command output or held work. A turn, pending request
or running Graph keeps its owner and restarts the grace; finished Graphs do not.
Native Claude prompt/stop hooks preserve silent waits, background tasks and
scheduled wakeups until Claude reports settlement. Terminal status counts only
while a viewer is attached to refresh it; an idle prompt cannot settle an
unfinished native turn. Headless Runs never end for want of viewers.

Unsubmitted input keeps its native editor alive so returning can edit it.
A plain, single Claude submission can release this protection after its exact
text and prompt ID are acknowledged and settled, with no later typing.
Edits, paste, long input, input during a turn, ambiguous submissions and missing
prompt IDs retain the owner until the native client exits. Other providers
retain typed input because they have no matching acknowledgement here. Clearing
the screen or showing an idle prompt cannot discard that protection. Pending
approvals and questions stay with their original provider until answered or canceled.

Input waits in the owner until its retention record commits. A busy store pauses
delivery to the provider while output, control queries and viewer return remain
responsive. Delivery continues automatically when the store becomes writable;
returning to the Session does not resubmit queued input.

Removing, replacing or superseding the owner's lf executable ends it after two
seconds of command silence, once no input, turn, pending request, running Graph
or headless Run holds it. Viewers cannot extend this interval. The owner logs the
path to `logs/lf.log`; attached viewers receive the reason and run the resume
command through the lf entry. The viewer restores its terminal before running
that command and preserves its exit status or launch error. Failed store
judgments retry at the next poll.
Policy exit saves the screen, stops the agent and records its exit. `lf resume`
reopens provider history without starting a turn. Other ended Sessions stay stopped.

`lf resume` also moves an idle owner from an earlier build onto the lf entry.
If that owner is busy and answers its socket, resume shows the running turn;
an owner with this lifetime policy moves when the turn ends. An older owner
without the policy needs another resume after its turn. If a superseded owner
does not answer, resume ends it and reports that its turn was cut off, then
reopens the saved conversation without replaying input.

`lf monitor rollover --dry-run` lists the same takeover choices across owners;
omit `--dry-run` to move them. The sweep ends owners directly; existing viewers
may close and need `lf resume` again. Graph runners keep running and launch each
next step through the entry. Owners and runners whose store has advanced beyond
their reader floor cannot recover themselves through this policy.

`lf resume` reconnects to a retained native terminal. A structured Run may require
an explicit transition to the provider's native UI. That transition preserves the
Session and saved conversation, but does not preserve its old in-flight turn or
resubmit its input.

An owner that answers but then fails to attach leaves delivery unknown; that
reconnection launches no other provider and replays no Graph. Finished Runs retain
results in Session history.
After connecting, attachment waits up to five seconds for acknowledgement. Timeout
or Ctrl+C while waiting closes only the viewer; execution remains unknown. Once
acknowledged, the attachment has no time limit.

For commands using pipes, the controlling viewer forwards stdin EOF after all
accepted input. Disconnecting the viewer alone leaves stdin open. Terminal
attachments keep their PTY open; the running program interprets terminal EOF keys.

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
| `0x11` request | viewer → owner | JSON with `id` and `kind` |
| `0x12` input | viewer → owner | keystrokes from the connection holding input control |

A viewer's stream is the bytes of its snapshot, out and err frames in order. The
viewer counts them; the owner spools the last 1 MiB per viewer.

```json
{"id":1,"kind":"attach","control":true,"replace":true,"own":"take","rows":40,"cols":120,"name":"/dev/ttys004","moved":null}
{"answers":1,"seq":0,"kind":"attached","viewer":"…","reconnect":"…","nonce":"…","token":"…","terminal":true,"grace_ms":60000,"owner":"…"}
```

The stream then opens with the snapshot (pipes: the retained tail), a `resize`
event, a `theme` event when one is known, an `owner` event, and live output.

Events ride stream 1 as ``ESC _ lf1;<nonce>;<json> ESC \``, inserted only between
complete terminal sequences. The nonce is the viewer's own, so a program
printing the same bytes passes through as output. An event may span frames.
Pending events share the viewer's 4 MiB output queue limit even while waiting
for a terminal sequence to finish. Each carries a gapless `seq`
per viewer, and every reply carries the `seq` of the last event queued when it
was computed. Kinds: `resize {rows,cols}`, `theme {theme}`,
`owner {viewer,name,held}`, `control {held,notice,successor}` (input control
left this connection), `child_exited {code}`, `detached`, `hint {text}` and
`return {detach}`. A retiring owner sends `control` with `held: false` and
`successor: [entry, "session", "resume", session_id]`. The viewer runs that
command after restoring its terminal; `successor: null` simply leaves.

A connection that ends without a `leave` request keeps its stream for 60 s.
`{"kind":"resume","reconnect":…,"received":N,…}` continues from byte `N` with no
snapshot; a byte the spool no longer holds answers `reconnect_required`, and the
viewer attaches afresh. Input control and ownership release when the connection
ends; a resume asks for them again.

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
public command. Session rows name that Process; viewer tokens remain in memory,
and unresolved native input is recorded on the owner Process. A Claude conversation keeps one lf owner
and one relay per terminal showing it. `lf resume` resolves the Session,
then execs into `lf session attach OWNER`, which
opens no store while relaying and parses no commands. It carries the original
command's receipt through exec and records its result when relaying ends.
A Claude launcher under an owner hands
the owner what it would record at exit, then execs into the provider, so Claude
is the owner's direct child and no `lf -i` waits beside it; the owner records
the provider's exit, settles its capture and reports where the conversation
moved. Graph steps keep their launcher so each step settles before the Graph
continues. Codex and OpenCode keep their launcher, which relays to a private server.
The owner sleeps in `poll` on its sockets, its child's output and a wake pipe
that a blocking child wait and the lifetime policy write to. Native clients retain their original provider
channels, including pending request identities. Graph steps share their Graph's
terminal owner through explicit child handoff, retaining their own Process results.

While a native terminal is open, Loopflow sends its own diagnostics and background
helper output to private `logs/terminal-PROCESS.log` files under the selected
Machine home (`~/.lf` by default). Each launcher prints its location before handing
over the terminal and reports newly recorded diagnostics after returning. Native
provider output retains its original terminal; OpenCode keeps its separate
provider stderr log. Ordinary CLI output resumes when the client exits or the
viewer detaches. Read these logs from another terminal while editing a draft.

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
