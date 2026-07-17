# Architecture cutover review

## Implemented slice

- Replaced provider-wide `supports_steer` with one per-Turn
  `send_current` outcome: Sent, NotSteerable, Failed, or Unknown.
- Correlated Codex `turn/steer` with its JSON-RPC response and exact expected
  Turn id before reporting Sent.
- Made plain Steer additive: it never interrupts, and every delivery outcome
  remains input to the next boundary.
- Drove the four control shapes — live accepted, live rejected, ambiguous
  response, and opaque TUI without Turns — through the real controller.
- Typed Codex steer rejections from probed live evidence, and released the
  pending waiter on every terminal path.
- Renamed trace-only Rust ids to `TraceId` / `ExecId` and capture ids to
  `LaunchId` / `TurnId`, freeing the product Run vocabulary.

## Review findings

- **Fixed:** live Wave steering initially consumed a Sent message. It now
  requeues it because provider acceptance cannot advance the active Turn's
  immutable Basis.
- **Fixed:** Codex initially treated enqueueing JSON-RPC onto the local writer as
  Sent. It now waits for the correlated provider response; timeout or disconnect
  becomes Unknown.
- **Fixed:** the control-shape test asserted literals it had just read out of
  `control_contract.json`, so it passed against any controller — including one
  that dropped the Steer entirely. The fixture is deleted; the shapes now run
  through `absorb_commands`/`apply_input` and assert the durable outcome. A
  mutation that drops an ambiguous Steer turns the replacement red.
- **Fixed:** every Codex steer rejection is JSON-RPC `-32600`, so the expected
  Turn-boundary race reported `Failed` and warned on the normal path. Rejections
  are now classified by message, with unrecognized errors staying `Failed`.
- **Fixed:** a timed-out steer left its waiter in `pending_requests` until a late
  response or shutdown. A `PendingReply` guard now releases the slot on drop.
- **Retained boundary:** Project/Task still use `ChildCommand` as the transport
  receipt. The Steer/Send/Basis persistence checkpoint must replace it rather
  than grow another compatibility layer.
- **Known gap, needs Phase 1+3:** a confirmed live Send requeues an anonymous
  in-memory `PendingInput::system`, so a crash after `Sent` can still lose the
  seed. Durable Steer plus Basis is the fix; no ChildCommand patch should
  simulate it.

## Validation

- `cargo test -p loopflow --lib --no-fail-fast` — 1,563 passed.
- `cargo clippy -p loopflow --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.

Each new test was checked against the defect it claims to catch: reverting the
rejection classification, neutering the `PendingReply` guard, and dropping an
ambiguous Steer in `send_current_input` each turn the matching test red. The
count falls by one because the deleted fixture test is not replaced 1:1 — it
proved nothing a controller test does not now prove better.
