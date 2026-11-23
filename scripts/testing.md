# Loopflow verification commands

Shared by this repository's `implement` and `gate` skills. Every command below
runs from the checkout root with inherited `LF_*` unset and `< /dev/null`.

## Plan first

```bash
uv run python scripts/test.py --list        # changed paths → suites, nothing runs
```

Name the Rust modules and integration binaries the diff can reach before
building anything: the changed module's own tests, its direct callers, and any
boundary check its owner README names (see below).

Provider fixtures that launch a Task conversation beside a Graph must signal
readiness for the process the test exercises. A shared marker can arrive from
the conversation before the Graph's provider starts.

Task Graph selection, cursors or step names also reach
`test(ops::task::explain::tests::)`: previews share the execution selector and
display names. Run these rows before the Task launch fixtures.

Task stacking changes also run
`test(graph_tests::task_checkout_selects_parent_without_rewriting_work_or_publication)`:
the CLI fixture checks that selecting a parent retains checkout bytes and publication.

Markdown changes under `docs/`, builtins or skills, and README or AGENTS edits,
also run `cargo test -p loopflow --test integration documented_commands::`.
It checks command examples and maintained copy, including design documents;
include `test(documented_commands::)` in the gate's Rust filter.

Builtin Graph order or loop changes also include `test(world_recipes::)` in the
gate's Rust filter, alongside `test(graph::)`. These recipes check the steps
recorded by real Task runs, including the pursuit that stops at demo.

Embedded skill Markdown also reaches `binary(golden_prompt)`, even when no Rust
code changes. Follow the [prompt fixture checks](../rust/loopflow/tests/README.md)
to regenerate and review snapshots for intended prompt changes before gate.

After the final sync, publish again when the Rust tree changed, even if Git
merged cleanly. Main can add callers or fixtures for APIs this branch changes.
A passing check from before the sync, local or hosted, does not cover that
combination; new fixtures can omit fields this branch adds.

CLI command or flag changes also run `cargo test -p loopflow --lib lf::tests::`.
These parser and routing checks exercise canonical commands directly; navigation
aliases are covered separately by the `cli_discovery::` integration suite.
Commands with an optional positional Wave or `--wave` also run
`cargo test -p loopflow --lib wave_resolution_matrix::`; register their selection
there so the matrix covers ambient context, explicit overrides and no Wave.
Session command changes also run
`cargo test -p loopflow --test integration session_cli_tests::session_cli_uses_one_truthful_resolution_contract`.

Changes to server protocol types or their doc comments also run
`cargo test -p loopflow --lib server::schema::tests::`. Doc comments appear in
the generated schema; regenerate its checked-in fixture using the command in
[the server README](../rust/loopflow/src/server/README.md) when it changes.

Server startup or background store maintenance changes also run
`binary(served_reads)`, including configuration access with an unreadable store.
Seed corrupt databases before starting SQLite readers or writers; overwriting
only a live database's main file can leave readable pages in its WAL.

Changes to compile-time embedded files also reach native package builds. Internal
workspace crates are private path dependencies; the native package matrix owns
asset verification on all four targets.

Creation receipt changes also run `test(local_planning::)`,
`test(context_launch_tests::)`, `test(settings_launch_tests::)` and
`binary(session_lifecycle_tests)`. Served-write changes run
`test(lf::commands::write::tests::)`. Provider transition recording reaches
`test(process::runtime::tests::)`, `test(harness::codex_connection::tests::)`,
`test(harness::fact_writer::tests::)` and
`test(store::sqlite::sessions::metadata_tests::)`: recovery owners, turn attribution
and metadata reads over historical detail all depend on it.
Session completion or attachment changes also run
`test(session_record::capture_tests::)`: a headless continuation reopens its
Session while retaining the saved provider thread and completes again on exit.

Changes to Task/Graph membership or execution reads also include
`test(performance_tests::)` in the gate's Rust filter. Its planning and status
checks catch per-Task and per-Graph SQL growth beyond focused store tests.

