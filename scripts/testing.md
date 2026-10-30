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
`binary(name)` matches only the standalone `[[test]]` binaries in
`rust/loopflow/Cargo.toml`; suites inside `integration` take `test(name::)`, and
an unknown binary name fails the whole filterset before anything runs.
`--reuse-passing` returns at once for an identical tree and plan. A later pass
narrows `--rust-filter` to what changed since the last pass.
A filter reaching `ops::linear::` subtracts its four-to-five-minute fixtures
(`- test(native_runner_is_quiet_until_its_turn_ends) - test(linear_on_demand_native_hooks_share_freshness_across_processes) - test(linear_on_demand_shares_checks_and_excludes_finished_tasks)`)
unless the diff reaches them; together they exceed the 1200 s Rust budget, and
CI's partitions run them.
Omit `--rust-filter` (the whole workspace) only to reproduce a CI
`rust-test` failure; CI's eight partitions own the full Rust matrix.

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
| CI workflows, caches, Dependabot | `.github/README.md` |
| `tests/e2e/` | `tests/e2e/README.md` |
