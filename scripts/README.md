# Scripts

Verification, installation and benchmark entry points.

## Quick reference

```bash
lf sync                                                # refresh main and integrate it into this worktree
uv run python scripts/install.py local                 # build only under local-bin/
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json   # run a disposable experiment
uv run pytest python/tests/test_gate_bounded.py        # one Python behavior
uv run python scripts/resource_envelope.py             # attribute local disk pressure
uv run python scripts/check_architecture.py            # architecture owners and vocabulary
uv run python scripts/test.py --list                   # affected-suite plan
uv run python scripts/test.py --reuse-passing          # affected suites once per exact tree
uv run pytest python/tests/test_lifecycle_scorecard.py # scorecard behavior
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-check --samples 1  # needs lf-world built beside lf
lf telemetry-daily                                     # maintainer report
```

The opt-in desktop command exercises the native outline and retained Task panes.
Use it for relevant UI changes; see [performance measurements](benchmarks/desktop-performance/README.md)
for full sampling, comparison and the explicit capture-versus-presentation boundary.

Escalate from a focused behavior to affected suites when crossing a component
boundary. CI and release own the full matrix. Run `scripts/test.py --all` only
to reproduce a matrix failure or when release guidance requires it.

## Changed-aware runner

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

The Rust phase builds the fixture CLI once and supplies it through
`LOOPFLOW_TEST_LF`, so test processes run no Cargo build of their own.

On a pull request, CI runs nextest with the `pr` profile (`.config/nextest.toml`):
a test that runs past 2 minutes fails on that PR and names itself. The merge
queue and main use `default`, which only reports slow tests. Tests that were
already slower are listed there with their own allowance; make one fast, then
delete its entry. Reproduce locally with `cargo nextest run --profile pr`.

Fixtures still create their own disposable Homes; a test never alters an
installed database.

Runner changes also need the command-plan checks in
`python/tests/test_release_automation.py` during focused verification.

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
free disk. `scripts/performance-budgets.json` sets a 64 GiB cleanup target, a 32 GiB
emergency disk reserve, and four
low-priority verification workers. Total build size is measured, not capped.
Individual root thresholds are cleanup signals, not gates: an oversized sibling names its
worktree in a warning and does not block a healthy checkout. The 24 GiB build threshold
identifies cleanup candidates, not a limit on what a worktree may build.

```sh
lf wt prune --builds --dry-run       # candidates and retained reasons
lf wt prune --builds --json          # reclaim output; retain checkouts
```

Verification preflight and explicit resource `--recover` use the same build-output
owner. It requires a recorded completed local Task, no unfinished association,
idle execution and ignored output whose entire tree has gone unchanged for 24 hours.
It rechecks ownership, execution, cwd, executable mappings and open files under the
worktree lease before deletion. Unknown inspection, busy leases, running binaries,
tracked content, symlink escapes and primary/current/persistent checkouts retain
output. Dirty and untracked source outside generated roots survives.

Eligible output is reclaimed on the next supported invocation regardless of disk
pressure or individual size. Defaults bound each invocation to eight generated roots
and 64 GiB; `--retention-hours`, `--maximum-roots` and `--maximum-bytes` select the
prune policy. Repeated calls drain an eligible backlog. Python inventories capacity
and consumes Rust cleanup receipts; an unavailable capable `lf` retains builds and
reports the missing owner. A timed-out or invalid cleanup receipt reports an unknown
result and triggers fresh inventory, since deletion may already have occurred.
Each inventory `du` has a 15-second deadline;
failures appear as measurement issues. Total inventory time still scales with the
number of roots. Inventory timeouts grant no deletion authority; Rust independently
retains candidates whose ownership, activity or content inspection is unknown.

The lease coordinates Loopflow operations. Concurrent direct Cargo/npm/Swift
build or clean commands newly racing cleanup in a completed candidate are unsupported.
Cargo can replace a held profile lock during clean; native locks do not provide
universal exclusion. Detected activity always retains output.

Recovery also prunes old disposable gate output and entries accepted by
`uv cache prune`. Cleanup runs before verification, not on an independent timer.
A busy uv cache is reported immediately
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

Read the same gate evidence through the daily operator graph:

```bash
lf telemetry-daily
```

The scorecard joins recorded Session usage and duration with Task PR landing and pre-land phase records. It
prints aggregate values and coverage only—never commands, prompts, output, or
task ids. Missing evidence is `UNKNOWN`, reported zero remains measured, and
small samples stay `COLLECTING` until 20 observations support p95.

“Recorded agent attempt → merge” includes earlier and unfinished captured inputs attributed
to the exact PR. It is a lower bound on implementation time: unrecorded or
standalone work is outside that cohort. Compare equivalent cohorts before
attributing changes to an optimization. `scripts/performance-budgets.json`
owns thresholds; complete samples without a threshold are `UNBUDGETED`.

Path → suite mapping:

