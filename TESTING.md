# Testing

CI runs three test suites. All must pass before merging.

## Quick Reference

```bash
# Run all tests (what CI runs)
uv run pytest tests/                    # Python
swift test --package-path swift         # Swift package
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'  # Concerto UI
```

## Python Tests

Unit and integration tests for the loopflow CLI.

```bash
uv run pytest tests/           # All Python tests
uv run pytest tests/test_foo.py::test_bar -v  # Single test
uv run pytest tests/parity/ -v  # Rust/Python prompt parity
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

See `.github/workflows/ci.yml`. Three parallel jobs:

| Job | Runner | Command |
|-----|--------|---------|
| `loopflow-test` | ubuntu-latest | `uv run pytest tests/` |
| `swift-test` | macos-15 | `swift test --package-path swift` |
| `concerto-ui-test` | macos-15 | xcodegen + xcodebuild |

All three must pass for PRs to merge.
