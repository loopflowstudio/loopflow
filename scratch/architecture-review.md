# Architecture cutover review

## Implemented slice

- Replaced provider-wide `supports_steer` with one per-Turn
  `send_current` outcome: Sent, NotSteerable, Failed, or Unknown.
- Correlated Codex `turn/steer` with its JSON-RPC response and exact expected
  Turn id before reporting Sent.
- Made plain Steer additive: it never interrupts, and every delivery outcome
  remains input to the next boundary.
- Added the four control fixtures: live accepted, live rejected, ambiguous
  response, and opaque TUI without Turns.
- Renamed trace-only Rust ids to `TraceId` / `ExecId` and capture ids to
  `LaunchId` / `TurnId`, freeing the product Run vocabulary.

## Review findings

- **Fixed:** live Wave steering initially consumed a Sent message. It now
  requeues it because provider acceptance cannot advance the active Turn's
  immutable Basis.
- **Fixed:** Codex initially treated enqueueing JSON-RPC onto the local writer as
  Sent. It now waits for the correlated provider response; timeout or disconnect
  becomes Unknown.
- **Retained boundary:** Project/Task still use `ChildCommand` as the transport
  receipt. The Steer/Send/Basis persistence checkpoint must replace it rather
  than grow another compatibility layer.
- **Operational risk:** a lost Codex response holds the control call for at most
  15 seconds, then safely falls back to the next seed. Late responses only clear
  their pending waiter; they cannot trigger a duplicate same-Turn attempt.

## Validation

- `cargo test -p loopflow --lib --no-fail-fast` — 1,564 passed.
- `cargo clippy -p loopflow --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
