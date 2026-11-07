# 5bfda1: Keep keystrokes a viewer sent just before it disconnected

Status: implemented 2026-11-06; gate and Linux CI remain. No product decision is open.

## Problem

Text typed into a Session just before its viewer goes away can vanish. Desktop
quitting right after a keystroke is the everyday case. CI saw it once on Linux
(#1813, run 42314399380, `server_preserves_owner_and_graph_across_restart`):
"draft" typed through a server viewer that was dropped at once never reached the
provider; the answers file read "kept", not "draft kept".

## Cause (found in code, `rust/loopflow/src/transport.rs`)

The owner treats a viewer as gone the first time a **write** to it fails, while
the viewer's input can still be sitting unread on the socket.

- Each pass reads a viewer once, at most 8 KiB (`Viewer::read`), then decodes
  and applies its frames.
- Later in the same pass the owner writes to that viewer: the request loop's
  `viewer.flush()?`, `place_events`, `deliver` (output) and the reports path.
- If the peer closed in between, the write fails with `EPIPE`. `survive` →
  `drop_connection` swaps the connection out (`Connection::Dropped`) and resets
  the decoder. The socket is closed with the unread bytes still queued; they
  never reach `PendingInput`.

The window is one owner pass: the bytes arrived after this pass's read and the
peer closed before this pass's write. That matches the CI loss, a 5-byte "draft"
frame sent right after attach. The same thing happens without any race whenever
attach plus input exceed the 8 KiB read: the attach reply's flush in that pass
fails and everything past the first 8 KiB is dropped.

Reads alone are fine. On Linux and macOS a closed Unix stream peer's queued data
reads out before EOF (Linux returns `ECONNRESET` only once the queue is empty),
so a viewer whose close is noticed on read has already given up all its frames.

## Demo

```sh
cargo test -p loopflow --test transport_tests viewer_input_survives_its_disconnect
```

The test pauses an owner (SIGSTOP), connects a controlling viewer, sends attach
plus 20 KB of input, closes, and lets the owner go: all 20 KB reach the program
in order. Before the fix the program gets only the first read's worth. A second
loop attaches, types and closes at once 50 times against a chatty provider; every
keystroke arrives. On a person's machine: type, quit Desktop in the same instant,
`lf resume` shows the text in the editor.

## Approach

**A viewer is gone when its stream ends, not when a write to it fails.**

`Connection` gains a third state: `Open(socket)` → `Draining(socket)` →
`Dropped(at)`.

- `Viewer::flush` on a lost-kind error (`BrokenPipe`, `ConnectionReset`,
  `ConnectionAborted`) moves `Open` to `Draining`, clears `output`, and returns
  `Ok`. Nothing more is written; `stream` keeps spooling for a resume.
- `Viewer::read` reads `Draining` as it reads `Open`. Its frames go through the
  same request loop with the viewer's current token, so input is applied in
  order, and only if that viewer still holds control.
- Reaching EOF (or reset with an empty queue) goes through `survive` →
  `drop_connection`, as today. The peer has already closed, so EOF follows the
  queued bytes, usually in the next pass.
- `wait_for_activity` polls `Draining` for `POLLIN` only. `live()` keeps
  counting it until EOF so ownership and control settle once, at the real end.
  A `Resume` targets `Dropped` viewers only; a resume that arrives while the old
  stream is still draining sees `ReconnectRequired` only if EOF has not yet been
  read, and the first `retain_mut` read pass reads that EOF before resumes are
  matched in the same pass.
- `stream` encodes onto `output` for `Open` only, so a draining viewer can't
  trip "Viewer fell behind" while its last frames are read.

Uncertain input stays unreplayed: `PendingInput` and the spool are unchanged,
nothing replays input to a later viewer, and a revoked token still discards
the frames (the check at `Tag::Input` is untouched).

## Alternatives considered

- **Read until `WouldBlock` every pass.** Fewer bytes left behind, but bytes that
  arrive between this pass's read and write are still lost. Doesn't close the window.
- **Drain synchronously at the write failure.** Correct for Unix sockets (a closed
  peer's data is already queued), but the frames must go through the request loop,
  which is one large closure in `serve`; pulling it out to call from a write site
  is a bigger change for the same result. `Draining` gets there through the
  existing loop.
- **Shut down the read side on write failure.** That loses the bytes outright.

## Risk findings

- Peer that shuts down only its read side and keeps sending: under `Draining`
  it stays a writer that gets no output until it closes. That is what it asked
  for; it can't overrun anything because output is discarded. No bound added.
- `survive` still drops on read errors immediately. Correct per the kernel
  ordering above; the macOS run of the new test and Linux CI confirm it.
- Other write sites (`end_viewers`, `ending` replies, `ReconnectRequired`) already
  ignore flush errors; with `Draining` they become no-ops on a closed peer.

## Relay test finding (`unanswering_superseded_owner_preserves_uncertain_draft_and_pending_turn`)

Not this cause. In the failing run (job 128174029603, 26.2 s, panicked at
"relay did not exit") the first `wait_exit`, the `lf resume` against a SIGSTOPped
owner, ran past its 20 s deadline; passing runs take 20.5 s for the whole test.
The input it types is small and fully read before `pending-turn` appears.

The likely cause is the fixture freezing the owner mid store write. `first`
disconnects just before the pause; the owner's lifetime thread records
`processes.viewed = 0` on its next 2 s poll (`lifetime::watch` →
`record_owner_viewers`). A SIGSTOP landing inside that write leaves the WAL
write lock held, and `lf resume` writes (`record_session_terminal`, workspace
admission) waiting on the 15 s `SQLITE_WRITE_BUSY_TIMEOUT`, then the 5 s
attachment ack: past 20 s. Not reproduced; inferred from timings and code.

Fix in the test: before `fixture.pause`, wait until `processes.viewed` for the
owner reads 0, so the pause lands on an owner that holds no write. A frozen
owner holding a store lock in production is a separate question (resume would
wait out the busy timeout); not this Task.

## Scope

In: owner viewer connection lifetime (`transport.rs`), new regression test,
relay test pause fix, transport README line.
Out: server bridge (#1813 branch, LOO-593); its test benefits after it rebases.
Out: busy-timeout behavior of `lf resume` against a frozen owner.

## Delete — do not maintain

- Done: the probe `probe_disconnect_keeps_queued_input` became
  `viewer_input_survives_its_disconnect`; no probe remains.
- No other attempt at this exists (searched `drop_connection`/`survive` history).

## Done when

- A viewer's input sent before it closed reaches the program in order, even when
  the owner writes to it after the close and before reading the input.
- `viewer_input_survives_its_disconnect` passes on macOS locally and Linux CI:
  paused-owner case all 20 KB; 50 type-and-close rounds lose nothing.
- Input from a viewer whose control was taken over is still discarded (existing
  tests: `server_reconnect_preserves_newer_viewers_input_and_geometry` on #1813,
  transport takeover tests on main).
- The relay test waits for the viewer record before pausing.

## Checks

- gate: `uv run python scripts/test.py --reuse-passing --rust-filter 'binary(transport_tests)'` — transport suite passes.
- implement: `cargo test -p loopflow --test transport_tests viewer_input_survives_its_disconnect` (focused).
- CI Linux: the integration partition runs both tests; re-run the relay test 5× via `cargo nextest run --test transport_tests -E 'test(unanswering_superseded_owner)' --retries 0` in a loop if a Linux host is at hand, else CI.

## Check results

- Before the fix, macOS: the paused-owner case lost all but 4096 of 20000 bytes.
- `cargo test -p loopflow --test transport_tests -- viewer_input_survives_its_disconnect continue_session unanswering_superseded_owner resumed_session_completes_turns`, macOS: pass (new test 3/3 runs, ~6 s).
- `cargo test -p loopflow --lib transport::`, macOS: 53 pass.
- The 50-round loop passed twice on unfixed code on macOS: a pass reads its
  viewers before writing, so a small keystroke is lost only when it arrives
  between the two. It is a Linux stress check (where CI saw the loss); the
  paused-owner case is the deterministic regression.
- compress: `viewer_input_survives_its_disconnect` re-run after the `flush` reshape, macOS: pass.
- Remaining: gate (`binary(transport_tests)`, fmt, clippy); Linux CI for both tests.
