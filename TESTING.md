# Testing

Publish checkpoints with working notes in `scratch/`; hosted CI defers the full
matrix until scratch is clear. `lf pr submit`, `lf pr arm`, and `lf pr land`
clear scratch before pushing a landing candidate. Scratch-free PRs, including
small changes and Dependabot updates, run the full proof matrix in parallel.
Local work should run the smallest proof that can change the next decision.

For checkpoint PRs, `scratch-clear` reports deferred proof and `tests-result`
and the matrix jobs skip. That is not a passing test result. The required merge
queue always rejects scratch artifacts and runs full proof before merging.
Missing scratch, classifier failures, and unexpected skipped candidate jobs
still fail the aggregate. Restoring scratch on a later PR head defers its proof.

A new PR update cancels the previous CI run for that PR. Main and merge-group
runs remain independent.

CI Rust cache keys include the root workspace's build profiles, which the cache
action's member-manifest discovery omits. Profile changes get fresh dependency
caches; version-only releases retain them. Main publishes the shared caches;
PRs and merge groups restore them without accumulating private copies. Cache
restoration alone never skips proof. After a merge, main can reuse a
successful merge-group CI run for the identical SHA and workflow, linking that
run in its summary. Missing or unreadable proof runs the full matrix. Main still
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

## Quick Reference

```bash
uv run pytest python/tests/test_gate_bounded.py        # one Python behavior
uv run python scripts/resource_envelope.py             # attribute local disk pressure
uv run python scripts/check_architecture.py            # architecture owners and vocabulary
uv run python scripts/test.py --list                   # affected-suite plan
uv run python scripts/test.py --reuse-passing          # affected suites once per exact tree
uv run pytest python/tests/test_lifecycle_scorecard.py # scorecard behavior
uv run python scripts/desktop_performance.py run --output /tmp/desktop-check --samples 1
lf telemetry-daily                                     # maintainer report
```

The opt-in desktop command exercises the native outline and retained Task panes.
Use it for relevant UI changes; see [performance measurements](performance/README.md)
for full sampling, comparison and the explicit capture-versus-presentation boundary.

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
worktrees, source, worktree metadata, gate receipts, Run bundles, and SQLite
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
a new proof receipt. Changed content or commands require fresh verification and
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

The scorecard joins current Run usage and duration with Task PR landing and pre-land phase records. It
prints aggregate values and coverage only—never commands, prompts, output, or
task ids. Missing evidence is `UNKNOWN`, reported zero remains measured, and
small samples stay `COLLECTING` until 20 observations support p95.

“Recorded agent attempt → merge” includes earlier and unfinished Runs attributed
to the exact PR. See [metric definitions and coverage limits](performance/README.md).

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
| `swift/` | swift | `swift test --package-path swift --no-parallel -Xswiftc -gnone`, then the multiplatform boundary check |
| `swift/LoopflowMac/`, `swift/project.yml` | loopflow *(slow)* | xcodegen + xcodebuild |
| local store/worktree code, `tests/e2e/` | e2e *(slow)* | CLI smoke |

## Python Tests

Tests for release automation, installers, and repository scripts.

