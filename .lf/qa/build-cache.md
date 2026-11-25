Read when the change touches `scripts/build_cache.py`, its post-checkout hook or target seeding.

# Build cache and seeded targets

The hook runs the copy at `$(git rev-parse --git-common-dir)/hooks/loopflow-build-cache.py`,
refreshed only by `uv run python scripts/build_cache.py install`. `diff -q` it against the
branch's script first; a stale copy tests yesterday's code.

lf makes a Task's checkout the way the real repository does, so drive that sequence by hand:
`git worktree add --quiet --force --detach ../<repo>.qa-ready <base>` (this is when the hook
seeds), then `git worktree move --force` it to the Task path and
`git switch --quiet --no-track -c <branch> <base>` inside it. The switch has a non-null previous
HEAD, so it never re-seeds. A seeded `<repo>.ready` can serve as a source once Cargo builds there.
`target/.loopflow-seed` and `build_cache.py status` (`seed     from …`) name the source.

The Machine's `cargo` is Homebrew's; rustup's `stable` (Cargo 1.100, CI's toolchain) is the
other layout, with no `.fingerprint` or `deps`. Prepend
`~/.rustup/toolchains/stable-aarch64-apple-darwin/bin` to `PATH` to drive it, and run the
seeding tests under both. The fixture builds an external path dependency offline
in both worktrees. Cargo reports it fresh and rebuilds the checkout's application.

The python-test job runs on ext4, where `cp --reflink=always` fails, so the seeding tests
that require a real clone skip there. The source lock and overlapping-build checks
also run on filesystems without reflinks.

`clonefile(2)` keeps file mtimes but gives cloned directories the clone time. The hook copies
the source's `target/debug/.fingerprint` mtime (`build` under Cargo 1.100) and uses it to
rank built siblings. To see the order and which are busy, run
`b._seed_sources(root)` and `b._quiescent(debug)` from Python with `sys.path` pointed at
`scripts/`. A sibling skipped while ranked higher is usually mid-build: `ls -lt` its
`target/debug/deps` shows the link.

A Cargo build in a sibling holds its locks: run `git worktree add` while it builds to see the
seeder skip it. A non-cloning volume: `hdiutil attach -nomount ram://65536`, `newfs_hfs`,
`mount -t hfs` under a `/tmp` directory, add a worktree there; the hook stays silent, no
`target/` appears, and a manual `build_cache.py seed` prints `[Errno 18] Cross-device link`.
`hdiutil detach` afterwards.

In an isolated `HOME` (see Shared build cache below) builds run client-side: no
sccache daemon starts, so there is nothing to stop.

To exercise the busy-source skip on the real repository, start a build in a fresh seed and
`touch` its `target/debug/.fingerprint` so it ranks first; the next `git worktree add` must name
another built source, or leave the target absent if all built sources are busy. Unbuilt seeds
should still have no Cargo lock files under `target/debug`. For the reverse race, hold a
built source's locks from Python
(`with build_cache._quiescent(Path("target/debug"))`, `sys.path` pointed at `scripts/`) and run
`cargo build -v` there: it prints `Blocking waiting for file lock` until the block exits.

## Shared build cache

Drive `scripts/build_cache.py` in an isolated `HOME` and `CARGO_HOME`, keeping
the real `RUSTUP_HOME`. Copy the installed pin's `bin/sccache` to the same
revision path under that `HOME`; `install` then skips `cargo install`. Run the
branch's `.venv/bin/python` directly, since `uv` under a new `HOME` refetches.
A dependency-free two-crate workspace is enough. Give it `[profile.dev]
incremental = false`, like the repository; never read `CARGO_MANIFEST_DIR`
in a crate meant to hit, because that value stays verbatim in the hash. Touch a
source before a build that must start a daemon: a fresh build compiles nothing.
Stop that daemon by the PID `status --json` reports.
