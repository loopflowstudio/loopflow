# Testing

CI runs six test suites. All must pass before merging.

## Quick Reference

```bash
# Run all checks (what CI runs)
cargo fmt --check                      # Rust formatting
cargo clippy -- -D warnings            # Rust lints (warnings = errors)
cargo test --all                       # Rust tests
uv run pytest python/tests/            # Python tests
swift test --package-path swift        # Swift package tests
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO  # Concerto UI
tests/e2e/test_smoke.sh               # E2E smoke
uv run pytest tests/e2e/test_api_smoke.py -v  # API smoke (live lfd HTTP)
```

Run at minimum the checks that apply to files you changed. A PR that passes locally but fails CI is a broken gate.

## Python Tests

Unit and integration tests for the Python client (`python/loopflow/`).

```bash
uv run pytest python/tests/                          # All Python tests
uv run pytest python/tests/test_client.py::TestClientErrors -v  # Single class
```

## Swift Tests

Tests for the Swift package (models, protocols, shared logic).

```bash
swift test --package-path swift        # All Swift tests
swift test --package-path swift --filter SomeTestClass  # Filtered
```

## Concerto UI Tests

UI tests for the macOS app. Requires Xcode and xcodegen.

```bash
cd swift
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

## What CI Runs

See `.github/workflows/ci.yml`. Six parallel jobs:

| Job | Runner | Command |
|-----|--------|---------|
| `rust-test` | ubuntu-latest | `cargo fmt`, `cargo clippy`, `cargo test --all` |
| `python-test` | ubuntu-latest | `uv run pytest python/tests/` |
| `e2e-smoke` | ubuntu-latest | `tests/e2e/test_smoke.sh` + `uv run pytest tests/e2e/test_api_smoke.py -v` |
| `docker-smoke` | ubuntu-latest | `docker version` + `cargo test -p loopflow docker_` |
| `swift-test` | macos-15 | `swift test --package-path swift` |
| `concerto-ui-test` | macos-15 | xcodegen + xcodebuild |

All six must pass for PRs to merge.

## Rust Tests

Prompt parity and golden prompt tests live in Rust.

```bash
cargo test -p loopflow golden_prompt
uv run pytest tests/parity/test_prompt_parity.py
```

## E2E Tests

Shell-based workflows for CLI and live HTTP API behavior.

```bash
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py -v
```

Long-running workflow tests for `lf ops`:

```bash
tests/e2e/test_full_cycle.sh
tests/e2e/test_rebase_conflict.sh
```

## Validation Scripts

`scripts/` contains runnable validation and demo scripts. Use these for branch validation and manual UI walkthroughs.

```bash
uv run python scripts/concerto-dev.py run-debug     # build and launch lfd + Concerto (macOS)
uv run python scripts/concerto-dev.py run-ios        # build and launch in iOS Simulator
uv run python scripts/check_swift_multiplatform_boundaries.py  # Stage 01 boundary guardrails
uv run python scripts/test_session.py               # session API smoke test (starts lfd)
uv run python scripts/test_auth_live_contract.py --providers github,claude,codex  # live provider-auth contract + evidence capture
uv run python scripts/test_remote_smoke.py --url https://lfd.example.com --token "$LFD_AUTH_TOKEN" --repo /remote/repo/path  # remote TLS smoke (repo required on fresh hosts)
```

When adding features that need manual verification, write or extend a script in `scripts/` rather than documenting a list of commands. One command to run, one environment to verify in.
