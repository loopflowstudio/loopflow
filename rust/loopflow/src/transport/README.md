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
an attached viewer prints why, restores its terminal, and runs the supplied
`lf resume <session>` command through the lf entry. Its exit status or launch
error becomes the viewer's result.
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

Terminal snapshots retain a text grid and up to 2,000 scrollback rows. They restore
alternate-screen state, application keys, bracketed paste and mouse modes supported
by vt100. Images, hyperlinks, extended keyboard protocols and exact restoration of
both screen buffers are not supported. Headless observation retains a bounded output
tail and preserves stdout/stderr on delivery. Slow viewers disconnect; execution
continues draining.

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
