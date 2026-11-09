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

After syncing, rerun affected tests when main changed APIs they call, even if
Git merged cleanly. A passing check from before the sync does not cover that
combination; renamed enum variants can leave new branch tests uncompilable.

CLI command or flag changes also run `cargo test -p loopflow --lib lf::tests::`.
These parser and routing checks exercise canonical commands directly; navigation
aliases are covered separately by the `cli_discovery::` integration suite.

Creation receipt changes also run `test(local_planning::)`,
`test(context_launch_tests::)`, `test(settings_launch_tests::)` and
`binary(session_lifecycle_tests)`. Served-write changes run
`test(lf::commands::write::tests::)`. Provider transition recording reaches
`test(process::runtime::tests::)`, `test(harness::codex_connection::tests::)`,
`test(harness::fact_writer::tests::)` and
`test(store::sqlite::sessions::metadata_tests::)`: recovery owners, turn attribution
and metadata reads over historical detail all depend on it.

Terminal-title changes run `test(transport::output::tests::)` and
`binary(transport_tests)`. Cover attachment before the asynchronous Session-name
lookup completes in a deterministic output test; a live attachment can miss that
startup race locally.

Changes to `scripts/resource_envelope.py` or its imports also include
`test(build_cleanup::)` in the gate's Rust filter. That suite invokes Python
recovery outside pytest's configured import path. Use the gate's prepared CLI
so compilation cannot outlast its process-liveness fixtures.

## Implement: one focused test

```bash
cargo test -p loopflow --lib skills::catalog::          # one library module
cargo test -p loopflow --test integration context_tests:: # one integration module
cargo test -p loopflow --test golden_prompt              # one standalone binary
uv run pytest python/tests/test_gate_bounded.py          # one Python file
```

Never `cargo test`, `cargo test --all` or `cargo nextest run --all` here.

## Gate: affected suites once per tree

```bash
uv run python scripts/test.py --reuse-passing --rust-filter 'test(skills::catalog::) | test(prompt::) | binary(golden_prompt)'
```

The runner runs architecture, Python, website and Swift suites when their
paths changed, then Rust: inventory, `cargo fmt --check`, the filtered nextest
run with one prepared fixture CLI, then `cargo clippy --all-targets -- -D warnings`.
On macOS it builds the CLI before isolated Python checkout-refresh proofs, so
native archives and Cargo dependencies are fetched before networking is disabled.
`binary(name)` matches the standalone `[[test]]` binaries in
`rust/loopflow/Cargo.toml` and the CLI's own unit tests as `binary(lf)`;
suites inside `integration` take `test(name::)`, and an unknown binary name
fails the whole filterset before anything runs. A change to what `src/bin/lf.rs`
calls (store openers, work binding) reaches `binary(lf)`: `--lib` never runs it.
A filtered run covers `-p loopflow` only. Substring filters such as
`test(ship)` or `test(planning::)` also reach slow suites (`land_tests`,
`local_planning`) and can exhaust the Rust phase's 2400 s budget; name modules.
Changes to `swift/project.yml`, `scripts/loopflow-dev.py`, or the libghostty-vt
pin/build reach both Desktop build paths: run `--swift --loopflow`. Desktop
talks to the CLI server; GhosttyKit supplies its renderer. Xcode also packages
the selected helper. The runner's network sandbox blocks SwiftPM fetches; if the Swift suite fails
with `transport 'https' not allowed`, run `swift package resolve` in `swift/` once
and rerun.
`--reuse-passing` returns at once for an identical tree and plan. A later pass
narrows `--rust-filter` to what changed since the last pass.
Website checks generate command reference pages from `LF_BIN` or `lf` on PATH.
After building the candidate, prepend its directory to `PATH` for the gate or
website command when the installed CLI differs: `PATH="$PWD/target/debug:$PATH"`.
The network boundary clears inherited `LF_*`, including `LF_BIN`; PATH keeps
the docs check on the candidate's command tree inside that boundary.
Before gate, regenerate the portable architecture page after edits to
`docs/dev/index.md` or its rendering, following [website/README.md](../website/README.md#testing).
Include `docs/dev/index.html` in the change; the website suite checks it for drift.
A filter reaching `ops::linear::` subtracts its four-to-five-minute fixtures
(`- test(native_runner_is_quiet_until_its_turn_ends) - test(linear_on_demand_native_hooks_share_freshness_across_processes) - test(linear_on_demand_shares_checks_and_excludes_finished_tasks)`)
unless the diff reaches them; together they exceed the 2400 s Rust budget, and
CI's partitions run them. In filtersets `-` binds tighter than `|`, so wrap
the union first: `(test(a::) | test(b::)) - test(slow)`; without the
parentheses the subtraction applies only to the last term.
A filter matching only `#[ignore]` tests (the loopback-SSH `ssh_shells::`
suite) runs nothing and fails with `no tests to run`; run those by name with
`cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored`, then
clippy.
Omit `--rust-filter` (the whole workspace) only to reproduce a CI
`rust-test` failure; CI's eight partitions own the full Rust matrix.

## macOS traps

- `served_reads` compares paths, and macOS temp directories are symlinks: run it
  with a resolved `TMPDIR` (`/private/var/...`).
- File-watch tests (`TaskFiles`, `remote_file_observer_…`) need working FSEvents.
  If they fail, first check whether `fseventsd` is pinned at 100% CPU.

- The runner runs tests from the checkout with this Machine's PATH. Account
  routing fixtures then read repository-scoped routes ("has no resident login"),
  and Graph fixtures see every installed harness ("No agent selected"). Rerun
  such failures from the built binary in an empty directory with a temp `HOME`
  and a PATH holding only `uv`, the Rust toolchain and `/usr/bin:/bin`.
  The test constructor supplies its private home with a fixture Git identity;
  repository-local identities still win. Do not inject `GIT_CONFIG_GLOBAL` to
  repair a test: it hides missing setup in that private home.
- Changes to the test environment constructor reach the library suites too.
  Run `store::tests::` and `transport::viewer_tests::` from an empty cwd: they
  exercise claims for absent repositories and children with cleared environments.
  Such children set both `HOME` and `PATH` before the constructor adds its
  Keychain stand-in; an absent PATH would otherwise contain only the stand-in.

## Builds

- Cargo serializes builds per target directory, so a second concurrent `cargo`
  command only waits on the lock. Never build `--release` to test.
- Machines share compiled crates across worktrees through sccache, wired in
  `~/.cargo/config.toml` (`[build] rustc-wrapper = "sccache"`). The repository
  sets no wrapper because CI has no sccache.

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
