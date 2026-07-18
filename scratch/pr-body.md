## Try it!

```bash
# Inspect the architecture-owned Wave lifecycle projection
lf status --json

# Install the same operating contract in an external harness
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

`status` now reports the durable Work lifecycle (`ready`, `running`, typed
`waiting`, `done`, or `abandoned`) while listener liveness remains separate
evidence.

## Intent

Give CLI and Mac consumers one lifecycle vocabulary and make the public agent
package explicit about caller authority. An external harness acts as the User
over the same `lf` API as the Mac app; a Loopflow-launched worker stays on its
radio channel. Neither path gets a second status model, client, store, or
transport.

## Assumptions

- PR #1073's durable spine exists for every registered Wave.
- `WorkStatus` owns lifecycle; listener `live` owns reachability evidence.
- Authored pause policy and authored-only Wave repair remain transitional until
  typed UserStart waits and stable Wave creation ship.

## Key decisions

- Replace `WavePresence`/Swift `WaveStatus` with the existing Rust
  `WorkStatus` directly.
- Mirror the externally tagged Rust wire shape in Swift and prove all Wait
  variants with one shared fixture.
- Reuse the same Swift Work reference and Basis types in Launch projections.
- Serialize durable timestamps as RFC 3339 so Rust and Swift share one explicit
  DTO contract.
- Extend the existing `skills/loopflow/SKILL.md` and `docs/agent-api.md`
  package instead of adding another agent surface.

## Not included

This is the status and caller-authority slice. `lf start`, remote lifecycle via
`lf ssh`, authoritative pre-Run Home placement, stable `lf wave create`, shared
Home residency, and Session deletion remain follow-up architecture work. They
are intentionally not implemented against the legacy WaveHome/Session models.

## Verification

```bash
uv run python scripts/test.py --all
```

Passed locally: 124 Python tests, Rust fmt/Clippy plus 1,837 tests, 66 website
tests, 191 Swift tests, e2e smoke, and the signed Xcode build-for-testing. The
separate `--ui-host` attempt built and signed the app but macOS canceled
LocalAuthentication before the UI runner initialized; no UI test body ran, so
the authorized CI host remains the proof for that check.
