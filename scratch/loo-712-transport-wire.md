# LOO-712: Session transport speaks Rex's shape

Jack Heart asked (2026-11-02) for the owner (`lf session serve`) and its viewers
to adopt the parts of Rex's protocol worth copying. Status: accepted design,
implemented on this branch (realigned 2026-11-02); landing waits on LOO-705.
Evidence: `scratch/research/rex-architecture.md` in the LOO-672 checkout
(read 2026-11-02).

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
then snapshot frames, then a `resize` event, a `theme` event when known, an
`owner` event, a `return {detach}` event when the owner fronts a native client,
then live out/err frames. A `take` on attach replaces the attach-time `resize`
and `owner` with the ones every viewer gets for the new owner. Pipe mode has no
snapshot: the retained tail is the start of the stream.

**Events** travel in stream 1 as APC `ESC _ lf1;<nonce>;<json> ESC \` with
gapless `seq` per viewer stream. A program printing the same bytes lacks the
nonce and is passed through. Kinds: `resize {rows,cols}`, `theme {…}` (opaque,
from the owner viewer), `owner {viewer,name,held}`, `control {held,notice,successor}`
(input control taken; LOO-618 keeps the loser leaving until LOO-705's successor
lands), `child_exited {code}`,
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

Each line names the `transport_tests` case that proves it (all passing at compress).

- [x] Two viewers both start with the snapshot and both see live output and input
  — `every_viewer_starts_with_the_snapshot_and_shares_live_output`.
- [x] Resume by byte count continues exactly; spool exhaustion refuses; fresh attach
  repaints — `a_dropped_viewer_resumes_from_its_byte_count_without_a_snapshot`,
  `a_resume_past_the_spool_is_refused_and_a_fresh_attach_repaints`.
- [x] Resize reaches every viewer as a nonce APC; a program printing the same bytes
  does not — `resize_reaches_every_viewer_as_its_own_event_and_program_bytes_do_not`.
- [x] `request` behind `take` is granted on leave; `take` loser gets `owner held:false`
  — `ownership_queues_behind_a_taker_and_tells_the_loser_who_took_it`.
- [x] 1 MB burst → wire within 10% — `a_megabyte_burst_costs_within_ten_percent_on_the_wire`.
- [x] `transport_tests` and `golden_prompt` pass; `transport/README.md` documents the stream.
- [ ] `session_lifecycle_tests` and `session_launch_contract` pass — gate runs them.

## Remaining

- **Land order** (Jack Heart, 2026-11-02, via the Task comment): LOO-705 first,
  then LOO-708/LOO-709, then this Task. None of the three is on origin/main as of
  2026-11-02 14:40; the branch is otherwise even with main.
- **LOO-705 successor.** The `control` event already carries
  `successor: Option<PathBuf>`; the owner sends `None` and the viewer's
  `held: false` arm in `transport.rs` leaves as before. After the rebase over
  LOO-705, the owner fills `successor` from LOO-705's Moved entry and that arm
  runs the viewer's invocation through it (`session resume <id>` via the
  installed lf) instead of returning.
- **LOO-708 coordination.** Both Tasks edit `transport.rs`; this one owns the
  wire format, LOO-708 which processes exist. Expect a conflict-bearing rebase.
- `session_lifecycle_tests`, `session_launch_contract` and clippy are gate's.

Check (compress, 2026-11-02): `cargo test -p loopflow --test transport_tests` 26 passed; `--lib transport::` 18 passed; `cargo clippy -p loopflow --all-targets -- -D warnings` clean; goldens unchanged since regeneration.
Check (realign, 2026-11-02): no source change since compress; `git rev-list --count HEAD..origin/main` = 0; tests not rerun.
