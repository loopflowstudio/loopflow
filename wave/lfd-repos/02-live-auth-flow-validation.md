# 02: Live Auth-Flow Validation

Status: **shipped** (branch `jack-heart.lfd-repos.20260225_2127`)

## What shipped

- Added `scripts/test_auth_live_contract.py` to run local live auth contract validation for `github`, `claude`, and `codex`.
- Validated the `/v0/auth/{provider}` flow over HTTP + websocket, including URL/code payload fields and lifecycle event ordering (`auth.flow_started` → terminal event).
- Captured machine-readable evidence per run in `reports/auth-live/<timestamp>/`:
  - provider matrix summary,
  - raw CLI transcripts + CLI versions,
  - auth payload/status samples,
  - auth lifecycle events,
  - credential tree snapshots.
- Added Claude disconnect validation for auth-artifact cleanup while preserving non-auth settings.
- Hardened provider auth behavior in `rust/loopflow/src/lfd/provider_auth.rs`:
  - broader user-code parsing,
  - ANSI escape stripping before parsing,
  - GitHub status fallback from credential files,
  - GitHub disconnect tolerance when already logged out,
  - Claude login command update to `claude auth login`.
- Added regression tests for parser/disconnect edge cases and documented the live harness command in `TESTING.md`.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow provider_auth`
- `uv run python -m py_compile scripts/test_auth_live_contract.py`
- `uv run python scripts/test_auth_live_contract.py --help`

## Follow-ups

- Keep running `scripts/test_auth_live_contract.py` during release validation to catch upstream CLI drift.
- Promote any newly observed provider output/credential patterns into Rust regression tests.
