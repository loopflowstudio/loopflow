# LOO-712: Session transport speaks Rex's shape

Jack Heart asked (2026-11-02) for the owner (`lf session serve`) and its viewers
to adopt the parts of Rex's protocol worth copying. Status: accepted design,
implementing. Evidence: `scratch/research/rex-architecture.md` in the LOO-672
checkout (read 2026-11-02).

## Wire

One Unix socket per owner, one connection per viewer or control client. Every
message is a frame: `tag u8 · len u32 BE · payload`. Payloads are at most
64 KiB for stream frames; the parser refuses anything over 4 MiB.

Owner → viewer:

| tag | name | payload |
|---|---|---|
| 0x01 | reply | JSON; answers one request (`answers`, `seq`, `kind`) |
| 0x02 | snapshot | owner's terminal snapshot bytes, format-agnostic |
| 0x03 | out | stream 1 bytes (PTY or stdout) |
| 0x04 | err | stream 2 bytes (pipe mode stderr) |

Viewer → owner:

| tag | name | payload |
|---|---|---|
| 0x11 | request | JSON with `id` |
| 0x12 | input | raw keystrokes from the controlling connection |

A viewer's *stream* is the ordered bytes of its snapshot/out/err frames. The
viewer counts them; the owner spools the last 1 MiB per viewer.

**Attach:** `{"id":1,"kind":"attach","control":bool,"replace":bool,"own":"take"|"request"|null,"rows","cols","name","moved"}` →
reply `attached {answers, seq, viewer, reconnect, nonce, token, terminal, grace_ms, owner}`,
then snapshot frames, then a `resize` event, a `theme` event when known, then live
out/err frames. Pipe mode has no snapshot: the retained tail is the start of the stream.

**Events** travel in stream 1 as APC `ESC _ lf1;<nonce>;<json> ESC \` with
gapless `seq` per viewer stream. A program printing the same bytes lacks the
nonce and is passed through. Kinds: `resize {rows,cols}`, `theme {…}` (opaque,
from the owner viewer), `owner {viewer,name,held}`, `control {held,notice}`
(input control taken; LOO-618 keeps the loser leaving), `child_exited {code}`,
`detached`, `hint {text}`, `return {detach}`. Events are inserted only at
terminal sequence boundaries, like OSC 7501 reports. Every reply carries the
`seq` of the viewer's last queued event, in the shape `--watch` uses
(request `id` / reply `answers`).

**Resume:** EOF without a `leave` request keeps the viewer's spool for 60 s.
`{"kind":"resume","reconnect","received":N,"control","own","rows","cols"}` continues the
stream from byte N with no snapshot; a count outside the spool answers
`reconnect_required` and the viewer attaches afresh. Input control and
ownership release at EOF (an automatic attach must not see Busy for a dead
viewer); a resume re-takes them if asked.

**Ownership** arbitrates size and colors only. `take` replaces the owner (the
loser gets `owner` with `held:false` and stays attached); `request` queues and
is granted when the owner leaves. The PTY takes the owner's size; a non-owner's
resize is remembered for when it owns. Who may type is unchanged: `token`
input control with `replace` semantics, `Busy`, `Moved` → `control` event.

**Agent parity:** LOOPFLOW.md gains the two Rex sentences Loopflow lacked:
the person may be viewing another Session, and never `lf resume` your own.

## Delete — do not maintain

Done (2026-11-02): the JSON-lines `Request`/`Response` enums, `Viewer::queue`,
`Output::attach`, and the JSON-lines test fixtures are gone; `wire::Frame` and
events replaced them. Nothing remains on this list.

Preserve: every existing test's behavior (input fencing, Busy, automatic
attach after disconnect, pipe EOF, slow observer disconnect, native pending
request, silent owner), OSC 7501 reports, snapshot pending-prefix carry.

## Done when

- Two viewers both start with the snapshot and both see live output and input.
- Resume by byte count continues exactly; spool exhaustion refuses; fresh attach repaints.
- Resize reaches every viewer as a nonce APC; a program printing the same bytes does not.
- `request` behind `take` is granted on leave; `take` loser gets `owner held:false`.
- 1 MB burst → wire within 10%.
- `transport_tests`, `session_lifecycle_tests`, `session_launch_contract`, `golden_prompt` pass; README documents the stream.

## Remaining

- LOO-705 has not landed on main (checked 2026-11-02): the `control` event
  carries `successor`, and the viewer still leaves on `held:false`. When this
  Task rebases over LOO-705, the viewer reruns its invocation through
  `successor` instead of leaving, per Jack's steer.
- `session_lifecycle_tests`, `session_launch_contract` and clippy are gate's.

Check (compress, 2026-11-02): `cargo test -p loopflow --test transport_tests` 26 passed; `--lib transport::` 18 passed; `cargo clippy -p loopflow --all-targets -- -D warnings` clean; goldens unchanged since regeneration.
