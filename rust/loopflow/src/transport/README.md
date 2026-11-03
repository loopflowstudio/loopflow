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

`lf resume --all` is the one automatic restart: it resumes Sessions whose owner
never recorded an exit and is gone from the OS, as after a reboot, crash or kill.
Leaving through the provider or detaching records an exit, so those never qualify.
Each resumes through ordinary `lf resume` in a new owner with no viewer; its
interrupted turn is flagged, not replayed. It reports each Session it resumes or
skips. Desktop runs it at launch.

A Session nobody views ends after `session.end_detached_after` (default `24h`,
`off` disables it), counted from when its last viewer left. A Session whose lf
executable is removed or replaced (a deleted worktree, a relinked build, a rebuilt
Dev.app) ends within seconds, viewed or not, and logs the path to `logs/lf.log`;
an attached viewer prints why and the `lf resume <session>` that reopens it.
A turn in progress or a Graph step keeps its owner running until it settles; a
viewer keeps it only from the detached limit; headless Runs are never ended. Each
owner judges only itself and ends by signaling itself, which stops its agent and
records its exit. `lf resume` reopens the conversation from provider history under
whichever lf runs it; `lf resume --all` skips it.

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
scrollback, styles, hyperlinks, modes and unfinished input. `lf resume` paints that
screen while the provider starts. The live owner takes over its display and input
modes, including a blank screen. Startup failure or cancellation restores the
terminal; resume the same Session again. A fresh owner without a saved preview
leaves the viewer's existing screen in place; mode-only output is still restored.
Snapshot format v1 promises no compatibility
across revisions, so another revision's file reads as absent.
Headless observation retains a bounded output tail and preserves stdout/stderr on
delivery. Slow viewers disconnect; execution continues draining.

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
