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
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v  # API smoke + concurrent-client broadcast checks (live lfd HTTP)
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
swift test --package-path swift --filter CatalogTests  # Catalog DTO / used-by coverage
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
| `e2e-smoke` | ubuntu-latest | `tests/e2e/test_smoke.sh` + `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` |
| `docker-smoke` | ubuntu-latest | `docker version` + `cargo test -p loopflow docker_` |
| `swift-test` | macos-15 | `swift test --package-path swift` |
| `concerto-ui-test` | macos-15 | xcodegen + xcodebuild |

All six must pass for PRs to merge.

## Dependabot workflow

```bash
gh pr list --author app/dependabot
gh run list --workflow CI
```

Weekly dependency PRs come from `.github/dependabot.yml` for `uv`, `cargo`, `swift`, and `github-actions`.

`.github/workflows/dependabot-auto.yml` keeps those PRs zero-touch:
- enable squash auto-merge when a Dependabot PR opens or reopens
- when the `CI` workflow fails on a pull-request run, comment and close the matching PR

Keep `workflow_run.workflows: ["CI"]` in sync with `.github/workflows/ci.yml`. Renaming the CI workflow without updating the Dependabot workflow disables the close-on-red path.

## Rust Tests

Prompt parity and golden prompt tests live in Rust.

```bash
cargo test -p loopflow golden_prompt
uv run python tests/goldens/update_goldens.py   # refresh prompt goldens after prompt changes
```

## E2E Tests

Shell-based workflows for CLI and live HTTP API behavior.

```bash
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
```

Long-running workflow tests for `lf op`:

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
uv run python scripts/verify_skill_sync.py --live  # sync a probe step, then invoke it through Claude and Codex
```

When adding features that need manual verification, write or extend a script in `scripts/` rather than documenting a list of commands. One command to run, one environment to verify in.
