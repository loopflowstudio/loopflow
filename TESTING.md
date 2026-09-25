# Testing

CI runs the full proof matrix in parallel. Local work should run the smallest
proof that can change the next decision.

## Quick Reference

```bash
uv run pytest python/tests/test_gate_bounded.py        # one Python behavior
uv run python scripts/resource_envelope.py             # attribute local disk pressure
uv run python scripts/check_architecture.py            # architecture owners and vocabulary
uv run python scripts/test.py --list                   # affected-suite plan
uv run python scripts/test.py --reuse-passing          # affected suites once per exact tree
uv run pytest python/tests/test_lifecycle_scorecard.py # scorecard behavior
lf telemetry-daily                                     # maintainer report
```

Escalate from a focused behavior to affected suites when crossing a component
boundary. CI and release own the full matrix. Run `scripts/test.py --all` only
to reproduce a matrix failure or when release guidance requires it.

## Changed-Aware Runner

```bash
uv run python scripts/test.py          # run only the suites your branch touched
uv run python scripts/test.py --reuse-passing # reuse only an identical tree + plan pass
uv run python scripts/test.py --list   # print the plan, run nothing
uv run python scripts/test.py --all    # reproduce the serial full matrix
```

`scripts/test.py` diffs your branch against `origin/main`, maps changed paths
to the CI jobs below, and runs just those—fast suites first. `--reuse-passing`
uses a prior pass only when tracked and untracked file content, the worktree,
and the selected command plan are identical. Full and required-host runs never
reuse evidence.

Slow suites (`loopflow`, `e2e`) stay off in changed-mode even when
their paths change—the run prints why and how to force them:

```bash
uv run python scripts/test.py --loopflow   # force the Loopflow UI suite on
uv run python scripts/test.py --base HEAD~5  # diff against a different ref
```

### Bounded and honest

```bash
uv run python scripts/resource_envelope.py
uv run python scripts/resource_envelope.py --recover
```

The resource preflight names the owner and budget for every worktree build,
gate-artifact root, the Home-local Run record store, uv cache, Cargo cache, and
free disk. Budgets live in `performance/budgets.json`: 64 GiB free, 12 GiB per
worktree build, 128 GiB across builds, 16 GiB each for Run records and uv, and
four low-priority verification workers. Recovery removes only allowlisted build
roots from inactive worktrees, old disposable gate output, and entries accepted
by `uv cache prune`. It never removes source, a worktree, gate receipts, Run
bundles, or SQLite state. Unresolved pressure stops before product tests run and
prints the local path that needs an explicit retention decision.

Every phase runs under a printed wall-clock limit. A phase that overruns is
killed—process group and all—and reported as `VERIFICATION BUDGET`, so
**no phase can hang the
gate**. The plan and summary print each phase's `elapsed / budget`; later
phases remain visible as `not_run` after an earlier failure. On failure the
phase log (and any `.xcresult`) is preserved under
`.lf/tmp/gate/run-<pid>/<suite>/` for one-command repair without opening Xcode.

Each invocation also checkpoints a compact JSON record under the repository's
Git common directory:

```text
<git-common-dir>/loopflow/pre-land/runs/<kind>/<run-id>.json
```

This evidence is shared by linked worktrees and survives `.lf/tmp` cleanup.
Records contain operational identity, exact-tree and plan fingerprints,
phase status, and elapsed time—not commands or output. Persistence failure is
a warning and never replaces the underlying test result. Schema-3 records also
carry child CPU, minimum observed free disk, build bytes, and source-attributed
growth. A product assertion remains a product failure; a stopped phase under
sustained macOS `syspolicyd`/`trustd`/`amfid`/`taskgated` pressure is reported as
host-security pressure with the product result explicitly unproven.

Read the same gate evidence through the daily operator flow:

```bash
lf telemetry-daily
```

The scorecard joins accepted provider Turn usage with pre-land phase records. It
prints aggregate values and coverage only—never commands, prompts, output, or
task ids. Missing evidence is `UNKNOWN`, reported zero remains measured, and
small samples stay `COLLECTING` until 20 observations support p95.

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
| any changed path | architecture | `uv run python scripts/check_architecture.py` |
| `rust/`, `Cargo.toml/lock` | rust | `cargo fmt`, `cargo clippy --all-targets`, then draft materialization in a disposable exact-tree worktree and `cargo nextest run --all` (falls back to `cargo test --all`) |
| `python/`, `scripts/*.py`, top-level `*.py`, `pyproject.toml` | python | `uv run pytest python/tests/` (scoped to changed `test_*.py` when no source moved) |
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

