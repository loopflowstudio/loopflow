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
`lf resume SESSION --prepare --json` returns the one command Desktop runs, without
opening a socket or starting a provider. Desktop decides from the record's `state`
whether showing a pane reattaches: a `detached` Session reattaches, an `active` one
waits for Move here, and an ended one waits for an explicit open.
Preparing asks a running owner whether a viewer holds control, so an owner from an
earlier lf, which cannot record its viewers in a store a newer lf advanced, reads
`detached` once Desktop restarts and its pane reattaches. An owner that is running
but never answers reads `detached` too; reattaching stops its provider and continues
the same provider thread in a new owner, without replaying input.

`lf resume --all` is the one automatic restart: it resumes Sessions that were on
screen when the machine went down, as after a reboot, crash or kill: their owner
had a viewer, never recorded its own exit, and is gone from the OS. Leaving
through the provider or idling out records an exit, so those never
qualify, and a conversation nobody was viewing waits for `lf resume`. Dead PIDs,
pre-boot start times and rows retention already closed read as gone and block
nothing. Each resumes through ordinary `lf resume` in a new owner with no
viewer; its interrupted turn is flagged, not replayed. It reports each Session
it resumes or skips. Desktop runs it at launch.

A Session nobody views ends after `session.end_detached_after` (default `2m`,
`off` disables it) of quiet. The grace counts wall time, including laptop sleep,
from the last viewer change, command output or held work. A turn or running
Graph keeps its owner and restarts the grace; finished Graphs do not. Native
Claude prompt/stop hooks preserve even silent tool or network waits until Stop.
Terminal status counts only while a viewer is attached to refresh it. Headless
Runs never end for want of viewers.

Removing or replacing the owner's lf executable ends it after two seconds of
command silence, once no turn, running Graph or headless Run holds it. Viewers
cannot extend this interval. The owner logs the path to `logs/lf.log`; attached
viewers receive the reason and a resume command. Failed store judgments retry
at the next poll.

At policy or command exit, the owner saves `$LF_HOME/screens/<session>.vt`:
scrollback followed by the visible grid. Policy exit then signals the owner,
stops its agent and records its exit. `lf resume` reopens provider history at
the bottom without starting a turn; `lf resume --all` skips this recorded exit.
Painting the saved screen on return belongs to LOO-710's reader and format
cutover.

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

Terminal snapshots, live or saved, retain a text grid and up to 2,000 scrollback rows. They restore
alternate-screen state, application keys, bracketed paste and mouse modes supported
by vt100. Images, hyperlinks, extended keyboard protocols and exact restoration of
both screen buffers are not supported. Headless observation retains a bounded output
tail and preserves stdout/stderr on delivery. Slow viewers disconnect; execution
continues draining.

An execution command delegates to a detached owner, which holds its PTY or pipes
and records an ordinary Process. Loopflow launches that owner itself; it has no
public command. Session rows name that Process; viewer tokens and
terminal bytes remain in memory. Native clients retain their original provider
channels, including pending request identities. Graph steps share their Graph's
terminal owner through explicit child handoff, retaining their own Process results.

While a native terminal is open, Loopflow sends its own diagnostics and background
helper output to private `logs/terminal-PROCESS.log` files under the selected
Machine home (`~/.lf` by default). Each launcher prints its location before handing
over the terminal and reports newly recorded diagnostics after returning. Native
provider output retains its original terminal; OpenCode keeps its separate
provider stderr log. Ordinary CLI output resumes when the client exits or the
viewer detaches. Read these logs from another terminal while editing a draft.
