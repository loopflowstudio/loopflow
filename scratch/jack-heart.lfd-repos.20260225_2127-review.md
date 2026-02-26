# Live auth-flow validation (gate review)

## What was implemented

- Added `scripts/test_auth_live_contract.py` to run live auth contract checks against local `lfd` for `github`, `claude`, and `codex`.
- The script now captures:
  - raw CLI auth transcripts + CLI versions,
  - `/v0/auth/{provider}` payload/status samples,
  - `auth.*` websocket lifecycle events,
  - provider credential tree snapshots before/after,
  - provider matrix summary in `reports/auth-live/<timestamp>/matrix.json`.
- Added Claude disconnect validation that confirms auth artifacts are removed while non-auth settings remain.
- Hardened provider auth parsing/disconnect behavior in `rust/loopflow/src/lfd/provider_auth.rs`:
  - broader user-code parsing,
  - ANSI escape stripping before parsing auth output,
  - GitHub status fallback from credential files,
  - GitHub logout handling for already-disconnected state,
  - Claude login command updated to `claude auth login`.
- Added Rust regression tests covering new parser/disconnect edge cases.
- Added the new live harness command to `TESTING.md`.

## Key choices

- **Strict on API contract, tolerant on CLI prose:** validate URL/code/event fields and ordering, but avoid pinning exact human-readable CLI text.
- **Capture evidence by default:** every run writes machine-readable artifacts per provider so failures are reviewable, not anecdotal.
- **Check terminal status before cleanup:** the gate fix ensures `auth.connected` is validated against pre-disconnect status.
- **Fail gracefully on malformed disconnect responses:** Claude disconnect check now reports non-JSON responses as actionable failures instead of crashing the script.

## How it fits together

`test_auth_live_contract.py` boots an `lfd` runtime, runs provider validations via HTTP + websocket, and snapshots credential evidence on disk. Rust auth broker/parser updates make `/v0/auth` resilient to real-world CLI output drift, while regression tests lock in discovered behavior.

## Risks and bottlenecks

- Live validation is credential- and binary-dependent (`gh`, `claude`, `codex` installed + logged in flow capability).
- Event timing can vary across hosts; long/slow environments may need timeout tuning.
- CLI output formats may continue to drift; harness evidence still requires periodic human review and fixture promotion.

## What's not included

- No CI gating for live auth runs.
- No Concerto UI changes (Connections panel remains a later step).
- No new provider support beyond GitHub/Claude/Codex.
- No token refresh or provider-specific onboarding UX changes.

## Validation run for this gate pass

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow provider_auth`
- `uv run python -m py_compile scripts/test_auth_live_contract.py`
- `uv run python scripts/test_auth_live_contract.py --help`