After changing `docs/architecture.md` or its website rendering, regenerate the
portable HTML before gate and include it in the same change:

```bash
cd website
uv run python dev.py sync-docs
uv run python ../scripts/render_architecture_html.py
uv run python ../scripts/render_architecture_html.py --check
```

The website suite checks that `docs/architecture.html` matches the rendered
source. A Markdown-only edit can fail that check.

## Swift Tests

Tests for the Swift package (models, protocols, shared logic).

```bash
swift test --package-path swift        # All Swift tests
swift test --package-path swift --filter CatalogTests  # Catalog DTO / used-by coverage
swift test --package-path swift --filter SomeTestClass  # Filtered
```

SwiftPM links GhosttyKit; the Xcode project builds the terminal fallback.
Keep tests that reference Ghostty-only types or helpers inside
`#if canImport(GhosttyKit)`. When changing terminal code or its tests, gate both
configurations: run the focused SwiftPM tests and
`uv run python scripts/test.py --loopflow`. A SwiftPM pass alone does not prove
the Xcode test target compiles.

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

See `.github/workflows/ci.yml`. Ten proof jobs run in parallel and feed the
aggregate `tests-result` check:

| Job | Runner | Command |
|-----|--------|---------|
| `architecture-check` | ubuntu-latest | map every durable owner, public boundary, provider edge, and named shim; reject stale control vocabulary |
| `scratch-clear` | ubuntu-latest | reject landing-only scratch artifacts |
| `rust-lint` | ubuntu-latest | `cargo fmt`, `cargo clippy --all-targets -- -D warnings` |
| `rust-test` | ubuntu-latest | `cargo nextest run --all` |
| `migration-check` | ubuntu-latest | verify migration namespaces/history |
| `python-test` | ubuntu-latest | `uv run pytest python/tests/` |
| `website-test` | ubuntu-latest | `cd website && uv run python dev.py test` |
| `e2e-smoke` | ubuntu-latest | `tests/e2e/test_smoke.sh` |
| `swift-test` | macos-15 | package tests, boundary check, Wave-state render proof |
| `loopflow-ui-test` | macos-15 | xcodegen + app/test-runner compile |

All ten must pass for `tests-result` to pass. `.github/workflows/architecture-drift.yml`
runs the same architecture command every Monday and retains its JSON report for
90 days; four consecutive runs are the time-based architecture KR evidence.

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

For shared repository discovery or CLI dispatch changes, include the PM and
Wave consumers in the focused proof. Follow
[repository-context-check](.lf/skills/repository-context-check.md); global-command
tests alone do not cover cached PM reads from registered directories without Git.

When changing Wave chat operations, include the parent module's HTTP and SSE
tests. Selecting only `runner::tests` or steering-named tests misses them.

```bash
cargo nextest run -p loopflow --lib -E 'test(controller::wave::)' --no-fail-fast
```

Retired operations must be rejected without journaling, while ordinary messages
and bare interrupts retain their behavior.

When changing Task controls, include the GitHub-cache integration tests as well
as controller tests. Bare interrupts prove local control during GitHub outages;
steering publishes to Linear and belongs with the mocked Linear boundary tests.

```bash
cargo nextest run -p loopflow --test task_github_cache_tests --no-fail-fast
```

When changing Linear response shapes, run the client tests and PM-operation
consumers together. Team migration also reads issue comments; its fixtures must
include the requested pagination metadata.

```bash
cargo nextest run -p loopflow --lib -E 'test(pm::linear::) | test(ops::pm::) | test(ops::linear_observe::)' --no-fail-fast
```

Session-command fixtures must work without an installed `lf`. Supply an `LF_BIN`
fixture, restore it afterward, and serialize environment changes with
`test_env_lock`. Reuse `TestLfBinGuard` in Task controller tests; keep Session
spawning mocked. Reproduce executable-resolution failures with the compiled test
binary, `LF_BIN` and `CARGO_BIN_EXE_lf` unset, and a PATH containing Git but no `lf`.

For worktree creation or checkout-refresh changes, build the current CLI before
running its Python behavior tests:

