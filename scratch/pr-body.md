## Usage

```bash
# Inspect architecture-owned Wave lifecycle
lf ls
lf status --json | jq .wave.status

# Install the operating contract in an external harness
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

## Summary

Give CLI and Mac consumers one lifecycle vocabulary and make the public agent
package explicit about caller authority. An external harness acts as the User
over the same `lf` API as the Mac app; a Loopflow-launched worker stays on its
radio channel. Neither path gets a second status model, client, store, or
transport.

## Changes

- Replace Rust `WavePresence` and Swift `WaveStatus` with architecture's
  existing `WorkStatus`, derived from Epoch, Run, and Wait facts.
- Keep listener `live` as independent reachability evidence rather than folding
  it into lifecycle.
- Mirror every `WorkStatus` and typed Wait variant in Swift through one shared
  Rust/Swift fixture.
- Reuse Swift `WorkReference` and `WorkBasis` in Launch projections instead of
  maintaining Launch-specific copies.
- Serialize durable timestamps as RFC 3339 so the Rust and Swift DTOs share one
  explicit wire contract.
- Extend the existing README, `docs/agent-api.md`, and published Loopflow skill
  with the external-User versus internal-worker authority distinction.
- Keep #1091's deleted doctrine-anchor matrix deleted; the focused packaging
  test proves the new caller-authority behavior instead of pinning duplicate
  prose.

## Not included

This is the status and caller-authority slice. `lf start`, remote lifecycle via
`lf ssh`, authoritative pre-Run Home placement, stable `lf wave create`, shared
Home residency, and Session deletion remain follow-up architecture work. They
are intentionally not implemented against the legacy WaveHome/Session models.

## Verification

`uv run python scripts/test.py --all` passed locally: 124 Python tests, Rust
fmt/Clippy plus 1,837 tests, 66 website tests, 191 Swift tests, e2e smoke, and
the signed Xcode build-for-testing.

The separate `--ui-host` attempt built and signed the app but macOS canceled
LocalAuthentication before the UI runner initialized. No UI test body ran; the
authorized CI host remains the proof for that check.