Store-wide schema or trigger changes also run `binary(two_people)`,
`test(transport::)` and `binary(transport_tests)`: shared planning exchanges,
viewer holds and held-terminal lifetimes exercise writers beyond the changed store module.
Task storage changes also run `binary(session_lifecycle_tests)`: creation and
promotion cover absent checkouts and primary Sessions. When moving nullable
columns into separate tables, update fixture reads to handle absent rows and
direct SQL writes and shared fixture lookups to use the new tables. Run
`test(server::service::tests::a_plan_watch_refreshes_and_a_task_view_resolves_its_primary_session)`
for primary Session storage changes: the served watch fixture writes the
association directly before resolving the Task's conversation. Run
`scripts/test_desktop.sh --filter 'TaskRowMetadataTests|WaveStandupTests|DTOFixtureTests'`
when changing Task ownership DTOs; inline Swift fixtures must include required
nullable keys alongside the shared JSON fixtures. Run
`test(performance_tests::)` to check that seeded Graphs still belong to their
Tasks. Include `test(ops::task::tests::)`:
checkout adoption must still find a running Graph and prevent a second run.
Store query changes also run `test(store::query_plans::tests::)`. Missing-key
fixtures must use prefixes outside generated UUID keys, not short hex strings
that can randomly match a captured input.
Fixture database seeding changes include
`test(concurrent_copies_share_one_initialized_machine)` and
`test(two_task_research_runs_leave_distinct_uncommitted_artifacts_for_the_next_prompt)`:
concurrent first opens must retain one Machine identity and every writer's rows.
Include `test(lf::commands::waves::tests::planning_failure_keeps_the_stored_workflow_readable)`
for Task schema or trigger changes: its missing-column fixture checks that a
planning read failure still leaves the Wave and stored Workflow readable.

Configuration file loading or filtering changes also run `binary(two_people)`:
it checks that a shared Wave graph choice reaches another person's Task launch.
Include `test(configuration_changes_reach_an_existing_subscription)`,
`test(graph_defaults_follow_task_wave_and_repository)` and
`test(wave_setup_tests::)` to cover live reload, invalid defaults and Wave scope.

Terminal-title changes run `test(transport::output::tests::)` and
`binary(transport_tests)`. Cover attachment before the asynchronous Session-name
lookup completes in a deterministic output test; a live attachment can miss that
startup race locally.

Viewer readiness changes also run `test(transport::viewer_tests::)` and
`test(session_cli_tests::terminal_titles_follow_session_rename_and_reconnect_without_provider_accounts)`.
A provider may exit before its viewer sends readiness; final output and exit
status must survive that acknowledgement failing.

Changes to `scripts/resource_envelope.py` or its imports also include
`test(build_cleanup::)` in the gate's Rust filter. That suite invokes Python
recovery outside pytest's configured import path. Use the gate's prepared CLI
so compilation cannot outlast its process-liveness fixtures.

## Implement: one focused test

```bash
cargo test -p loopflow-execution --lib skills::catalog::          # one library module
cargo test -p loopflow --test integration context_tests:: # one integration module
cargo test -p loopflow --test golden_prompt              # one standalone binary
uv run pytest python/tests/test_gate_bounded.py          # one Python file
```

Never `cargo test`, `cargo test --all` or `cargo nextest run --all` here.