```bash
uv run pytest python/tests/                          # All Python tests
uv run pytest python/tests/test_install_script.py -v # One file
```

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
cargo build -p loopflow --bin lf # Required by the real CLI transport proof
swift test --package-path swift --no-parallel # All Swift tests
swift test --package-path swift --filter CatalogTests  # Catalog DTO / used-by coverage
swift test --package-path swift --filter SomeTestClass  # Filtered
```

Pass `--no-parallel` explicitly for the full suite. Native proofs share AppKit's
main actor; concurrent suites can starve async observations and distort timing
budgets. Swift Testing otherwise runs suites concurrently.

`scripts/prove_wave_surface_states.sh` separately renders four fixture states at
two widths using the built app. It runs the two widths in separate processes,
waits for both, then advances to the next state. All eight images must be nonempty
and pairwise distinct. Interrupting the script stops and reaps its capture children.

Failed Swift CI runs retain a `swift-tests-<run>-<attempt>` artifact for seven
days with the toolchain, output log, and Swift Testing event stream. When console
output stops after compilation, inspect `events.jsonl` for the last started test
without a matching end event. SwiftPM buffers console output, so silence alone
does not establish that test execution never began.

The Swift transport suite launches `target/debug/lf` against a temporary Home.
Build the current CLI before running it; an existing developer build can hide
a missing prerequisite in a clean checkout. CI and `scripts/test.py --swift`
include this build.

Transport tests register `Process.terminationHandler` before launch and await
its notification. Do not use `waitUntilExit()` after an async suspension: the
test can resume on a different thread and hang in Foundation's run-loop wait
even after the child exits. When a Swift run stops reporting progress, sample
the test helper process before changing timeouts; cleanup can be the blocker.

In asynchronous terminal proofs, observe the surface after each wake-up before
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
whole-file platform gate. When changing terminal code or its tests, gate both
configurations: run the focused SwiftPM tests and
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

**Hosted run** (`ui-host` required gate, permissioned host only). Actually runs
`LoopflowUITests`; needs macOS UI-automation permission. Never runs under
`--all`—absence of the permission is a named failure, not a silent skip.

```bash
uv run python scripts/test.py --ui-host
```

See `release/UI_HOST_GATE.md`.

## What CI Runs

See `.github/workflows/ci.yml`. These proof jobs run in parallel and feed the
aggregate `tests-result` check:

| Job | Runner | Command |
|-----|--------|---------|
| `architecture-check` | ubuntu-latest | map every durable owner, public boundary, provider edge, and named shim; reject stale control vocabulary |
| `scratch-clear` | ubuntu-latest | defer checkpoint PR proof; reject scratch artifacts on queue/main |
| `rust-lint` | ubuntu-latest | `cargo fmt`, `cargo clippy --all-targets -- -D warnings` |
| `rust-test` | ubuntu-latest | `cargo nextest run --all --no-fail-fast` |
| `migration-check` | ubuntu-latest | verify migration namespaces/history |
| `python-test` | ubuntu-latest | `uv run pytest python/tests/` |
| `website-test` | ubuntu-latest | `cd website && uv run python dev.py test` |
| `e2e-smoke` | ubuntu-latest | `tests/e2e/test_smoke.sh` |
| `task-installation` | ubuntu-latest | `uv run python scripts/test_task_installation.py` |
| `swift-test` | macos-15 | package tests, boundary check, Wave-state render proof |
| `loopflow-ui-test` | macos-15 | xcodegen + app/test-runner compile |

Candidate and merge-group jobs must all pass for `tests-result` to pass. Rust
collects every test failure in the run instead of stopping at the first failure.
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

For shared repository discovery or CLI dispatch changes, include the PM and
Wave consumers in the focused proof:

```bash
cargo test -p loopflow --test wave_resolution_tests --test wave_resolution_matrix --test global_commands
```

Preserve fixtures for registered Wave directories without Git metadata. Adding
Git would hide the cached-PM context regression; global-command tests alone do
not cover it.

Work-command dispatch also needs the registration lifecycle and repository
ownership proofs. Empty registrations can be forgotten from their registered
directory without Git metadata; Work operations still enforce repository ownership.

```bash
cargo nextest run -p loopflow --test status_tests --test wave_repository_ownership --no-fail-fast
```

Task decision recovery has a focused public-operation proof:

```bash
cargo test -p loopflow --lib task_decision_public_resume -- --test-threads=1
```

It runs the driver-created Ask through resume, including a later branch-adoption
refusal, retained feedback and direction, and a fresh Run with the saved agent.
PM, provider and process-launch effects are simulated; the public resume core,
account selection, store, prompt and driver execute.

The opt-in `task_decision_live_policy_blocks_two_empty_replacement_passes` test
uses actual Codex judgment and simulated PM. Run it with `--ignored --nocapture`
and explicitly set `LOOPFLOW_LIVE_TASK_LF` to the source `lf` built in this checkout,
`LOOPFLOW_LIVE_CODEX_HOME` to an existing configured account Home, and
`LOOPFLOW_LIVE_TASK_OUTPUT` to a fresh evidence directory. It retains its isolated
Home and Run records on failure. This also tests the provider tool shell's
executable/Home propagation: manually supplying a verdict or rewriting the
prompt's commands does not satisfy the proof. The test first runs a separate provider tool-shell probe:
`command -v lf`, `lf task status TEST-1 --json`, and an allowlisted Home/database
capture must identify the source fixture despite an installed `lf` on PATH.
It then judges the recorded passes, completes the resulting policy Ask with
explicitly synthetic feedback, and checks that the same decision Run finishes.
Keep earlier failed receipts; an automatic failure Ask is not policy acceptance.

Task stall proof uses `run_record::activity::tests`, `task_live_unblock`, and
Swift `TaskFlowProofTests`. The sampler retains PID/start identity and cumulative
CPU for the body and descendants in existing Run events. Five quiet minutes
requires samples no more than 45 seconds apart; the worker samples every 15
seconds. Unit proofs advance a simulated clock; the CLI/desktop proof samples a
real sleeping process with a seeded five-minute history. Neither establishes a
five-minute configured provider stall. Preserve CPU-active silence, fresh-event,
missing-sample and PID-reuse counterexamples when changing the projection.
Repeated samples of a reused body PID must remain Unknown; they cannot replace
the observer's original body identity and later establish a stall for that Run.

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
`test_env_lock`. Reuse `TestLfBinGuard` in Task controller tests and `AskHome`
in Ask-session tests; keep Session spawning mocked. Listing waiting Sessions
also resolves the executable for their open command, even with spawning mocked.
Enter `journal::with_runtime` after selecting the fixture Home so its Exec and
the Session driver references share the same database. Simulated finite-provider
harnesses must record their owned child exit; an absent endpoint is not exit
evidence.

`exec_ownership_tests::interruption_records_the_exec_without_a_fabricated_signal_name`
checks the OS exit, durable Exec outcome and absence of its owned scorecard child.
It retains child output for failures. On Linux, `cc` builds the test-only
`support/hold_group_kill.c` interposer: after delivering the real group kill it
holds the signal hook for 200 ms, exposing normal command return racing cleanup.
The fixture asserts that this scheduling point was reached. This is controlled
ordering evidence, not a claim about how long a hosted signal handler paused.
Other Unix platforms exercise the ordinary interruption path.

Exercise native Flow recovery with real Codex and a local Responses fixture:

```bash
uv run --script tests/e2e/codex_connect.py --codex "$(command -v codex)" \
  --lf target/debug/lf --launch --flow-driver-loss completed \
  --output .lf/tmp/native-flow-completed