| Changed | Suite | Runs |
|---------|-------|------|
| any changed path | architecture | `uv run python scripts/check_architecture.py` |
| `rust/`, `Cargo.toml/lock` | rust | `cargo fmt`, `cargo clippy --all-targets`, then `cargo nextest run --all` (falls back to `cargo test --all`) |
| `python/`, `scripts/*.py`, top-level `*.py`, `pyproject.toml` | python | `uv run pytest python/tests/` (scoped to changed `test_*.py` when no source moved) |
| `website/`, `docs/` | website | `cd website && uv run python dev.py test` |
| `swift/` | swift | `scripts/test_desktop.sh -Xswiftc -gnone`, then the multiplatform boundary check |
| `swift/LoopflowMac/`, `swift/project.yml` | loopflow *(slow)* | xcodegen + xcodebuild |
| local store/worktree code, `tests/e2e/` | e2e *(slow)* | CLI smoke |

## External network isolation

`tests/e2e/opencode_terminal.py` probes installed OpenCode through the real lf
interactive launch with a private home and local API. Prepare a disposable npm
prefix with `@opencode-ai/plugin` matching `opencode --version`, then pass that
prefix as `--dependencies` alongside `--lf`, `--native`, and `--output`. Run through
`scripts/test_network.py` with `uv run --no-sync`. It retains raw terminal bytes,
screen text, requests and diagnostic logs; checks repeated model-access failures,
draft editing across resize, recovery, typed submission and a fresh successful
launch. `--expect-overlay` reproduces the old stderr corruption against the
installed lf release. The
`session_launch_contract::opencode_` fixtures also cover early failure, arbitrary
stderr bytes, subprocess diagnostics and resume identity without a provider install.

```bash
uv sync
uv run --no-sync python scripts/test_network.py uv run --no-sync pytest python/tests/
```

Prepare dependencies and compile binaries before isolating test execution.
The test runner and CI use `test_network.py` for Python, Rust test executables,
Swift tests, website tests and smoke tests. Each invocation proves loopback works
and an external connection is denied; the OS boundary covers descendants too.
Git transport is file-only. Local bare remotes and loopback protocol fixtures
remain available. macOS cannot nest Seatbelt profiles: the checkout-observation
proof skips inside this wrapper and runs separately in gate/macOS CI, installing
its own read-only, network-denying subprocess policy. No developer shell or global Git configuration changes.

macOS uses `sandbox-exec`; Linux uses passwordless sudo, `unshare --net` and `ip`
to create a private network namespace, enable loopback and restore the caller's
user identity. macOS permits only the fixed `/bin/ps` system reader outside the
sandbox, matching Desktop's existing headless profile; it cannot launch fixture
children or open service connections. Unsupported isolation fails explicitly. Installation-container
proofs disconnect their external Docker network after compiling, then run the
same denial/loopback probes before executing tests. Display diagnostics remain opt-in.

## Installation harness

`uv run python scripts/test_task_installation.py --repository-upgrade`
installs the checksum-verified 0.14.3 CLI, creates a Task checkout and Session,
then promotes the candidate against that populated store. It checks automatic
Git locator enrollment, retained identity, historical paths, unpublished edits
and promotion retry without external network access during the proof. Native
provider resume and macOS Desktop installation require separate acceptance.

`uv run python scripts/test_task_installation.py --test running_provider_uses_updated_install_for_commands_hooks_and_graph`
starts a fixture provider, observes a controlled command failure, switches the
installed CLI, and retries the same command in the same provider and checkout.
Saved hooks and later Graph children also use the replacement. The proof preserves
Session history, pending steers and caller attribution, and checks stale inherited
`LF_BIN` selections through the real installation gate.
The initial CLI is a failing script and the replacement is the source build;
this does not prove released-version migration or configured native-provider
behavior. The Graph starts after the switch, so it does not prove survival of an
already-running Graph.

The disposable installation job caches its Cargo target and registry on
the Linux runner, keyed by image identity, architecture, lockfile, profiles and
harness. Successful PR runs can seed this cache within their GitHub cache scope;
the first run is cold. Source, OS users, Homes and installations are recreated
for every proof. Both managed and published candidates still pass through Cargo
and execute their checks after restoration. Locally, omit `--cache-dir` to use
Docker volumes; Mac-host bind mounts have caused unnecessary Cargo rebuilds.

## Python tests

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

## Third-party inventory

After changing `Cargo.lock`, including a release version bump, run
`uv run python scripts/third_party.py refresh` and
`uv run pytest python/tests/test_third_party.py`. Release preparation refreshes
the inventory after bumping versions. Gate must check the resulting release
tree: stale inventory fails both Python tests and Desktop packaging.

## Validation scripts

`scripts/` contains runnable validation and demo scripts. Use these for branch validation and manual UI walkthroughs.

```bash
uv run python scripts/loopflow-dev.py run-debug     # build and launch Loopflow (macOS)
uv run python scripts/check_swift_multiplatform_boundaries.py  # Stage 01 boundary guardrails
```

When adding features that need manual verification, write or extend a script in `scripts/` rather than documenting a list of commands. One command to run, one environment to verify in.