Promotion and binding decisions (`test(adoption::tests::)`), launch and
resume settings (`test(run::launch_policy_tests::)`), Task run and checkout
previews (`test(ops::task::explain::tests::)`) and Machine dispatch and
ownership (`test(ssh::dispatch_tests::) | test(task_ownership::tests::) | test(store::ownership_lifecycle_tests::)`) run
as library rows
(`cargo nextest run -p loopflow -p loopflow-execution --lib -E '<filter>'`); run those before the
surface's full-Machine tests. The lists are in
[the Rust tests README](../rust/loopflow/tests/README.md#decision-rows-and-full-machine-tests).

## Gate: a few named tests; CI runs the suites

Gate publishes the candidate first ([CI.md](../.github/CI.md)) and CI's
partitions run the suites. "Plan first" above is the map for naming the two to
five tests that matter most. Run them here only for fast feedback on a review
fix, to reproduce a CI failure, or for a risk no suite covers:

```bash
uv run python scripts/test.py --reuse-passing --rust-filter 'test(skills::catalog::) | test(prompt::) | binary(golden_prompt)'
```

The runner runs architecture, Python, website and Swift suites when their
paths changed, then Rust: inventory, `cargo fmt --check`, the filtered nextest
run with one prepared fixture CLI, then `cargo clippy --all-targets -- -D warnings`.
Dependency and bundled-resource changes also require the
[third-party inventory refresh and Python check](README.md#third-party-inventory),
even when no Python paths changed.
On macOS it builds the CLI before isolated Python checkout-refresh proofs, so
native archives and Cargo dependencies are fetched before networking is disabled.
For Python tests on a fresh checkout, run `cargo fetch --locked` before disabling
networking; the provenance fixture compiles a small Cargo workspace offline.
`binary(name)` matches the standalone `[[test]]` binaries in
`rust/loopflow/Cargo.toml` and the CLI's own unit tests as `binary(lf)`;
suites inside `integration` take `test(name::)`, and an unknown binary name
fails the whole filterset before anything runs. A change to what `src/bin/lf.rs`
calls (store openers, work binding) reaches `binary(lf)`: `--lib` never runs it.
A filtered run covers matching tests across the workspace, including extracted
libraries. Substring filters such as
`test(ship)` or `test(planning::)` also reach slow suites (`land_tests`,
`local_planning`) and can exhaust the Rust phase's 1800 s budget; name modules.
Changes to `swift/project.yml`, `scripts/loopflow-dev.py`, or the libghostty-vt
pin/build reach both Desktop build paths: run `--swift --loopflow`. Desktop
talks to the CLI server; GhosttyKit supplies its renderer. Xcode also packages
the selected helper. The runner's network sandbox blocks SwiftPM fetches; if the Swift suite fails
with `transport 'https' not allowed`, run `swift package resolve` in `swift/` once
and rerun.
`--reuse-passing` returns at once for an identical tree and plan. A later pass
narrows `--rust-filter` to what changed since the last pass.

Phone, shared Swift models and server protocol changes also select `ios`.
`uv run python scripts/test_ios.py` runs that suite directly; pass
`-only-testing:LoopflowiOSTests/SessionTerminalTests` for a focused check. It creates
and removes its own Simulator, using an installed iOS runtime and ad-hoc signing.
The suite covers pairing, Task creation and the terminal; the shared protocol
decoders, view stream included, run with the macOS Swift tests. Whether a
Session works is checked on a physical phone against a real Machine.
Website checks generate command reference pages from `LF_BIN` or `lf` on PATH.
After building the candidate, prepend its directory to `PATH` for the gate or
website command when the installed CLI differs: `PATH="$PWD/target/debug:$PATH"`.
The network boundary clears inherited `LF_*`, including `LF_BIN`; PATH keeps
the docs check on the candidate's command tree inside that boundary.
Before gate, regenerate the portable architecture page after edits to
`docs/dev/index.md` or its rendering, following [website/README.md](../website/README.md#testing).
Include `docs/dev/index.html` in the change; the website suite checks it for drift.
A filter reaching `ops::linear::` subtracts its long on-demand fixture
(`- test(linear_on_demand_shares_checks_and_excludes_finished_tasks)`)
unless the diff reaches it; CI's partitions run it. In filtersets `-` binds
tighter than `|`, so wrap the union first: `(test(a::) | test(b::)) - test(slow)`; without the
parentheses the subtraction applies only to the last term.
A filter matching only `#[ignore]` tests (the loopback-SSH `ssh_shells::`
suite) runs nothing and fails with `no tests to run`; run those by name with
`cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored`, then
clippy.
Omit `--rust-filter` (the whole workspace) only to reproduce a CI
`rust-test` failure; CI's eight partitions own the full Rust matrix.

## macOS traps

- The runner stops at sustained `syspolicyd` pressure, which can last as long
  as other checkouts keep compiling. Run its printed Rust plan by hand (build,
  the `nextest` line with the same `-E` filter, clippy), then each suite's
  command, and record that the runner's pressure stop was bypassed. A run by
  hand keeps nextest's limits from `.config/nextest.toml`: a test is killed at
  five minutes and the run at twenty. Don't raise them or wait past them; a
  test that times out only under load goes to CI. In zsh, clear `LF_*`
  inline with `env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /')`; a
  variable holding those flags is passed as one word and clears nothing.
- `served_reads` compares paths, and macOS temp directories are symlinks: run it
  with a resolved `TMPDIR` (`/private/var/...`).
- File-watch tests (`TaskFiles`, `remote_file_observer_…`) need working FSEvents.
  If they fail, first check whether `fseventsd` is pinned at 100% CPU.

- The runner runs tests from the checkout with this Machine's PATH. Account
  routing fixtures then read repository-scoped routes ("has no resident login"),
  and Graph fixtures see every installed harness ("No agent selected"). Rerun
  such failures from the built binary in an empty directory with a temp `HOME`
  and a PATH holding only `uv`, the Rust toolchain and `/usr/bin:/bin`.
  Fixtures that place a Task also read the Machine's `~/.lf/config.yaml`
  unless `LF_HOME` is set (the OS supplies the real home); one rewritten by a
  newer `lf` fails them with "unknown or ineligible setting". Set a temp
  `LF_HOME` too.
  The test constructor supplies its private home with a fixture Git identity;
  repository-local identities still win. Do not inject `GIT_CONFIG_GLOBAL` to
  repair a test: it hides missing setup in that private home.
- Changes to the test environment constructor reach the library suites too.
  Run `store::tests::` and `transport::viewer_tests::` from an empty cwd: they
  exercise claims for absent repositories and children with cleared environments.
  Such children set both `HOME` and `PATH` before the constructor adds its
  Keychain stand-in; an absent PATH would otherwise contain only the stand-in.

## Builds

```bash
cargo build -p loopflow --bin lf    # build this checkout's target/debug/lf
scripts/dev-lf --version           # build and run this checkout's CLI
scripts/dev-lf --isolated-build --version # bypass compilation caches for debugging
```

Run these from the checkout root with the repository's Rust toolchain. Invoke
`scripts/dev-lf` directly; wrapping it in `uv run` changes argument and terminal
handling. A build does not install the executable.

For source debugging or cache diagnosis, `--isolated-build` uses this checkout's
`target/isolated` directory and disables both Rust compiler wrappers and sccache.
It leaves the ordinary target and shared cache in place. Reuse that isolated
target for later debugging builds. This also works when another checkout that
produced cached debug-source paths has been removed.

Storage fixture changes first compile `cargo test -p loopflow-storage --lib --no-run`
without `test-support`, then `cargo test -p loopflow --lib --no-run` with its
dev dependency. Application acquisition and lifecycle tests stay above storage;
prepared SQL invariants and migration checks stay in the storage crate.

Cargo rebuilds changed libraries and their dependents. Command help or rendering
rebuilds `loopflow`; execution and Task shipping edits rebuild `loopflow-execution`
and `loopflow`; saved-record edits rebuild storage and both callers. Unchanged
lower libraries can be reused. Binaries and test executables still compile and
link even when library compilations hit the cache. Use debug builds for local
verification; never build `--release` to test.

Execution fixtures first compile with `cargo test -p loopflow-execution --lib --no-run`
independently of the application. Its dev-only `test-support` feature exposes
shared fixtures and inspection hooks to application tests. CLI parsing, server
dispatch, help and presentation tests remain in `loopflow`.

Application provenance watches the selected branch's reflog and existing refs,
so a commit on a sibling branch leaves a packed application branch fresh.
Without a branch reflog, a packed branch conservatively watches its ref parent
until a commit creates a loose ref; sibling commits can then relink the application.
Storage owns the `origin/main` watch and propagates catalogue changes to callers.

Each worktree keeps its own target directory. Cargo serializes commands sharing
one target directory, so run one Cargo command at a time in each worktree. Do
not point sibling worktrees at a shared mutable target directory.

### Shared compilation cache

```bash
uv run python scripts/build_cache.py install   # once per Machine
uv run python scripts/build_cache.py status    # in any worktree
```

```text
wrapper  ~/.local/share/loopflow/sccache/ca81ec2a…/bin/sccache (pinned)
daemon   pid 4242, /…/ca81ec2a…/bin/sccache, up 3m; configured idle timeout sccache default (600s) (next start)
roots    61 of 61 loaded; this checkout loaded
Rust     258 hits, 1 miss
cache    6.2 GiB of 10 GiB
```

After `install`, plain `cargo build` reuses eligible Rust libraries across
worktrees. `cargo test` keeps incremental state for workspace crates and
shares registry dependencies through the cache. Each worktree owns its
target. A fresh target still compiles and links test executables, binaries,
proc-macros and build scripts. Application tests also build execution with
`test-support` and `loopflow-test-support`. Those libraries embed checkout
paths, so they compile in each root even with incremental off.

`install` builds one pinned sccache revision into
`~/.local/share/loopflow/sccache/<revision>/`, runs it once, and only then
points `~/.cargo/config.toml` at it: `build.rustc-wrapper` plus `SCCACHE_*`
entries under `[env]`. Every other line of that file stays as it was. The
repository sets no wrapper and CI never reads this configuration. Running
`install` again changes nothing; `--revision <sha>` moves the pin. The released
sccache 0.19.0 cannot reuse Rust crates across worktrees, and it prints the same
`--version` as the pin, so `status` identifies the binary by its path.

A new worktree registers before `git worktree add` returns. The installed
`post-checkout` hook carries its own copy of the registration script, so it
also works in older revisions. It lists the root in
`~/.cache/loopflow/sccache.json` (under `XDG_CACHE_HOME` when set) and retains
other repositories' roots. Git can finish the checkout if registration fails.

The pinned daemon reads its roots at startup. When it is serving a client-side
cache and lacks this root, registration asks it to stop. The next compile
starts a daemon with the updated list. Rust compiles in the client process
tree. The daemon gives outstanding connections up to ten seconds to finish,
so a late cache write can be lost. In the two-worktree check, a
`rustls` compile continued across the stop and its Cargo invocation passed the
focused execution test. The [probe](benchmarks/crate-rebuild/README.md#registration-during-a-build)
exercises this through the installed hook.

`scripts/dev-lf` and `status` refresh the inventory. Only `register` asks a
daemon to stop. `status` reads the executable of the process accepting
connections on the socket, the loaded roots and cache counters.
`status --json` returns the same observations. Reading stats resets the idle timer.

| `status` exit | Meaning | Next command |
|---|---|---|
| 0 | The root is loaded, or listed for the next daemon start | Run the focused Cargo command |
| 3 | The serving daemon lacks this root | `uv run python scripts/build_cache.py register` |
| 1 | Check the wrapper, socket owner or reported configuration error | `uv run python scripts/build_cache.py install` |

A wrapper path that no longer exists fails every Rust build on the Machine, in
every repository. Build directly while you repair it:

```bash
RUSTC_WRAPPER= cargo build -p loopflow --bin lf
```

A miss costs compilation time and nothing else. It never calls for cleaning the
target directory or resetting the cache.

The pinned daemon listens on its own socket,
`~/.cache/loopflow/sccache-<revision, first 8>.sock`, and keeps the Machine's existing
cache directory and size cap. A Homebrew sccache daemon that was already running
gets no more requests and exits by itself.

The shared cache reached its 10 GiB cap during LOO-921. Entries can be evicted
before another worktree needs them. Warm measurements in a private cache show
the cost of a surviving entry; they do not measure how often shared entries
survive. Installation leaves the cap unchanged.

**Edit to passing test.** The test profile enables incremental compilation
because it won both three-edit comparisons. Times below are median seconds
across three warm cohorts with two Cargo jobs. Each edit includes a passing
focused test and its link.

| Workflow | Cache mode | Incremental |
|---|---:|---:|
| First application edit in a new worktree | 213 | 210 |
| Next application edit in that worktree | 40 | 9 |
| Three application edits, starting in a new worktree | 285 | 228 |
| First execution edit after the application tests | 90 | 107 |
| Next execution edit in that worktree | 86 | 15 |
| Three execution edits after the application tests | 260 | 131 |
| First execution edit in a new worktree | 272 | Unmeasured |

To use the shared cache for eligible workspace libraries in a focused test:

```bash
CARGO_INCREMENTAL=0 cargo test -p loopflow-execution --lib skills::catalog::
```

The dev profile keeps incremental off. The installer sets
`SCCACHE_ALLOW_INCREMENTAL=1` for the test profile and command-line overrides.
Registry dependencies use separate artifact keys for each mode. The private
warm cache served 300 Rust hits and two misses on each fresh cache-mode
application test. With incremental enabled, it served 296 Rust hits and the
incremental workspace compiles bypassed the cache. Ranges, CPU cost, memory,
compiler invocations and counters are in the [test-loop report](benchmarks/crate-rebuild/results/loo-921.html).

**A private cache.** Set a directory and a socket for one command, and it gets
its own daemon and counters while keeping the pinned binary and the root list:

```bash
SCCACHE_DIR=/tmp/my-cache SCCACHE_SERVER_UDS=/tmp/my-cache.sock cargo build -p loopflow --bin lf
```

Any `SCCACHE_*` variable already set by the caller wins over the Machine's
`[env]` entry. Stop a private daemon when you are done:
`SCCACHE_SERVER_UDS=/tmp/my-cache.sock <wrapper> --stop-server`.

**Source debugging.** A hit can carry the absolute source paths of the checkout
that first compiled the crate, and that checkout may be gone.
`scripts/dev-lf --isolated-build` compiles in `target/isolated` without the
cache; the [crate rebuild probe](benchmarks/crate-rebuild/README.md) records
those paths beside its runtime-identity checks.

Run one Cargo command at a time. How many worktrees can build at once on a
Machine has not been measured; the results give one loop's peak memory and
CPU-seconds, not a budget.

To turn the cache off, delete the `rustc-wrapper` line and the `SCCACHE_*`
entries from `~/.cargo/config.toml`.

## Leave to CI unless the change reaches them

`planning_reconnect_tests` (Linux only), `release_tests`, `session_lifecycle_tests`,
`scripts/test_task_installation.py`, `tests/e2e/`, `--loopflow` (Xcode build),
and `--ui-host`. Run one when the diff reaches its boundary, by name.

## Boundary checks by owner

The checks a change must add live with the code they protect:

| Change touches | Read |
|---|---|
| Rust fixtures, migrations, prompts, Sessions, Linear, Tasks | `rust/loopflow/tests/README.md` |
| runner, network isolation, resources, installation harness | `scripts/README.md` (this directory) |
| Swift, Desktop, Xcode | `swift/README.md` (Testing) |
| website, `docs/` | `website/README.md` (Testing) |
| CI workflows, caches, Dependabot | `.github/CI.md` |
| `tests/e2e/` | `tests/e2e/README.md` |
