# Testing

CI runs six test suites. All must pass before merging.

## Quick Reference

```bash
# Run all checks (what CI runs)
cargo fmt --check                      # Rust formatting
cargo clippy --all-targets -- -D warnings # Rust lints (warnings = errors)
cargo test --all                       # Rust tests
uv run pytest python/tests/            # Python tests
cd website && uv run python dev.py test # Website tests
swift test --package-path swift        # Swift package tests
cd swift && xcodegen generate && xcodebuild build-for-testing -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' -derivedDataPath .build/xcode-derived-data -disableAutomaticPackageResolution CODE_SIGNING_ALLOWED=YES CODE_SIGNING_REQUIRED=YES CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY=- DEVELOPMENT_TEAM=  # Loopflow UI compile
tests/e2e/test_smoke.sh               # E2E smoke
```

Run at minimum the checks that apply to files you changed. A PR that passes locally but fails CI is a broken gate.

## Changed-Aware Runner

```bash
uv run python scripts/test.py          # run only the suites your branch touched
uv run python scripts/test.py --list   # print the plan, run nothing
uv run python scripts/test.py --all    # run every suite (the full matrix)
```

`scripts/test.py` diffs your branch against `origin/main`, maps changed paths
to the CI jobs above, and runs just those—fast suites first. Use it as the
tight loop while iterating; run `--all` once before you ship.

Slow suites (`loopflow`, `e2e`) stay off in changed-mode even when
their paths change—the run prints why and how to force them:

```bash
uv run python scripts/test.py --loopflow   # force the Loopflow UI suite on
uv run python scripts/test.py --base HEAD~5  # diff against a different ref
```

### Bounded and honest

Every phase runs under a printed wall-clock budget (see
`release/GATE_BUDGET.md`). A phase that overruns is killed—process group and
all—and reported as `TIMEOUT <phase> (budget Ns)`, so **no phase can hang the
gate**. The plan and summary print each phase's budget and elapsed time; on
failure the phase log (and any `.xcresult`) is preserved under
`.lf/tmp/gate/run-<pid>/<suite>/` for one-command repair without opening Xcode.

The summary states **what each suite proves**. The `loopflow` suite compiles
the app and UI-test runners; it does **not** run hosted UI behavior. That real
run is a separately named **required host gate**—it never runs under `--all`
because it needs a permissioned macOS host:

```bash
uv run python scripts/test.py --ui-host   # real hosted LoopflowUITests run
```

See `release/UI_HOST_GATE.md` for the maintained host, the capability it needs,
and how a missing permission is reported (never silently skipped).

Path → suite mapping:

| Changed | Suite | Runs |
|---------|-------|------|
| `rust/`, `Cargo.toml/lock` | rust | `cargo fmt`, `cargo clippy`, then `cargo nextest run --all` (falls back to `cargo test --all`) |
| `python/`, top-level `*.py`, `pyproject.toml` | python | `uv run pytest python/tests/` (scoped to changed `test_*.py` when no source moved) |
| `website/`, `docs/` | website | `cd website && uv run python dev.py test` |
| `swift/` | swift | `swift test --package-path swift -Xswiftc -gnone`, then the multiplatform boundary check |
| `swift/LoopflowMac/`, `swift/project.yml` | loopflow *(slow)* | xcodegen + xcodebuild |
| local store/worktree code, `tests/e2e/` | e2e *(slow)* | CLI smoke |

## Python Tests

Tests for release automation, installers, and repository scripts.

```bash
uv run pytest python/tests/                          # All Python tests
uv run pytest python/tests/test_install_script.py -v # One file
```

## Website Tests

Browser and accessibility tests for `website/`. The dev helper syncs canonical
`docs/` into `website/docs/`, installs the Chromium browser, starts the app, and
runs the test suite.

```bash
cd website && uv run python dev.py test        # All website tests
cd website && uv run python dev.py test -a     # Accessibility tests only
cd website && uv run python dev.py sync-docs   # Refresh generated docs copy
```

## Swift Tests

Tests for the Swift package (models, protocols, shared logic).

```bash
swift test --package-path swift        # All Swift tests
swift test --package-path swift --filter CatalogTests  # Catalog DTO / used-by coverage
swift test --package-path swift --filter SomeTestClass  # Filtered
```

## Loopflow UI Tests

Two levels, split on purpose:

**Compile check** (`loopflow` suite, in `--all` and CI). Compiles the macOS app
and its signed test runners. Swift package tests already exercise the shared
suites un-hosted, so the only unique signal here is that the app target builds:

```bash
cd swift
xcodegen generate
xcodebuild build-for-testing -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' -derivedDataPath .build/xcode-derived-data -disableAutomaticPackageResolution CODE_SIGNING_ALLOWED=YES CODE_SIGNING_REQUIRED=YES CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY=- DEVELOPMENT_TEAM=
```

**Hosted run** (`ui-host` required gate, permissioned host only). Actually runs
`LoopflowUITests`; needs macOS UI-automation permission. Never runs under
`--all`—absence of the permission is a named failure, not a silent skip.

```bash
uv run python scripts/test.py --ui-host
```

See `release/UI_HOST_GATE.md`.

## What CI Runs

See `.github/workflows/ci.yml`. Six parallel jobs:

| Job | Runner | Command |
|-----|--------|---------|
| `rust-test` | ubuntu-latest | `cargo fmt`, `cargo clippy`, `cargo test --all` |
| `python-test` | ubuntu-latest | `uv run pytest python/tests/` |
| `website-test` | ubuntu-latest | `cd website && uv run python dev.py test` |
| `e2e-smoke` | ubuntu-latest | `tests/e2e/test_smoke.sh` |
| `swift-test` | macos-15 | `swift test --package-path swift` |
| `loopflow-ui-test` | macos-15 | xcodegen + xcodebuild |

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

Fresh-store coverage exercises the live SQLite schema. Existing incompatible
databases are rejected with a direct delete-and-recreate instruction; there is
no historical upgrade chain.

Trace capture has focused storage, prompt-accounting, harness, and reader
checks:

```bash
cargo test -p loopflow trace
cargo test -p loopflow journal
cargo test -p loopflow store
cargo test -p loopflow harness::conformance_tests
```

After schema or capture changes, run `lf trace`, `lf context --json`, and
`lf doctor --json` against a fresh local ledger.

## E2E Tests

Shell-based workflows for CLI and worktree behavior.

```bash
tests/e2e/test_smoke.sh
```

Long-running workflow tests for mechanical `lf` commands:

```bash
tests/e2e/test_full_cycle.sh
tests/e2e/test_rebase_conflict.sh
```

## Nightly Package Tests

`.github/workflows/nightly-packages.yml` builds the same native `lf` tarballs as the release workflow. Each runner extracts its tarball and runs:

```bash
package-smoke/lf --version
```

Nightly package artifacts are verification only. They are uploaded for 14 days and not deployed.

## Validation Scripts

`scripts/` contains runnable validation and demo scripts. Use these for branch validation and manual UI walkthroughs.

```bash
uv run python scripts/loopflow-dev.py run-debug     # build and launch Loopflow (macOS)
uv run python scripts/check_swift_multiplatform_boundaries.py  # Stage 01 boundary guardrails
uv run python scripts/verify_skill_sync.py --live  # sync a probe step, then invoke it through Claude and Codex
```

When adding features that need manual verification, write or extend a script in `scripts/` rather than documenting a list of commands. One command to run, one environment to verify in.
