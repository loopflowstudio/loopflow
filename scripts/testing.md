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

Markdown changes under `docs/`, builtins or skills, and README or AGENTS edits,
also run `cargo test -p loopflow --test integration documented_commands::`.
It checks command examples and maintained copy, including design documents;
include `test(documented_commands::)` in the gate's Rust filter.

Embedded skill Markdown also reaches `binary(golden_prompt)`, even when no Rust
code changes. Follow the [prompt fixture checks](../rust/loopflow/tests/README.md)
to regenerate and review snapshots for intended prompt changes before gate.

After the final sync, rerun affected tests and `cargo clippy --all-targets -- -D warnings`
when the Rust tree changed, even if Git merged cleanly. Main can add callers or
fixtures for APIs this branch changes. A passing check from before the sync
does not cover that combination; new fixtures can omit fields this branch adds.

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

Changes to compile-time embedded files also run
`cargo package --locked --allow-dirty -p loopflow`. The extracted source archive
must compile without access to files outside the crate. Server examples shared
with Swift live under `rust/loopflow/tests/fixtures/dto/server/` so the archive
contains them.

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

Store-wide schema or trigger changes also run `binary(two_people)` and
`binary(transport_tests)`: shared planning exchanges and detached owner lifetimes
exercise writers beyond the changed store module.

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
Dependency and bundled-resource changes also require the
[third-party inventory refresh and Python check](README.md#third-party-inventory),
even when no Python paths changed.
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
(`- test(native_runner_is_quiet_until_its_turn_ends) - test(linear_on_demand_shares_checks_and_excludes_finished_tasks)`)
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

- The runner stops at sustained `syspolicyd` pressure, which can last as long
  as other checkouts keep compiling. Run its printed Rust plan by hand (build,
  the `nextest` line with the same `-E` filter, clippy), then each suite's
  command, and record that the runner's pressure stop was bypassed. The same
  applies when its resource preflight fails with "cannot measure
  ~/.cache/uv: 15-second inventory deadline exceeded" while disk is ample: a
  cold `du` of a large uv cache outlasts the deadline. In zsh, clear `LF_*`
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
