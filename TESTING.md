# Testing

CI runs five test suites. All must pass before merging.

## Quick Reference

```bash
# Run all tests (what CI runs)
uv run pytest tests/                    # Python
cargo test --all                        # Rust
swift test --package-path swift         # Swift package
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'  # Concerto UI
tests/e2e/test_smoke.sh                 # E2E smoke
```

## Python Tests

Unit and integration tests for the loopflow CLI.

```bash
uv run pytest tests/           # All Python tests
uv run pytest tests/test_foo.py::test_bar -v  # Single test
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
| `loopflow-test` | ubuntu-latest | `uv run pytest tests/` |
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
