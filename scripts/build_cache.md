# Builds

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
and `loopflow`; saved-record edits rebuild storage and both callers. A decoder
or transcript reader edit rebuilds `loopflow-harness`, execution and `loopflow`,
never storage, and neither a storage nor an execution edit rebuilds
`loopflow-harness`. Unchanged lower libraries can be reused. Binaries and test executables still compile and
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

## Shared compilation cache

```bash
uv run python scripts/build_cache.py install   # run in each repository that shares the cache
uv run python scripts/build_cache.py status    # in any worktree
```

```text
wrapper  ~/.local/share/loopflow/sccache/ca81ec2a…/bin/sccache (pinned)
daemon   pid 4242, /…/ca81ec2a…/bin/sccache, up 3m; configured idle timeout sccache default (600s) (next start)
roots    61 of 61 loaded; this checkout loaded
seed     from ../loopflow.loo-917-theme-cards at 42d8b45944
Rust     258 hits, 1 miss
cache    6.2 GiB of 10 GiB
```

Plain `cargo build` shares eligible libraries across worktrees. Each worktree
keeps its own target. `cargo test` shares registry dependencies and keeps
incremental state for workspace crates. The application's `test-support`
libraries embed checkout paths and also compile in each root.

**New worktrees start warm.** The installed hook seeds the new worktree from
a sibling's `target/debug`. To see which crates Cargo reuses, run:

```bash
git worktree add --detach ../loopflow.my-change HEAD && cd ../loopflow.my-change
cargo build -v -p loopflow --bin lf
```

Only siblings where Cargo has run can seed a worktree. A freshly seeded
checkout becomes eligible when Cargo creates its lock files. The hook tries
built targets in order of `.fingerprint` directory mtime, newest first.
It holds Cargo's locks while cloning `.fingerprint`, `build` and `deps`;
Cargo 1.100 keeps each unit under `build/<package>/<hash>/`, so a target it
built seeds from `build` alone, ranked by that directory's mtime.
A build starting in the source waits until the clone finishes. The hook skips
siblings whose build records are as new as any tracked file in the checkout.
Otherwise Cargo could treat a sibling's workspace code as fresh. Each worktree
builds its own top-level binaries and keeps its own incremental state. If no
source qualifies or the filesystem lacks copy-on-write clones, Cargo builds cold.

In the [measured builds](benchmarks/crate-rebuild/results/loo-922.html), Cargo
reused registry crates, build scripts and proc macros. Two seeded builds compiled
7 units, all workspace crates. A seed with different dependencies compiled 31.
Cold builds compiled 256. The first `lf` build wrote 1.7 GiB into a seeded
target (2.0 GiB cold). After a focused test about half the target still shared
its original files. Incremental workspace libraries accounted for about 200
CPU-seconds in a fresh test build.

To build a new worktree cold, `rm -rf target` before the first build. A target
that already exists is never replaced. `lf wt prune --builds` counts a seeded
target at full size, so it reports more space freed than the disk gains back.

The installer builds and checks the pin, then writes `build.rustc-wrapper` and
`SCCACHE_*` entries in the personal Cargo configuration (`~/.cargo/config.toml`,
or `~/.cargo/config` when it exists). It keeps other settings,
the existing cache directory and the size cap. Reinstall to update the hook or
pass `--revision <sha>` to move the pin. The verified binary and stock 0.19.0
print the same version. The revision in the installation path and socket name
identifies the pin. Installing registers the checkout like the hook does, so
a daemon already serving other repositories loads it. CI uses its own Cargo
configuration.

Git's `post-checkout` hook seeds a new worktree's target, then registers the
worktree in `~/.cache/loopflow/sccache.json` (under `XDG_CACHE_HOME` when set).
The hook carries its own script so it works in older checkouts. Rerun
`install` to refresh an existing hook. Both steps run silently and let Git
finish checkout on failure. Registration retains other repositories' roots.
Installation keeps an existing shell hook in place when it can insert the
registration command. Other hooks move beside it to `post-checkout.before-loopflow`.
Git runs the saved hook with its original arguments if it is executable.

The daemon reads that list at startup. If the pinned client-side daemon lacks
this root, registration asks it to stop. The next compile starts its replacement.
In the two-worktree probe the stop took 11 seconds. The in-flight compiler
finished, Cargo kept its library and the focused test passed. The cache missed
that library write. In a separate check, two worktrees built an identical edit
to `loopflow-definitions`. The first recorded one miss and the second recorded
one hit and produced a byte-identical library.
[Run the registration probe](benchmarks/crate-rebuild/README.md#registration-during-a-build).

After a skipped hook, run `status` from the affected worktree. It reads the
configured roots, the serving executable, its loaded roots and cache counters.
A stats read resets the idle timer. `status --json` includes both root lists.

| Exit | What status found | Recovery |
|---|---|---|
| 0 | The root is listed and any serving daemon has loaded it | Build normally |
| 3 | The Machine inventory or its pinned client-side daemon lacks the root | `uv run python scripts/build_cache.py register` |
| 3 | Your `SCCACHE_CONF` or `SCCACHE_BASEDIRS` lacks the root | Add the root and reload your daemon if running |
| 3 | A caller-owned or server-side daemon lacks a listed root | Restart it with the updated list |
| 1 | Wrapper, socket owner or configuration error | Check the diagnostic and rerun `install` |

A missing wrapper affects every repository using the personal configuration.
To build while repairing it, run `RUSTC_WRAPPER= cargo build -p loopflow --bin lf`.

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

The installer sets `SCCACHE_ALLOW_INCREMENTAL=1`. Each incremental setting
uses different registry artifact keys. A fresh cache-mode application test in
the warm private cache recorded 300 Rust hits and two misses. With incremental
enabled, it recorded 296 hits, and incremental workspace compiles bypassed the
cache. The shared Machine cache reached its 10 GiB cap, so entries may be
evicted between worktrees. The report gives ranges, CPU, memory and compiler
counters for the warm-cache measurements. Concurrent-worktree capacity is unmeasured.
[Edit-to-test measurements](benchmarks/crate-rebuild/results/loo-921.html).

**Private cache.** Caller-supplied variables override the installer's Cargo
settings. Give a build its own directory and socket, then stop that daemon
using the wrapper path printed by `status`.

```bash
SCCACHE_DIR=/tmp/my-cache SCCACHE_SERVER_UDS=/tmp/my-cache.sock cargo build -p loopflow --bin lf
wrapper=$(uv run python scripts/build_cache.py status --json | jq -r .wrapper)
SCCACHE_SERVER_UDS=/tmp/my-cache.sock "$wrapper" --stop-server
```

**Source debugging.** Cache hits can retain absolute source paths from another
checkout. `scripts/dev-lf --isolated-build` compiles in `target/isolated` with
the cache bypassed. To turn off the Machine cache, remove the installer's
`rustc-wrapper` and `SCCACHE_*` entries from the personal Cargo configuration.