```

Use `--flow-driver-loss running` for a surviving turn during public resume,
`--flow-driver-loss both` for explicit retry after both driver and engine die,
`--flow-automatic-retry` for failed then successful turns in one command,
and `--flow-decision-retry missing|replace` for discarded failed-turn navigation.
The managed recovery unit fixture exercises the public Flow-to-Task dispatch,
preserved adoption refusal, replacement claim and exact native-history consumption;
its provider history is synthetic, with an owned process supplying exit evidence.
Use
`--flow-retry` for a recorded failure, and `--flow-engine-loss` for replacement
after confirmed engine death. The fixture copies the candidate, uses private
Homes, and stops only its identified engine children. These proofs establish
native execution with synthetic upstream responses, not managed-account or
installed-Home acceptance.

Task-planning fixtures also need an explicit `LF_BIN`: Task status validates
launch authority before reconciling a user merge. Pin the test executable when
no child is launched; an installed `lf` on PATH can hide this missing fixture.
Fixtures selecting a private `LF_HOME` must also clear and restore
`LF_CONTROL_HOME` and `LF_CONTROL_DB_PATH`: the materialized test runner pins
control authority, and Run lookup otherwise reads outside the fixture's Home.
Clearing inherited authority alone does not isolate a process that falls back
to its development Home. Give each proof phase a disposable default `LF_HOME`
and `LF_DB_PATH`; individual fixtures can override those with their own stores.
Reproduce executable-resolution failures with the compiled test
binary, `LF_BIN`, `LF_CONTROL_BIN`, and `CARGO_BIN_EXE_lf` unset, and a PATH
containing Git but no `lf`.

When editing the repeated Task body, exercise every step on two passes and
saved-decision recovery. Keep loop-decide after work and review so navigation
cannot skip a later review.

Include the CLI/desktop unblock projection when changing builtin Flow composition:

```bash
cargo test -p loopflow --test task_initialization_tests task_live_unblock
```

Fixtures targeting a named boundary should resolve its node ID in the expanded
invocation. A hard-coded step index can silently select a different skill when
a nested Flow gains a step.

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

When changing the builtin catalog or Flow composition, also exercise discovery.
Listings keep effective skills and authored flows separate and show their sources
and invocations; `lf flow show` expands a flow's steps.

```bash
cargo test -p loopflow --test cli_discovery list_preserves_kinds_overrides_sources_and_reserved_invocations
```

Catalog retirement also affects historical migration tests. Keep their persisted
names and data-preservation assertions at the migration boundary; current catalog
resolution belongs in engine tests. Include the legacy Flow repair proof:

```bash
cargo test -p loopflow --lib legacy_task_flow_repair
```

Skill export tests use isolated homes and cover builtin/global definitions,
personal agent directories and pruning. Repository `.lf/skills` are local
execution overrides; they are not exported by `sync-skills`.

```bash
cargo test -p loopflow --lib engine::skills::tests
```

Prompt parity and golden prompt tests live in Rust.

```bash
cargo test -p loopflow golden_prompt
uv run python tests/goldens/update_goldens.py   # refresh prompt goldens after prompt changes
```

Changes to builtin `LOOPFLOW.md` affect every prompt golden. Regenerate and
review them before gate, and run `cargo test -p loopflow --lib skill_launch_seed`
to cover interactive skill launches. Keep prose contracts in builtin tests;
launch tests should prove that the canonical document is included.
For migration regressions, use the materialized Rust
test path above: inspect historical fields at their migration boundary, then
finish the upgrade and verify the current schema. When chapter triggers change,
include Task controller consumers: durable work reservation retains Started
after failure, while a mechanical worker claim alone leaves it unset. Use CI's materialized migration graph for trigger
changes; an ordinary draft build may omit the trigger. Installed development
builds record draft checksums too: add a forward draft after the owning migration
instead of rewriting an applied draft. Preserve populated historical fixtures.

For manual migration proof in a shared checkout, materialize only in a disposable
source copy that includes the current tracked and untracked inputs. Materialization
can change package versions, the lockfile, registry and migration files; another
Run can commit those temporary changes before cleanup. Keep the assigned checkout
on its authoring schema and leave the live Home untouched. A copy without Git
metadata cannot prove fixtures that require `git rev-parse HEAD`: run those in
the assigned checkout when its schema suffices, and report that separate proof.
Do not count a fixture setup failure as a passing materialized test.

After removing a public concept, run `uv run python scripts/check_architecture.py`;
retained tables and subprocesses still need their actual owners in the map.
For architecture/README documentation changes, run
`cd website && uv run python dev.py test -k 'portable_architecture or readme_index_sync'`.
Keep README and docs/index openings identical and regenerate docs/architecture.html
when its source changes. Retired Project surfaces also affect CLI fallback,
builtin discovery, prompt goldens and storage settlement.

For landing changes, prove same-head recovery and authoritative merge separately
from commit creation. Exercise takeover while the old repair is still running;
generation fencing alone does not stop its effects. Label simulated
provider/GitHub proofs. Known live-proof limits belong in Infrastructure memory.

Run CLI-backed Python tests only after the Rust build finishes; replacing their
binary mid-test mixes migration frontiers in a single temporary Home.

Fresh-store coverage exercises the live SQLite schema. Populated historical
fixtures exercise the migration chain and verify retained facts. A fresh-store
pass alone does not prove that an existing Home can upgrade without losing work.

Prove branch data isolation against the real installed CLI after building `lf`:

```bash
uv run python scripts/verify_branch_data.py --output scratch/branch-data-proof.json
```

This opts into seeding the source-specific data directory from the current
installation.
It writes a synthetic remote Home observation only in that copy, proves that a
repeat invocation retains it, and checks that the installed CLI still opens its
store with identical schema and migration receipts. It performs no SSH calls,
Task launch or promotion. The observation remains in the branch database. This is
database-isolation evidence, not the complete Task-worker demo for LOO-321.

`ops::task_destination::tests::managed_operations_move_before_branch_effects`
uses paired disposable stores and a simulated installed subprocess. It proves
operation routing before branch effects, returned installed snapshots, rejection
of branch-only Task identities, and preservation of private writes after installed
failure. Its test-only installation root cannot redirect production installation
authority. It does not launch an actual Task worker or
prove installed Session readiness, tmux, or daemon execution.

Run the real CLI resume regressions with isolated installation authority:

```bash
uv run python scripts/test_task_installation.py
# One changed managed operation proof:
uv run python scripts/test_task_installation.py --test task_operation_starts_with_durable_history_after_claim_only_failure
```

This copies source into a disposable Linux container and creates an OS account
whose installation records select the compiled CLI. An ELF trailer gives
the installed CLI a distinct identity: byte-identical copies are installed too,
regardless of path. No host Home, credentials or installation is mounted.
These installation tests run only through
this harness (`task-installation` in CI); ordinary Rust runs mark them ignored.
They prove Task continuation’s auto-merge revocation and review continuity, agent
selection read from the installed database while branch reads remain private, and direct-open
refusal without changing the owned development database/WAL bytes. Review
completion rejects branch-only feedback and a stale readiness token, resolves
both boundary and Run selectors, records the exact installed feedback once,
and preserves the branch Flow and events. The same review scenario proves agent
persistence and repeated `task run` retaining its invocation, cursor and prepared
review Run. It also rejects a missing replacement Flow before changing the installed
review or committing a restart checkpoint. Managed-operation CLI assertions belong
in this disposable account: overriding `HOME` or `LF_HOME` does not remove the
host account's installation authority. Read-only Flow projections stay in the
ordinary suite.

The mechanical Flow proof uses that disposable account without an installation
selection. A claim followed by admission failure/release leaves Started absent;
the real worker records operation history and Started together. It retains the
captured Flow after the template disappears and creates no operation Run.

The recovery proof adds a draft unknown to the branch, preserves both
databases and independent private writes, recommends the
installed executable/database pair only after its real exact-store preflight,
and refuses
that recommendation after the installed schema changes or executable disappears.
The focused `incompatible_seed_preserves_source_receipts_and_private_writes`
unit proof covers a newer source snapshot, its WAL, and reseeding without replacing
private work. GitHub is
simulated and review Runs are prepared without launching a provider. These are
real current-CLI operation proofs, not older-version compatibility, promotion
or configured worker acceptance.
The harness keeps Docker build/registry caches, serializes use of its build
cache, and destroys the account and installation after each attempt.

The `store::branch_data::tests` private-data subprocess fixture checks ordinary
storage, observation, Run artifacts, and child context with stale control pins,
including a relative custom database and malformed inherited control path.
`global_commands` has focused real-CLI proofs for explicit data-directory
reads/writes and
Task-origin promotion refusal with read-only candidate preflight. These use
disposable stores; they do not prove installed worker routing or a live demo.

After rebasing across a release cut, run the installed-development migration
tests as well as the new migration's tests. Adoption fixtures must include the
draft receipts for every pending release; a fixture pinned to one released
draft stops representing an adoptable Home when another release is appended.

```bash
cargo test -p loopflow --lib installed_development_
```

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

Exercise Task deletion through the real CLI on Linux:

```bash
cargo test -p loopflow --test task_deletion_tests
```

Requires `uv`, Python and OpenSSL. The test creates an isolated Home/store and a
local HTTPS proxy with synthetic Linear state and credentials. Its CA is trusted
only by CLI children through `SSL_CERT_FILE`; macOS platform TLS ignores that
setting, so this test is Linux-only. It verifies native removal, local retirement,
completed history, retained PRs/files, retries, planning sync, diagnostics and
rejection of the removed `pm`/`work` groups. No installation or live provider is
used.

Exercise Linear expiry and rejection through an installed development CLI:

```bash
# Inside a disposable Linux container, after candidate promotion:
uv run tests/e2e/linear_oauth.py --lf /root/.local/bin/lf
```

Give the container `--add-host api.linear.app:127.0.0.1`, Python, Git, and
`uv`. Install the published CLI fallback and promote the candidate with
`lf install promote --from-build ...` first. That Home needs a registered
repository. Never mount a real
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
CLI to `/fixture/install_bootstrap.py`, `/fixture/install.sh`, and
`/fixture/bin/lf`. Build the CLI in a separate disposable source snapshot
after `canonicalize_migrations.py --materialize-for-tests`, using release
provenance and published migration authority only in that isolated build.
The runtime container needs curl, OpenSSL, CA certificates, Git, useradd,
runuser, and uv; run the proof as root without host Home mounts. Optionally copy
a checksum-verified older released pair into `/fixture/prior` to exercise the
external-installer transition.

The script creates separate OS accounts and a local HTTPS release endpoint.
It exercises the real CLI, verified shell installer, store creation, repeat
installation, missing-CLI repair, checkout preservation, and recovery after
a forced activation failure. Transport is simulated; this does not replace a
public release-channel demo. Discard the container afterward.

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

When changing Flow step or prepared Run ownership, include the invocation
store tests and the saved-Flow cutover proofs. Human boundaries prepare their
Run before provider launch; fixtures must start that Run instead of binding a
fresh capture.

```bash
cargo test -p loopflow --lib store::sqlite::flows
cargo nextest run -p loopflow --test session_cutover_tests -E 'test(a_task_flow_runs_on_its_row) | test(a_taskless_step_records)'
```

Include `cargo test -p loopflow --test pr_tests` for Task resume changes. Resuming
a human review preserves its invocation and cursor while preparing its Run;
assert those facts instead of equality of the entire versioned Flow record.

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

Tests that construct session commands must supply their own `LF_BIN` fixture,
restore it afterward, and serialize environment changes with `test_env_lock`.
Reuse `TestLfBinGuard` in Task controller tests. Session spawning remains mocked.

For gate runs launched inside a managed Run, clear inherited `LF_*` authority and
pin `LF_BIN` to the checkout's compiled `target/debug/lf` before invoking the test
runner. The materialization wrapper clears only its listed variables; it does not
clear every inherited pin. The provider harness prepends the selected CLI's parent
to PATH, so an inherited CLI directory containing `claude` can outrank a fixture's
fake provider and launch the real one. A temporary `LF_HOME` alone does not prevent
this. Keep the failed evidence if this occurs, stop the test group, and verify the
fixture under the corrected executable context before completing the suite.

For executable-resolution failures, reproduce with the compiled test binary:
unset `LF_BIN` and `CARGO_BIN_EXE_lf`, and use a PATH containing Git but no `lf`.
Verify the repair in that same environment. A pass under a developer's installed
Loopflow can hide the CI failure.

### Shared identity fixtures

Exercise Session fixtures through Rust as well as Swift after ancestry changes.
`wave_id` uses WaveId's UUID encoding; prefixed Task/Project Work IDs are different
types. A Swift String round trip alone cannot prove that a Rust producer accepts
an identity value.

Session fixtures share `AMBIENT_TASK_ENV` in
`rust/loopflow/tests/support/ambient.rs`. Its guard clears inherited control Home,
Run identity/directory, Task authority, and review Session identity under the
suite's environment lock and restores them afterward. Prove isolation from a
live Session without shell-level scrubbing:

```bash
cargo test -p loopflow --lib ops::flow_session::tests -- --test-threads=1
```
