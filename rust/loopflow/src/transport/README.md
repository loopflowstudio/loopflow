# Retain execution across terminal loss

```sh
lf session attach SESSION
lf session attach SESSION --if-free
lf session attach SESSION --observe
lf --machine build session attach SESSION
```

Ctrl+] detaches a viewer. Ctrl+C in an attached terminal reaches the running
program. A Task step's live controls (`/status`, `/interrupt`, `/detach`,
messages) are shown to each controlling viewer as it attaches; headless output
and observers never see them. Closing a terminal leaves execution, provider requests and capture
settlement with the detached owner; `lf session list` shows its Session as
`detached` and `lf connect` reattaches it. Observing takes no input claim; a controlling
attachment revokes the previous viewer's input token. `--if-free` refuses when
another viewer controls input. Desktop uses it for restoration; it never displaces
a viewer just because a pane becomes visible. `session attach --prepare --json`
returns an attach-only command without opening a socket or starting a provider,
or `null` when nothing is running to attach to. Repository, Wave and Task ensure
prepare metadata without launching execution; existing executions return no
command and attach when shown. Ended execution needs explicit resume.

`lf resume --all` is the one automatic restart: it resumes Sessions whose owner
never recorded an exit and is gone from the OS, as after a reboot, crash or kill.
Leaving through the provider or detaching records an exit, so those never qualify.
Each resumes through ordinary `lf resume` in a new owner with no viewer; its
interrupted turn is flagged, not replayed. It reports each Session it resumes or
skips. Desktop runs it at launch.

A Session nobody views ends after `session.end_detached_after` (default `24h`,
`off` disables it), counted from when its last viewer left. A Session whose lf
executable is removed or replaced (a deleted worktree, a relinked build, a rebuilt
Dev.app) ends within seconds, viewed or not, and logs the path to `logs/lf.log`.
A turn in progress or a Graph step keeps its owner running until it settles; a
viewer keeps it only from the detached limit; headless Runs are never ended. Each
owner judges only itself and ends by signaling itself, which stops its agent and
records its exit. `lf resume` reopens the conversation from provider history under
whichever lf runs it; `lf resume --all` skips it.

`lf resume` reconnects to a retained native terminal. A structured Run may require
an explicit transition to the provider's native UI. That transition preserves the
Session and saved conversation, but does not preserve its old in-flight turn or
resubmit its input. `session attach` never performs that transition.

An unavailable owner leaves delivery unknown. Reconnection does not launch another
provider or replay a Graph. Finished Runs retain results in Session history.
After connecting, attachment waits up to five seconds for acknowledgement. Timeout
or Ctrl+C while waiting closes only the viewer; execution remains unknown. Once
acknowledged, the attachment has no time limit.

For commands using pipes, the controlling viewer forwards stdin EOF after all
accepted input. Disconnecting the viewer alone leaves stdin open. Terminal
attachments keep their PTY open; the running program interprets terminal EOF keys.

Terminal snapshots retain a text grid and up to 2,000 scrollback rows. They restore
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
