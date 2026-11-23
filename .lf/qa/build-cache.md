Read when the change touches `scripts/build_cache.py`, its post-checkout hook or target seeding.

# Build cache and seeded targets

The hook runs the copy at `$(git rev-parse --git-common-dir)/hooks/loopflow-build-cache.py`,
refreshed only by `uv run python scripts/build_cache.py install`. `diff -q` it against the
branch's script first; a stale copy tests yesterday's code.

lf makes a Task's checkout the way the real repository does, so drive that sequence by hand:
`git worktree add --quiet --force --detach ../<repo>.qa-ready <base>` (this is when the hook
seeds), then `git worktree move --force` it to the Task path and
`git switch --quiet --no-track -c <branch> <base>` inside it. The switch has a non-null previous
HEAD, so it never re-seeds. `<repo>.ready` is usually the newest-ranked source on this Machine.
`target/.loopflow-seed` and `build_cache.py status` (`seed     from …`) name the source.

`clonefile(2)` keeps file mtimes but gives cloned directories the clone time; sources are
ranked by `target/debug/.fingerprint` mtime, which moves only when a new unit directory
appears (workspace unit hashes don't depend on the checkout path). Compare the ranks with
`stat -f "%m %N" <tree>/target/debug/.fingerprint` across `git worktree list`.

A Cargo build in a sibling holds its locks: run `git worktree add` while it builds to see the
seeder skip it. A non-cloning volume: `hdiutil attach -nomount ram://65536`, `newfs_hfs`,
`mount -t hfs` under a `/tmp` directory, add a worktree there; the hook stays silent, no
`target/` appears, and a manual `build_cache.py seed` prints `[Errno 18] Cross-device link`.
`hdiutil detach` afterwards.

In an isolated `HOME` (see the qa skill's shared-cache notes) builds run client-side: no
sccache daemon starts, so there is nothing to stop.

To exercise the busy-source skip on the real repository, start a build in a fresh seed and
`touch` its `target/debug/.fingerprint` so it ranks first; the next `git worktree add` must name
another source. For the reverse race, hold a seed's locks from Python
(`with build_cache._quiescent(Path("target/debug"))`, `sys.path` pointed at `scripts/`) and run
`cargo build -v` there: it prints `Blocking waiting for file lock` until the block exits.