```bash
cargo build -p loopflow --bin lf
LOOPFLOW_TEST_LF="$PWD/target/debug/lf" uv run pytest python/tests/test_checkout_refresh.py
```

The background-push regression holds Git until the CLI exits, then verifies
upstream tracking and a subsequent rebase. Immediate local pushes can hide
broken pipes that interrupt Git after the remote ref moves; background children
must use stdio that survives their parent's exit.

Builtin skill Markdown is compiled into Rust. After editing it, run the builtin
contract tests alongside the relevant behavior tests; PR renderer tests alone do
not cover the prompts that generate their input. Update obsolete assertions to
match the intended contract instead of restoring retired commands in the prose.

```bash
cargo test -p loopflow --lib engine::builtins::tests
```

Prompt parity and golden prompt tests live in Rust.

```bash
cargo test -p loopflow golden_prompt
uv run python tests/goldens/update_goldens.py   # refresh prompt goldens after prompt changes
```

Changes to builtin `LOOPFLOW.md` affect every prompt golden. Regenerate and
review them before gate. For migration regressions, use the materialized Rust
test path above: inspect historical fields at their migration boundary, then
finish the upgrade and verify the current schema.

Fresh-store coverage exercises the live SQLite schema. Populated historical
fixtures exercise the migration chain and verify retained facts. A fresh-store
pass alone does not prove that an existing Home can upgrade without losing work.

Run records have focused storage, harness, reducer, and reader checks:

```bash
cargo test -p loopflow run_record
cargo test -p loopflow journal
cargo test -p loopflow store
cargo test -p loopflow harness::conformance_tests
```

When changing harness event mapping, run the recorded-trace conformance tests
alongside the provider's unit tests. Keep trace expectations aligned with the
event contract, including durable final-answer receipts and usage checkpoints.

After Run-record or schema changes, run `lf runs --json`, `lf usage --json`, and
`lf doctor --json` against a fresh local Home.

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

Exercise Linear expiry and rejection through an installed development CLI:

```bash
# Inside a disposable Linux container, after candidate promotion:
uv run tests/e2e/linear_oauth.py --lf /root/.local/bin/lf
```

Give the container `--add-host api.linear.app:127.0.0.1`, Python, Git, and
`uv`. Install the published CLI/daemon fallback and promote the candidate with
`lf install promote --from-build ...` first. That Home needs a registered
repository and `tmux` so promotion can verify its daemon. Never mount a real
Home or credentials into this container: the fixture replaces its Linear row
and seeds planning data in the selected development store.

The fixture serves synthetic Linear HTTPS on port 443 with a temporary CA
trusted only by its CLI children. It enters through the installed launcher and
asserts fresh planning, encrypted rotation, bounded replay, and rejected-refresh
preservation with and without a snapshot. Its JSON receipt identifies the CLI
digest and installation. This proves the installed Linux path against simulated
provider responses; live Linear behavior and the operational reliability window
remain separate evidence. No production endpoint override is needed.

Exercise first installation and recovery inside a disposable Ubuntu container:

```bash
uv run --no-project --python 3.14 /fixture/install_bootstrap.py
```

Copy `tests/e2e/install_bootstrap.py`, `release/install.sh`, and a Linux candidate
pair to `/fixture/install_bootstrap.py`, `/fixture/install.sh`, and
`/fixture/bin/{lf,lfd}`. Build the pair in a separate disposable source snapshot
after `canonicalize_migrations.py --materialize-for-tests`, using release
provenance and published migration authority only in that isolated build.
The runtime container needs curl, OpenSSL, CA certificates, Git, useradd,
runuser, and uv; run the proof as root without host Home mounts. Optionally copy
a checksum-verified older released pair into `/fixture/prior` to exercise the
external-installer transition.

The script creates separate OS accounts and a local HTTPS release endpoint.
It exercises the real CLI, verified shell installer, store creation, repeat
installation, missing-daemon repair, checkout preservation, and recovery after
a forced activation failure. Transport is simulated; this does not replace a
public release-channel demo. Discard the container afterward.

## Nightly Package Tests

`.github/workflows/nightly-packages.yml` builds the same native `lf` tarballs as the release workflow. Each runner extracts its tarball and runs:

```bash
package-smoke/lf --version
package-smoke/lf --help
package-smoke/lf --list
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
