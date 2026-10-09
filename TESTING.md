# Testing

Publish checkpoints with working notes in `scratch/`; hosted CI defers the full
matrix until scratch is clear. `lf pr submit`, `lf land`, and `lf pr land`
clear scratch before pushing a landing candidate. Scratch-free PRs, including
small changes and Dependabot updates, run the full test matrix in parallel.
Implement/compress only build changed code and run its focused test. Gate owns
affected suites and automated acceptance once; see the verification cadence in
[AGENTS.md](AGENTS.md#verification-cadence). Scratch keeps one check-result line.
Required checks run unattended; defer unavailable checks to capable CI and
leave people's judgment to demo/review. Neither blocks earlier Flow steps.

For checkpoint PRs, `scratch-clear` reports deferred tests and `tests-result`
and the matrix jobs skip. That is not a passing test result. The required merge
queue always rejects scratch artifacts and runs full tests before merging.
Missing scratch, classifier failures, and unexpected skipped candidate jobs
still fail the aggregate. Restoring scratch on a later PR head defers its tests.

A new PR update cancels the previous CI run for that PR. Main and merge-group
runs remain independent.

Dependabot checks weekly and groups minor/patch updates by ecosystem. Its
auto-merge workflow uses the `DEPENDABOT_MERGE_TOKEN` Actions secret, sourced
from Doppler, with repository Contents and Pull requests write access. Renew
the token before expiry. The built-in `GITHUB_TOKEN` cannot enqueue PRs into
the required merge queue. Successful PR CI explicitly enqueues the unchanged
Dependabot head when it is not already queued; queue CI still gates merging.

CI Rust cache keys include the root workspace's build profiles, which the cache
action's member-manifest discovery omits. Profile changes get fresh dependency
caches; version-only releases retain them. Main publishes the shared caches;
PRs and merge groups restore them without accumulating private copies. Cache
restoration alone never skips tests. After a merge, main can reuse a
successful merge-group CI run for the identical SHA and workflow, linking that
run in its summary. Missing or unreadable results trigger the full matrix. Main still
restores the shared caches and runs each job whose cache misses; those jobs
refresh their caches and must pass. Candidate PRs and merge groups always execute
every check.

Swift cache keys include the tracked Swift tree, compiler, and SDK. Successful
main jobs save the build and source hashes/timestamps. After checkout, CI restores
an input's cached timestamp only when its contents still match; changed inputs
keep their new timestamps and rebuild. Missing timestamp metadata leaves normal
Swift build detection in control. Source or toolchain changes may require a new
main cache fill; PRs and merge groups only restore shared caches.

CI wraps the expensive Rust, Swift, and Xcode commands with native
`/usr/bin/time`: `-v` on Linux and `-l` on macOS. Their existing job logs retain
elapsed time, user/system CPU seconds, maximum RSS, faults, and context switches
on success or command failure. Swift's failure artifact retains the same output.
Runner cancellation may prevent a final report. Skipped checks have no new measurement.
Compare the same command, source,
runner/toolchain, and cache state; sum user/system time to compare CPU work with
elapsed time. A timing difference alone does not establish contention.

[GNU time](https://www.gnu.org/software/time/manual/html_node/Memory-Resources.html)
reports maximum RSS in KiB; macOS reports bytes. These are the native command
accounting peaks, not simultaneous aggregate process-tree or host physical memory.
Work performed by external services may be excluded, so this does not measure
Docker daemon memory or establish a safe parallel-worker count. Local gate CPU
receipts remain under `.lf/tmp/gate`; use the same native timing command for a
focused local comparison.

The introductions in `README.md` and `docs/index.md` share the same text. When
editing either introduction, update both and run
`uv run --project website --extra test pytest website/tests/test_readme_index_sync.py`.

`cargo test -p loopflow --test agent_startup_tests` exercises bare `lf` and
Session reconnect to a stand-in provider with an empty environment and no
credentials. CI's ordinary Rust network isolation covers it. It bounds provider
and Git launches, SQLite statement/returned-row counts, and time to provider
handoff; [native readiness measurements](scripts/benchmarks/agent-startup/README.md)
remain a separate opt-in benchmark.

Changes to builtin Flows affect the parser, graph, Task controller and CLI fixtures.
Run the affected controller progression and CLI behavior tests as well as graph
checks; use authored fixture Flows when a test needs a fixed sequence independent
of product defaults.

Changes to interactive/headless selection also run `default_conversation_tests`
and `context_launch_tests` alongside `flow_tests`: bare `lf`, explicit `-b`, and a
Flow invoking the `default` skill must preserve their distinct launch modes.
Prompt fixtures read the harness's actual inputs, including context files and
stdin, rather than assuming everything remains in argv.

Provider fixtures must read the launch's actual context channel. When a final
sync adds fixtures that inspect a changed transport, run those focused tests
before arming; the earlier gate did not cover the newly combined behavior.

## Quick Reference

```bash
uv run pytest python/tests/test_gate_bounded.py        # one Python behavior
uv run python scripts/resource_envelope.py             # attribute local disk pressure
uv run python scripts/check_architecture.py            # architecture owners and vocabulary
uv run python scripts/test.py --list                   # affected-suite plan
uv run python scripts/test.py --reuse-passing          # affected suites once per exact tree
uv run pytest python/tests/test_lifecycle_scorecard.py # scorecard behavior
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-check --samples 1
lf telemetry-daily                                     # maintainer report
```

The opt-in desktop command exercises the native outline and retained Task panes.
Use it for relevant UI changes; see [performance measurements](performance/README.md)
for full sampling, comparison and the explicit capture-versus-presentation boundary.

Escalate from a focused behavior to affected suites when crossing a component
boundary. CI and release own the full matrix. Run `scripts/test.py --all` only
to reproduce a matrix failure or when release guidance requires it.

## External network isolation

`tests/e2e/native_titles.py` exercises real Claude/Codex TUIs against a local fake
API with fresh provider and Loopflow homes. Pass `--lf`, `--native`, `--provider`
and `--output`; prepare its inline dependencies with `uv run ... --help`, then run
it through `scripts/test_network.py` with `uv run --offline`. It checks first-request
OSC titles and native manual rename. `--headless` checks new `claude -p` and
`codex exec` conversations, saving naming readback beside stdout/stderr. Claude
saves the name; Codex currently records the missing embedded naming endpoint.
Old conversations and resume repair are out of scope. No installed
accounts, host shims, or cmux windows participate.
The probe uses native terminal-title defaults and checks the latest OSC title,
so an earlier correct title cannot hide a later overwrite. The TUI probe does not
cover embedded Codex or the configured shim; `--headless` records `codex exec` separately.

`uv run python scripts/test_task_installation.py --native-titles` builds a
release-shaped candidate only inside Docker and exercises published promotion
under a disposable OS account. It checks preview, existing settings and symlinks,
failed hook installation, same-candidate retry and repeated installation. CI's
installation job runs this proof; it never touches the host installation.

```bash
uv sync
uv run --no-sync python scripts/test_network.py uv run --no-sync pytest python/tests/
```

Prepare dependencies and compile binaries before isolating test execution.
The test runner and CI use `test_network.py` for Python, Rust test executables,
Swift tests, website tests and smoke tests. Each invocation proves loopback works
and an external connection is denied; the OS boundary covers descendants too.
Git transport is file-only. Local bare remotes and loopback protocol fixtures
remain available. No developer shell or global Git configuration changes.

macOS uses `sandbox-exec`; Linux uses passwordless sudo, `unshare --net` and `ip`
to create a private network namespace, enable loopback and restore the caller's
user identity. macOS permits only the fixed `/bin/ps` system reader outside the
sandbox, matching Desktop's existing headless profile; it cannot launch fixture
children or open service connections. Unsupported isolation fails explicitly. Installation-container
proofs disconnect their external Docker network after compiling, then run the
same denial/loopback probes before executing tests. Display diagnostics remain opt-in.

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
and the selected command plan are identical. Full and optional hosted runs never
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
gate-artifact root, the Machine-local Session artifact store, uv cache, Cargo cache, and
free disk. `performance/budgets.json` sets a 64 GiB cleanup target, a 32 GiB
emergency disk reserve, and four
low-priority verification workers. Total build size is measured, not capped.
Individual root thresholds are cleanup signals, not gates: an oversized sibling names its
worktree in a warning and does not block a healthy checkout. The 24 GiB build threshold
identifies cleanup candidates, not a limit on what a worktree may build.

Verification preflight and explicit `--recover` reclaim inactive worktrees'
allowlisted build roots only when unchanged for at least 24 hours and either
oversized or needed to restore the cleanup target. They also prune old disposable
gate output and entries accepted by `uv cache prune`. Cleanup runs before
verification, not on an independent timer. A busy uv cache is reported immediately
without waiting for its reader locks, including the parent `uv run`. Other cleanup
and above-reserve verification continue. To reclaim uv entries, run
`UV_LOCK_TIMEOUT=0 uv cache prune` directly when other uv commands are idle.
A nonblocking host lock permits only
one cleaner at a time across parallel workers. Verification prints each cleanup
result and reclaimed size, including failed pruning attempts, and saves them in
its resource receipt. All active
worktrees, source, worktree metadata, gate receipts, captured payloads, and SQLite
state are retained, including the current checkout's active builds. Only disk
below the emergency reserve stops product tests;
low-disk output names the largest inactive build roots. Measurement failures
remain explicit because unmeasured capacity cannot establish a safe build.
If eligible caches cannot restore 64 GiB free, verification warns and continues
above 32 GiB. The reserve is a last-resort stop, not a forecast of a build's disk
requirements or a host-wide reservation for concurrent builds.

Gate roots with only recent output consume no recovery slots, leaving capacity
for stale build cleanup. Removing even an empty old directory counts toward the
same per-pass root limit.

An empty plan or an identical passing result reused with `--reuse-passing`
returns before resource scans and cleanup. Neither executes a build nor writes
a new check receipt. Changed content or commands require fresh verification and
the normal resource checks.

Busy uv cache pruning times out after 15 seconds while other eligible recovery
continues. Retry `uv cache prune` after other uv processes exit, or run recovery
from an activated `.venv`; an enclosing `uv run` can hold the cache lock.

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

The scorecard joins recorded Session usage and duration with Task PR landing and pre-land phase records. It
prints aggregate values and coverage only—never commands, prompts, output, or
task ids. Missing evidence is `UNKNOWN`, reported zero remains measured, and
small samples stay `COLLECTING` until 20 observations support p95.

“Recorded agent attempt → merge” includes earlier and unfinished captured inputs attributed
to the exact PR. See [metric definitions and coverage limits](performance/README.md).

Desktop's `swift` suite builds the app through the test-target dependency and
runs model and production-view tests through `scripts/test_desktop.sh`, which
denies WindowServer connections even on a logged-in host. No Automation
permission is needed. Only macOS's setuid `/bin/ps` leaves the sandbox so CLI
process observation still works. `DesktopHeadlessTests` checks the four Work states and
selection through the real button action. Gate and CI use this same suite.

Window/Metal/PTY integration tests are optional display-session diagnostics:

```bash
LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --no-parallel
uv run python scripts/test.py --ui-host  # optional XCUITest diagnostic
```

They are never acceptance prerequisites for a headless Flow. See
`release/UI_HOST_GATE.md` for optional hosted execution and cleanup.

Path → suite mapping:

| Changed | Suite | Runs |
|---------|-------|------|
| any changed path | architecture | `uv run python scripts/check_architecture.py` |
| `rust/`, `Cargo.toml/lock` | rust | `cargo fmt`, `cargo clippy --all-targets`, then draft materialization in a disposable exact-tree worktree and `cargo nextest run --all` (falls back to `cargo test --all`) |
| `python/`, `scripts/*.py`, top-level `*.py`, `pyproject.toml` | python | `uv run pytest python/tests/` (scoped to changed `test_*.py` when no source moved) |
| `website/`, `docs/` | website | `cd website && uv run python dev.py test` |
| `swift/` | swift | `scripts/test_desktop.sh -Xswiftc -gnone`, then the multiplatform boundary check |
| `swift/LoopflowMac/`, `swift/project.yml` | loopflow *(slow)* | xcodegen + xcodebuild |
| local store/worktree code, `tests/e2e/` | e2e *(slow)* | CLI smoke |

## Python Tests

Tests for release automation, installers, and repository scripts.

```bash
uv run pytest python/tests/                          # All Python tests
uv run pytest python/tests/test_install_script.py -v # One file
```

After changing Infrastructure or Release's `GOAL.md` schedules, run
`uv run pytest python/tests/test_release_automation.py` even when no Python files
changed. It checks telemetry and release cadence under their owning Waves.

After changing the CLI guide, agent API docs, published Loopflow skill, or
builtin `LOOPFLOW.md`, run the shared inspection-command check even when no
Python files changed:

```bash
uv run pytest python/tests/test_loopflow_skill_alignment.py
```

## Website Tests

Browser and accessibility tests for `website/`. The dev helper syncs canonical
`docs/` into `website/docs/`, installs the Chromium browser, starts the app, and
runs the test suite.

Homepage touch targets must retain their minimum size with fallback fonts, whose
metrics differ across macOS and Linux. Run the website suite after the final CSS
or capture edit, including the fallback-font touch-target case.

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

App build and headless tests for models, protocols, and production views.

```bash
cargo build -p loopflow --bin lf # Required by the real CLI transport proof
scripts/test_desktop.sh -Xswiftc -gnone # Headless Swift tests
swift test --package-path swift --filter CatalogTests  # Catalog DTO / used-by coverage
swift test --package-path swift --filter SomeTestClass  # Filtered
```

Pass `--no-parallel` explicitly: main-actor observations share scheduling.
After integrating Session kind changes, run `scripts/test_desktop.sh --filter
'TaskFlowTests|WorkspaceNavigationTests'` to compile Desktop consumers and exercise
participation and navigation fixtures together.
Window and terminal integration suites opt in with `LOOPFLOW_NATIVE_TESTS=1`;
they are reported as skipped in headless runs, not counted as passing.

After upgrading GhosttyKit or its bundled shell integration, run
`scripts/test_desktop.sh --filter GhosttyShellBlockTests` to check the actual
shell output for prompt headers, command boundaries and exit status.

`scripts/prove_wave_surface_states.sh` is an optional demo capture: it launches
windows and therefore needs a display session. It is not in gate or CI.

Failed Swift CI runs retain a `swift-tests-<run>-<attempt>` artifact for seven
days with the toolchain, output log, and Swift Testing event stream. When console
output stops after compilation, inspect `events.jsonl` for the last started test
without a matching end event. SwiftPM buffers console output, so silence alone
does not establish that test execution never began.

The Swift transport suite launches `target/debug/lf` against a temporary Machine.
Build the current CLI before running it; an existing developer build can hide
a missing prerequisite in a clean checkout. CI and `scripts/test.py --swift`
include this build.

Transport tests register `Process.terminationHandler` before launch and await
its notification. Do not use `waitUntilExit()` after an async suspension: the
test can resume on a different thread and hang in Foundation's run-loop wait
even after the child exits. When a Swift run stops reporting progress, sample
the test helper process before changing timeouts; cleanup can be the blocker.

In asynchronous terminal tests, observe the surface after each wake-up before
checking the deadline. A busy main actor can resume after the deadline even
when the PTY produced its output in time; do not fail on a pre-sleep snapshot.

For async model readers, keep generic values crossing actor boundaries
`Sendable`. A newer local Swift compiler can accept code rejected by CI's
toolchain; record `swift --version` with compile evidence when investigating
concurrency diagnostics.

Keep native code compatible with CI's Xcode toolchain in both SwiftPM and Xcode
builds. In particular, `isolated deinit` requires Swift 6.2; CI's Xcode 16.4
cannot enable it even with an experimental feature flag. Put teardown that can
run off-actor in a resource owner and exercise release from outside the main
actor. A newer local compiler's pass does not establish older-toolchain support.

SwiftPM links GhosttyKit; the Xcode project builds the terminal fallback.
Keep tests that reference Ghostty-only types or helpers inside
`#if canImport(GhosttyKit)`. Keep file-local helpers inside the enclosing
whole-file platform gate. App code outside `#if GHOSTTY_ENABLED` compiles in
the fallback too: a type it names must also live outside that block. When changing terminal code or its tests, gate both
build configurations: run the headless Swift suite and
`uv run python scripts/test.py --loopflow`. A SwiftPM pass alone does not prove
the Xcode test target compiles.

## Loopflow UI Tests

Two levels, split on purpose:

**Compile check** (`loopflow` suite, in `--all` and CI). Compiles the macOS app
and its signed test runners with the terminal fallback. Swift package tests
exercise the shared suites with GhosttyKit:

```bash
cd swift
xcodegen generate
xcodebuild build-for-testing -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' -derivedDataPath .build/xcode-derived-data -disableAutomaticPackageResolution CODE_SIGNING_ALLOWED=YES CODE_SIGNING_REQUIRED=YES CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY=- DEVELOPMENT_TEAM=
```

CI caches this build only for identical tracked Swift inputs, Xcode, SDK and
XcodeGen versions. Matching source timestamps preserve compilation across
checkouts; changed inputs start fresh, including signed entitlement state.
Only main saves caches. PRs and merge groups still run the compile command and
build the current Rust control tools.

**Optional hosted diagnostic** (`--ui-host`). Runs `LoopflowUITests` on a
permissioned display host. It is excluded from `--all` and is never required
for Task acceptance. See `release/UI_HOST_GATE.md`.

## What CI Runs

See `.github/workflows/ci.yml`. These check jobs run in parallel and feed the
aggregate `tests-result` check:

| Job | Runner | Command |
|-----|--------|---------|
| `architecture-check` | ubuntu-latest | map every durable owner, public boundary, provider edge, and named shim; reject stale control vocabulary |
| `scratch-clear` | ubuntu-latest | defer checkpoint PR tests; reject scratch artifacts on queue/main |
| `rust-lint` | ubuntu-latest | `cargo fmt`, `cargo clippy --all-targets -- -D warnings` |
| `rust-test` | ubuntu-latest | `cargo nextest run --all --no-fail-fast` |
| `migration-check` | ubuntu-latest | verify migration namespaces/history |
| `python-test` | ubuntu-latest | `uv run pytest python/tests/` |
| `website-test` | ubuntu-latest | `cd website && uv run python dev.py test` |
| `e2e-smoke` | ubuntu-latest | `tests/e2e/test_smoke.sh` |
| `task-installation` | ubuntu-latest | `uv run python scripts/test_task_installation.py` |
| `swift-test` | macos-15 | app build, headless model/view tests, boundary check |
| `loopflow-ui-test` | macos-15 | xcodegen + app/test-runner compile |

Candidate and merge-group jobs must all pass for `tests-result` to pass. Both CI
and the local runner collect every Rust test failure with `--no-fail-fast`.
`.github/workflows/architecture-drift.yml`
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

Match CI's current stable toolchain before accepting a local lint pass:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
rustup update stable --no-self-update
rustc +stable --version
cargo +stable clippy --version
cargo +stable fmt --all -- --check
cargo +stable clippy --all-targets -- -D warnings
```

The installation harness pulls `rust:bookworm` on each run to follow stable
alongside CI's Rust jobs, and logs its compiler version. After adopting a newer
standard-library API, run `uv run python scripts/test_task_installation.py` too;
a local lint pass does not prove that the disposable installation builds.

CI installs stable on each run. An older local compiler can miss new Clippy
lints and standard-library deprecations. Put rustup's proxies first on `PATH`
so Cargo subcommands cannot select an older Homebrew Clippy or rustfmt.

Task cancellation uses a real child CLI with a disposable scorecard effect.
Build the sibling CLI before running this library-only proof:

```bash
cargo build -p loopflow --bin lf
cargo test -p loopflow --lib task_stop_waits_for_selected_step_after_driver_death
```

The proof covers retained and released driver claims, child exit, interrupted
Process history, and an unresolved mechanical outcome. Linux also holds interrupt
cleanup after the effect exits to exercise settlement ordering. It uses no
configured provider or installed Machine.

For shared repository discovery or CLI dispatch changes, include the PM and
Wave consumers in the focused check:

```bash
cargo test -p loopflow --test wave_resolution_tests --test wave_resolution_matrix --test global_commands
```

When adding a command with `--wave`, classify its selection in
`wave_resolution_matrix` and run that suite alongside the command's own tests.
Cover repository defaults and explicit Wave selection separately from ambient
Wave resolution.

Preserve fixtures for registered Wave directories without Git metadata. Adding
Git would hide the cached-PM context regression; global-command tests alone do
not cover it.

Work-command dispatch also needs the registration lifecycle and repository
ownership tests. Empty registrations can be forgotten from their registered
directory without Git metadata; Work operations still enforce repository ownership.

```bash
cargo nextest run -p loopflow --test status_tests --test wave_repository_ownership --no-fail-fast
```

Task decision feedback has a focused store-and-driver check:

```bash
cargo test -p loopflow --lib task_decision_live_unblock_returns_feedback_without_navigation -- --test-threads=1
```

It completes a keyed Ask while the decision is active and verifies that feedback
returns to the same captured conversation without choosing Flow navigation.
Provider effects are simulated. The name's `live` refers to the active local
decision, not configured-provider acceptance. Structured decision output owns
Advance, Iterate, or Blocked with a required reason; feedback alone is no verdict.

Task stall check uses `session_record::activity::tests`, `task_live_unblock`, and
Swift `TaskFlowProofTests`. The sampler retains PID/start identity and cumulative
CPU for the body and descendants in Session events. Five quiet minutes
requires samples no more than 45 seconds apart; the worker samples every 15
seconds. Unit tests advance a simulated clock; the CLI/desktop check samples a
real sleeping process with a seeded five-minute history. Neither establishes a
five-minute configured provider stall. Preserve CPU-active silence, fresh-event,
missing-sample and PID-reuse counterexamples when changing the projection.
Repeated samples of a reused body PID must remain Unknown; they cannot replace
the observer's original body identity and later establish a stall for that captured input.

See [boundary-specific checks](#boundary-specific-checks) for Task controls,
Linear response shapes and isolated Session fixtures.

`process_ownership_tests::interruption_records_the_process_without_a_fabricated_signal_name`
checks the OS exit, durable Process outcome and absence of its owned scorecard child.
It retains child output for failures. On Linux, `cc` builds the test-only
`support/hold_group_kill.c` interposer: after delivering the real group kill it
holds the signal hook for 200 ms, exposing normal command return racing cleanup.
The fixture asserts that this scheduling point was reached. This is controlled
ordering evidence, not a claim about how long a hosted signal handler paused.
Other Unix platforms exercise the ordinary interruption path.

Exercise a deciding step's structured output with real Codex and a local
Responses fixture:

```bash
uv run --script tests/e2e/codex_connect.py --codex "$(command -v codex)" \
  --lf target/debug/lf --launch --flow-decision-retry replace \
  --output .lf/tmp/native-flow-decision
```

`--flow-decision-retry missing` exhausts the corrections of an invalid answer;
`replace` proves a failed turn decides nothing. `--public-connect` covers a
live headless-to-terminal handoff.
`--shared-provider-home` proves Loopflow and plain Codex share one home signed
in as one stored account at a time: switching, saved-back logins, isolation,
and provider conversation IDs in `lf session`.
`tests/e2e/claude_shared_home.py --claude "$(command -v claude)" --lf
target/debug/lf --output <dir>` proves the same for Claude with synthetic
logins and a local endpoint: a Claude started after a switch sends the login
Loopflow installed, a shared launch gets no account home or credential
variable, and an isolated launch stays in its account's home. On macOS it
writes and removes one Keychain item scoped to its temporary config directory.
The fixture copies the candidate, uses private Machines and stops only its identified
engine children. Native execution uses synthetic Responses, not configured
accounts or installed data. Ordinary retry, usage, binding and review behavior
belong in `session_lifecycle_tests`; Chapter convergence belongs in
`ops::chapter::tests`, including interrupted rotation and second-Machine sync.

`planning_reconnect_tests` runs the public work-watch and Flow reconnect fixtures
on Linux, using disposable TLS trust and synthetic Linear state. They exercise
repository/Wave scope without Task selection, selection changes, stdin close/reopen,
lost replies and independent propagation during rejected field delivery. The
portable `work_watch` offline-completion test requires a foreground delivery error
with no Task selected; local frame propagation alone cannot establish sync lifetime.
Run these through the same external-network denial wrapper as other CLI fixtures.

Task-planning fixtures also need an explicit `LF_BIN`: Task status validates
launch authority before reconciling a user merge. Pin the test executable when
no child is launched; an installed `lf` on PATH can hide this missing fixture.

After Project ownership or uniqueness changes, run the Task-planning consumers
against CI's materialized schema. Planning sync already creates the local Project;
registered-Task fixtures must reuse it rather than insert a second owner.

Planning-owner changes must cover context/catalog reads, Wave relocation, Task
initialization, Workflow completion/reopening and PR delivery fixtures alongside
the new planning APIs. Seed stored definitions explicitly; writing a fixture file
is not a planning mutation.
An existing checkout fixture must settle initialization before testing delivery.
Prompt fixtures nested in the source tree must fence Git discovery so the enclosing
checkout cannot become their stored Wave owner. Verify them inside a Git checkout;
a source archive alone does not exercise that boundary.
Include the shared DTO fixtures and Desktop planning states in the affected gate.
Repository-wide Linear fixtures must return comments only for their owning issue;
exercise acquisition in a different order from Task creation. Process-sensitive
cleanup fixtures must observe child readiness before inspecting live executables.

```bash
uv run python scripts/materialize_rust_tests.py -- cargo nextest run -p loopflow --lib -E 'test(ops::pm::task_planning_tests::)' --no-fail-fast
```

Every CLI fixture must select an explicit disposable `LF_HOME` and pin
`LF_BIN` to the compiled test CLI. Clear inherited `LF_*` execution authority.
Without an explicit experiment a source CLI forwards to the installed CLI and
main Machine. Children use the same Machine and executable; PATH cannot choose a
second store. A Machine's database is always `$LF_HOME/loopflow.db`.

When editing the repeated Task body, exercise every step on two passes and
saved-decision recovery. Keep loop-or-next after work and review so navigation
cannot skip a later review.

Include the CLI/desktop unblock projection when changing builtin Flow composition:

```bash
cargo test -p loopflow --test task_initialization_tests task_live_unblock
```

Fixtures targeting a named boundary should resolve its node ID in the expanded
graph. A hard-coded step index can silently select a different skill when
a nested Flow gains a step.

For worktree creation or checkout-refresh changes, build the current CLI before
running its Python behavior tests:

```bash
cargo build -p loopflow --bin lf
LOOPFLOW_TEST_LF="$PWD/target/debug/lf" uv run pytest python/tests/test_checkout_refresh.py
```

The background-push regression holds Git until the CLI exits, then verifies
upstream tracking and a subsequent sync. Immediate local pushes can hide
broken pipes that interrupt Git after the remote ref moves; background children
must use stdio that survives their parent's exit.

Builtin skill Markdown is compiled into Rust. After editing it, run the builtin
contract tests alongside the relevant behavior tests; PR renderer tests alone do
not cover the prompts that generate their input. Update obsolete assertions to
match the intended contract instead of restoring retired commands in the prose.

```bash
cargo test -p loopflow --lib engine::builtins::tests
```

When changing the builtin catalog or Flow composition, also exercise discovery.
Listings keep effective skills and authored flows separate and show their sources
and invocations; `lf flow show` expands a flow's steps.

```bash
cargo test -p loopflow --test cli_discovery list_preserves_kinds_overrides_sources_and_reserved_invocations
```

Catalog retirement also affects historical migration tests. Keep their persisted
names and data-preservation assertions at the migration boundary; current catalog
resolution belongs in engine tests. Include the legacy Flow repair check:

```bash
cargo test -p loopflow --lib legacy_task_flow_repair
```

Skill export tests use isolated homes and cover builtin/global definitions,
personal agent directories and pruning. Repository `.lf/skills` are local
execution overrides; they are not exported by `sync-skills`.

```bash
cargo test -p loopflow --lib engine::skills::tests
```

After editing embedded skills, directions, surfaces, or prompt assembly, run
the Rust golden prompt check even for Markdown-only changes. If the mismatch
reflects the intended prompt change, regenerate the snapshots, review their
diff, and rerun the check before gate.
Terminology-only replacements count as prompt changes; include the golden
check in their focused verification even when no prompt assembly code changed.

```bash
cargo test -p loopflow --test golden_prompt
uv run python tests/goldens/update_goldens.py   # refresh prompt goldens after prompt changes
```

For document gathering or deduplication changes, run the complete context suite
alongside the delivery checks; a test-name filter alone misses ancestor coverage.
Rendering gathered documents must include the production deduplication step.

```bash
cargo test -p loopflow --test context_tests
cargo test -p loopflow --lib context_delivery
cargo test -p loopflow --lib skill_launch
```

Run the launch checks after syncing CLI changes; they parse the current flags
before verifying delivery to the provider.

Changes to builtin `LOOPFLOW.md` affect every prompt golden. Regenerate and
review them before gate. The `skill_launch` checks above cover terminal launches.
Keep prose contracts in builtin tests; launch tests should prove that the
canonical document is included.
For migration regressions, use the materialized Rust
test path above: inspect historical fields at their migration boundary, then
finish the upgrade and verify the current schema. When chapter triggers change,
include Task consumers: durable work reservation retains Started after
failure. Use CI's materialized migration graph for trigger
changes; an ordinary draft build may omit the trigger. Experimental Machines hold
this build's exact schema: use a fresh Machine after changing it. Preserve
populated historical fixtures for published migration coverage. A Task's draft is
edited in place, so test the released frontier against the finished draft and
write no test for a schema that existed only between two edits.

For manual migration check in a shared checkout, materialize only in a disposable
source copy that includes the current tracked and untracked inputs. Materialization
can change package versions, the lockfile, registry and migration files; another
execution can commit those temporary changes before cleanup. Keep the assigned checkout
on its authoring schema and leave the live Machine untouched. A copy without Git
metadata must set `LOOPFLOW_BUILD_PROVENANCE=development` when exercising
source behavior; otherwise the build defaults to release provenance. It also
cannot prove fixtures that require `git rev-parse HEAD`: run those in
the assigned checkout when its schema suffices, and report that separate check.
Do not count a fixture setup failure as a passing materialized test.

After adding, renaming or removing a public command owner or concept, run
`uv run python scripts/check_architecture.py`;
retained tables and subprocesses still need their actual owners in the map.
For architecture/README documentation changes, run
`cd website && uv run python dev.py test -k 'portable_architecture or readme_index_sync'`.
For Markdown changes under `docs/`, builtins or skills, and changes to README or
AGENTS, also run `cargo test -p loopflow --test documented_commands`. Its command
scanner covers prose and headings as well as fenced examples; a focused runtime
suite does not check those additions.
Keep README and docs/index openings identical and regenerate docs/architecture.html
when its source changes. Retired Project surfaces also affect CLI fallback,
builtin discovery, prompt goldens and storage settlement.

For landing changes, prove same-head recovery and authoritative merge separately
from commit creation. Exercise takeover while the old repair is still running;
generation fencing alone does not stop its effects. Label simulated
provider/GitHub tests. Known live-check limits belong in Infrastructure memory.

Landing changes also affect release finalization and Linear completion recovery:

```bash
uv run python scripts/materialize_rust_tests.py -- cargo nextest run -p loopflow -E 'binary(release_tests) | test(ops::pm::task_planning_tests::task_completion) | test(ops::pr_landing::)' --no-fail-fast
```

Detached repair proofs enter through the compiled CLI so admission records its
Process before launching a child. Assert the published tag and completed repair;
an in-process `release_run` call does not exercise that execution boundary.
Include stale failure observations after repair completion: a fresh authoritative
merge must still settle the release and publish its tag, even if repair admission
has already blocked the completed incident. Use fixture synchronization to prove
the ordering without timing-dependent sleeps.

Run CLI-backed Python tests only after the Rust build finishes; replacing their
binary mid-test mixes migration frontiers in a single temporary Machine.

Fresh-store coverage exercises the live SQLite schema. Populated historical
fixtures exercise the migration chain and verify retained facts. A fresh-store
pass alone does not prove that an existing Machine can upgrade without losing work.

Prove explicit experimental Machine continuity with `global_commands` and
`one_machine_tests`. Use a fresh `LF_HOME` for each schema version: source binaries
without an explicit experiment forward to the installed CLI and main Machine.
Never run candidate mutation checks against the main Machine.

Run the real CLI resume regressions with isolated installation authority:

```bash
uv run python scripts/test_task_installation.py
# One changed managed operation proof:
uv run python scripts/test_task_installation.py --test task_operation_starts_with_durable_history_after_claim_only_failure
```

Pass several names after `--test` to share one disposable build across related proofs.
The default container tracks stable Rust, matching the other CI Rust jobs, and
logs its compiler version. When adopting a newer standard-library API or Clippy
fix, verify the installation harness uses the same toolchain policy; a host lint
pass does not verify the container build. Use `--image` to reproduce an older
toolchain explicitly.

CLI owner-tree changes must include `cargo test -p loopflow --lib engine::flow_graph::tests`
to verify builtin operation labels, plus the affected proofs above. The regular
Rust suite skips those installation proofs; a skipped case is not verification.
Task status reads a Flow from recorded command outcomes and OS liveness. Run
`cargo test -p loopflow --lib ops::task_execution::tests` for running, between
steps, stopped and failed.
Managed Task fixtures must bind the checkout's Team and Initiative before
creating Task worktrees; reuse `support::bind_task_planning` for the shared fixture.

When changing Task planning lookup or provider response shapes, run the affected
installation proofs and the paired `local_planning` deletion proof. The portable `task_abandonment`
fixture retains unresolved Sessions, Processes, published PRs and files in both
connection modes. The regular Rust suite skips installation proofs. Keep
simulated provider revisions and checkout Team/Initiative bindings consistent
with the planning records those workflows resolve. Exercise unfinished work
before confirmed removal; do not resurrect deleted Tasks by resetting only
execution tables while retaining planning tombstones.

This copies source into a disposable Linux container, materializes its draft
migrations and builds the development CLI with two Cargo jobs. It checks
populated planning upgrades from the released schema. Intermediate branch
schemas follow the current-state cutover policy and are not imported. Installation-copy succession is no longer a
product contract; its two cross-store promotion/continuation cases were removed.

The adoption case starts with planning and no Task row. Public checkout/run
reuse a dirty existing Git worktree or fetch an open PR's unseen remote branch,
retain the PR identity, and leave an earlier Flow's Processes untouched after source
changes. Linear planning and GitHub reads are fixtures; Git and CLI paths are real.

The disposable OS account authors fixture installation records for routing proofs;
Task adoption uses an explicit experimental Machine. No host Machine, credentials or installation
is mounted. Default executable routing uses two real source CLI processes and
a simulated installed executable. It runs in this disposable account because
`HOME` and `LF_HOME` cannot isolate installation records. The ordinary
`pr_tests` suite covers Task continuation's auto-merge revocation and review
continuity in an explicit experimental Machine.

The separate planning CLI proof (`cargo test -p loopflow --test planning_lookup_tests`) runs planning-only `task status` by identifier and UUID against
normalized local planning, proving that inspection creates no execution or worktree.
That CLI case exercises cached planning; `ops::pm::planning_lookup_tests` covers
acquisition with simulated Linear responses, including missing Projects, partial
responses, absence, and provider failure. Neither is configured live-provider proof.

The mechanical Flow proof uses that disposable account without an installation
selection. A claim followed by admission failure/release leaves Started absent;
the real worker records operation history and Started together. It retains the
captured Flow after the template disappears and records an ordinary child Process without creating an AgentSession.
The default-Machine proof sends two nested source CLI processes through a simulated
installed executable and checks that both select the main Machine despite stale
control pins. The explicit-Machine Flow proof runs locally in `one_machine_tests`. The declaration proof starts Task Y
from Task X's agent and checks Y attribution while retaining X as the causal parent.

The harness keeps Docker build/registry caches, serializes use of its build
cache, and destroys the account and installation after each attempt.

Session history has focused storage, harness, reducer, and reader checks:

```bash
cargo test -p loopflow session_record
cargo test -p loopflow journal
cargo test -p loopflow store
cargo test -p loopflow harness::conformance_tests
```

When changing harness event mapping, run the recorded-trace conformance tests
alongside the provider's unit tests. Keep trace expectations aligned with the
event contract, including durable final-answer receipts and usage checkpoints.

After Session-history or schema changes, run `lf history list --json`, `lf usage --json`, and
`lf doctor --json` against a disposable Machine with inherited `LF_*` and
`LOOPFLOW_*` authority removed and `LF_BIN` pinned to the compiled source CLI.
Never use the installed store to prove a draft migration.

## E2E Tests

Shell-based workflows for CLI and worktree behavior.

```bash
tests/e2e/test_smoke.sh
```

Long-running workflow tests for mechanical `lf` commands:

```bash
tests/e2e/test_full_cycle.sh
tests/e2e/test_sync_safety.sh
```

Exercise repository planning reconnect through real work-watch and Flow commands on Linux:

```bash
cargo test -p loopflow --test planning_reconnect_tests
```

Requires `uv`, Python and OpenSSL. The fixture uses an isolated Machine/store and
local HTTPS proxy with synthetic Linear state and credentials. Its CA is trusted
only by CLI children through `SSL_CERT_FILE`; macOS platform TLS ignores that
setting, so reconnect is Linux-only. `task_abandonment` and `local_planning` cover
portable local decisions and deletion, retry identity and retained execution in
both connection modes without provider access. No installation or live provider is used.

Exercise Linear expiry and rejection through an explicit experimental CLI:

```bash
# Inside a disposable Linux container with a freshly built candidate:
LF_HOME="$(mktemp -d)" uv run tests/e2e/linear_oauth.py --lf target/debug/lf
```

Give the container `--add-host api.linear.app:127.0.0.1`, Python, Git, and
`uv`. The experiment needs a registered repository. Never mount a real Machine
or credentials into this container: the fixture replaces its Linear row
and seeds planning data in the explicit disposable store.

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
CLI to `/fixture/install_bootstrap.py`, `/fixture/install.sh`, and
`/fixture/bin/lf`. Build the CLI in a separate disposable source snapshot
after `canonicalize_migrations.py --materialize-for-tests`, using release
provenance and published migration authority only in that isolated build.
The runtime container needs curl, OpenSSL, CA certificates, Git, useradd,
runuser, and uv; run the check as root without host Machine mounts. Optionally copy
a checksum-verified older released pair into `/fixture/prior` to exercise the
external-installer transition.

The script creates separate OS accounts and a local HTTPS release endpoint.
It exercises the real CLI, verified shell installer, store creation, repeat
installation, missing-CLI repair, checkout preservation, and recovery after
a forced activation failure. Transport is simulated; this does not replace a
public release-channel demo. Discard the container afterward.

### Native Codex terminal capture

```bash
uv run python scripts/test_network.py uv run python tests/e2e/codex_terminal.py \
  --lf target/debug/lf --codex /path/to/native/codex
```

Use a native Codex binary supporting profile files. This headless PTY fixture
uses disposable provider and Loopflow homes, a loopback model endpoint, and an
independently authored wrapper supplying trust plus SessionStart/Stop hooks.
It verifies completed turns, exact native capture, both launch hooks, reconnect,
unchanged configuration and temporary-profile cleanup. It uses no account or
login. Native 0.160.1 resume emits neither fresh hook in this fixture, even
without lf. This proves launch composition, not cmux's actual tab tracking.

### Released capture-history preservation

```bash
uv run python tests/e2e/capture_history.py \
  --released-archive /tmp/lf-aarch64-apple-darwin.tar.gz \
  --candidate target/debug/lf
```

Supply the matching v0.13.3 CLI archive from its published release. The fixture
checks its pinned SHA256 before extraction, creates a temporary Machine with stub
providers, and exercises ordinary public commands with both binaries. It preserves
capture paths/bytes, native identity, usage and released review feedback through
candidate reads, replay, resume, review settlement and nested Process ancestry.
`runs/` remains the one opaque capture root; no migration or installation runs.
Provider and terminal transport are simulated, so this is not installed acceptance.

Pair this with `session_lifecycle_tests`' interruption, nested Task attribution and
review replacement cases and `session_cli_tests`' stale actor/identity cases.
Full affected verification belongs to gate. Do not run installation preflight or
promotion fixtures on a host account merely by overriding HOME/LF_HOME: installation
uses getpwuid. The three tests named in LOO-370's current isolation steer remain
isolated-CI owned until PR #1444's disposable-account runner is integrated.

## Nightly Package Tests

`.github/workflows/nightly-packages.yml` builds the same native `lf` tarballs as the release workflow. Each runner extracts its tarball and runs:

```bash
package-smoke/lf --version
package-smoke/lf --help
package-smoke/lf list
```

Nightly package artifacts are verification only. They are uploaded for 14 days and not deployed.

## Validation Scripts

`scripts/` contains runnable validation and demo scripts. Use these for branch validation and manual UI walkthroughs.

```bash
uv run python scripts/loopflow-dev.py run-debug     # build and launch Loopflow (macOS)
uv run python scripts/check_swift_multiplatform_boundaries.py  # Stage 01 boundary guardrails
```

When adding features that need manual verification, write or extend a script in `scripts/` rather than documenting a list of commands. One command to run, one environment to verify in.

## Boundary-specific checks

Current-state conversion preserves resumable conversations and current planning;
it does not reconstruct finished history from retired owners. Keep these populated
migration proofs when changing the three-owner schema, including in the disposable
release-materialized source tree:

```bash
cargo test -p loopflow --lib ownership_cutover_keeps_current_task_review_without_importing_history
cargo test -p loopflow --lib project_status_adoption_preserves_current_identity_and_custom_flow
```

They check retained review identity/capture and the current Project's custom Flow
default. Synthetic migration success does not authorize conversion of an installed
Machine or prove configured-provider resumption.

When changing how a Flow step is described or read back, include the step
argument and Process inventory tests and the public Session lifecycle proofs. A
Flow is its driver Process and step Processes; assert on those Processes and on the Session
turn a step Process captured.

```bash
cargo test -p loopflow --lib ops::flow_run
cargo test -p loopflow --lib store::sqlite::flow_inventory
cargo nextest run -p loopflow --test session_lifecycle_tests --no-fail-fast
```

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

### Test without an installed Loopflow

Session-command fixtures must work without an installed `lf`. Supply an `LF_BIN`
fixture, restore it afterward, and serialize environment changes with
`test_env_lock`. Reuse `TestLfBinGuard` in Task controller tests and `SessionHome`
in conversation tests; keep Session spawning mocked. Listing waiting Sessions
also resolves the executable for their open command, even with spawning mocked.
Enter `journal::with_runtime` after selecting the fixture Machine so its Process and
the Session driver references share the same database. Simulated finite-provider
harnesses must record their owned child exit; an absent endpoint is not exit
evidence.

Launch proofs use provider-written evidence and terminal receipts. Session
completion gives the asynchronous telemetry recorder only 250 ms to drain;
returning successfully does not guarantee `events.jsonl` is complete. Keep
usage and account-event assertions in the recorder tests, rather than racing
its queue in subprocess-launch tests. Reproduce suspected races with a temporary
recorder delay beyond that drain window; remove the delay before publication.

Orphan Session proofs must launch outside every registered Task checkout; changing
branches in the same directory does not remove checkout membership. Run these
proofs with a canonical `TMPDIR` (on macOS, `TMPDIR=/private/tmp`) so temporary
path symlinks cannot mask that association.

Session association changes also require the full `session_lifecycle_tests` suite:
explicit Task bindings retain membership even from a sibling checkout. Wave label
changes require `DesktopHeadlessTests` alongside `WaveTests` to verify rendered titles.

Fixture Machines must set `LF_HOME` explicitly, including when overriding `HOME`.
Open fixture stores at `$LF_HOME/loopflow.db`; no variable selects another file.

When changing Machine selection, run the affected fixtures with `LF_HOME` unset in
the test runner; the materialization wrapper's shared test Machine can mask missing
fixture setup. CLI fixtures must select their own disposable Machine. Upgrade proofs
must use published migration authority against a temporary shared store; opening
an existing experiment intentionally validates its schema without upgrading it.

Machine command changes also require `cargo test -p loopflow --test global_commands`
to preserve repository-independent commands and explicit Machine isolation.
When changing remote account-selection docs, run `cd website && uv run python
dev.py test -k test_docs_subscriptions_page_owns_account_selection`; the rendered
guide must retain the current invocation, laptop account selection, and target-resident login contract.

For gate runs launched inside managed execution, clear inherited `LF_*` authority and
pin `LF_BIN` to the checkout's compiled `target/debug/lf` before invoking the test
runner. The materialization wrapper clears only its listed variables; it does not
clear every inherited pin. The provider harness prepends the selected CLI's parent
to PATH, so an inherited CLI directory containing `claude` can outrank a fixture's
fake provider and launch the real one. A temporary `LF_HOME` alone does not prevent
this. Keep the failed evidence if this occurs, stop the test group, and verify the
fixture under the corrected executable context before completing the suite.

Default-runtime selection reads the OS account's installation records. Use the
installation harness for default-runtime proofs; never replace the machine's
selection to make tests pass. Flow/Session tests with an explicit experimental
`LF_HOME` and source `LF_BIN` stay within that experiment.

`lf install preflight` and `promote` read the OS account's store and take
its promotion lock; `HOME` and `LF_HOME` do not redirect them. Tests that run
either command are installation proofs: ignored in the regular suite and listed
in `scripts/test_task_installation.py`. On a developer machine they would copy
the live database, without bound while other workers write to it.

For executable-resolution failures, reproduce with the compiled test binary:
unset `LF_BIN` and `CARGO_BIN_EXE_lf`, and use a PATH containing Git but no `lf`.
Verify the repair in that same environment. A pass under a developer's installed
Loopflow can hide the CI failure.
Include direct provider-harness startup tests in this check: even an expected
spawn failure first resolves the conversation's `lf`. Pin a fixture executable
under the environment lock and restore the pin afterward.

Changes to terminal provider probes or spawning must run both `session_cli_tests`
and `agent_startup_tests`. An executable that exits with an error still started
and must record its opening; use an absent or non-executable fixture for spawn
failure, with PATH restricted so no installed provider can take its place.

Release repair checks must cover completion before inherited checkout locks close.
Use the public release path with a delayed repair launcher; a terminal Process receipt
does not prove that its process or descendants released their descriptors.

When a subprocess fixture signals readiness with file contents, write a sibling
temporary file and rename it into place after closing it. File existence alone
can expose an empty file between creation and the first write.

### Project readiness fixtures

Task fixtures must create their Wave and Project, select the Project in SQLite,
and retain Machine placement before creating work. YAML binding imports belong to
explicit activation tests; passive status reads do not import them. Keep readiness
fields in inline Swift responses as well as shared DTO fixtures.

Changes to Project selection or planning admission require the full materialized
Rust suite and headless Swift tests, including Task consumers and status readers.
Run the installation harness for migration changes; its released-source proof
must retain Wave placement before projecting accepted Projects.

### Shared identity fixtures

When changing Session activity or Waiting, run `session_cli_tests` and
`work_watch` together. Direct SQL fixtures must carry both driver and provider
generations, matching the production activity writer. Rebuild both SwiftPM and
the Xcode test targets after shared model renames; Foundation types such as
`Foundation.Process` need explicit qualification where names overlap.

Exercise Session fixtures through Rust as well as Swift after ancestry changes.
`wave_id` uses WaveId's UUID encoding; prefixed Task/Project Work IDs are different
types. A Swift String round trip alone cannot prove that a Rust producer accepts
an identity value.

Session fixtures share `AMBIENT_TASK_ENV` in
`rust/loopflow/tests/support/ambient.rs`. Its guard clears inherited control Machine,
captured-input identity/directory, Task authority, and review Session identity under the
suite's environment lock and restores them afterward. Prove isolation from a
live Session without shell-level scrubbing:

```bash
cargo test -p loopflow --test session_cli_tests session_names_are_shared_and_human_names_win
```
