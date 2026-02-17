# Testing

CI runs five test suites. All must pass before merging.

## Quick Reference

```bash
# Run all checks (what CI runs)
cargo fmt --check                      # Rust formatting
cargo clippy -- -D warnings            # Rust lints (warnings = errors)
cargo test --all                       # Rust tests
uv run pytest python/tests/            # Python tests
swift test --package-path swift        # Swift package tests
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'  # Concerto UI
tests/e2e/test_smoke.sh               # E2E smoke
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
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'
```

## What CI Runs

See `.github/workflows/ci.yml`. Five parallel jobs:

| Job | Runner | Command |
|-----|--------|---------|
| `rust-test` | ubuntu-latest | `cargo fmt`, `cargo clippy`, `cargo test --all` |
| `python-test` | ubuntu-latest | `uv run pytest python/tests/` |
| `e2e-smoke` | ubuntu-latest | `tests/e2e/test_smoke.sh` |
| `swift-test` | macos-15 | `swift test --package-path swift` |
| `concerto-ui-test` | macos-15 | xcodegen + xcodebuild |

All five must pass for PRs to merge.

## Rust Tests

Prompt parity and golden prompt tests live in Rust.

```bash
cargo test -p loopflow golden_prompt
uv run pytest tests/parity/test_prompt_parity.py
```

## E2E Tests

Shell-based workflows for `lf ops` that exercise a full cycle and rebase conflicts.

```bash
./tests/e2e/test_full_cycle.sh
./tests/e2e/test_rebase_conflict.sh
```
