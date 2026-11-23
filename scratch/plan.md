# LOO-922: seed a new worktree's Rust target by APFS clone

## Outcome

`git worktree add` (by hand or by lf for a Task) starts the new checkout's
`target/debug` as a copy-on-write clone of the newest quiescent target in the
same repository. Cargo still decides freshness. Registry crates, their build
scripts (aws-lc-sys, ring, libsqlite3-sys, sha2-asm) and proc macros come back
`Fresh`. The workspace crates rebuild because checkout gives them new mtimes,
and sccache serves them as before. Nothing new runs: no daemon, no service.

Demo: in a repo with the cache installed, `git worktree add ../x main && cd ../x
&& time cargo build -p loopflow --bin lf` prints 7 `Compiling` lines (workspace
only), not 256, and finishes in about 40% of the cold time.
`uv run python scripts/build_cache.py status` shows which target it was seeded from.

## What the probes showed (2026-11-22, base 0b3e42bd41)

Conditions: 16 CPUs, load average 48–62 from other agents, 2 Cargo jobs, the
pinned sccache warm. One sample per arm. Receipts are in `scratch/probe/*.sum`
and the `-v` logs beside them; the driver is `scratch/probe/measure.sh`.

| arm | wall | CPU (user+sys) | peak RSS | units compiled / fresh |
|---|---|---|---|---|
| cold `cargo build -p loopflow --bin lf` | 196 s | 208 s | 2.24 GB | 256 / 0 (35 build scripts run) |
| seeded, same command | 83 s | 87 s | 2.21 GB | 7 / 249 (5 build scripts: workspace only) |
| `cargo test -p loopflow --lib --no-run` after that cold build | 326 s | 266 s | 3.28 GB | 78 / 222 |
| seeded (`incremental/` cloned) | 274 s | 229 s | 3.36 GB | 8 / 292 |
| seeded (`incremental/` removed) | 216 s | 216 s | 3.32 GB | 8 / 292 |

- **Clone cost:** `cp -c -R` of a 2.1 GB / 3,373-file target took 1.6 s, and
  5.1 GB took 7.2 s. Both sources were on APFS, so it cloned; it would copy
  elsewhere. Free space moved by about 12 MB, which is within noise.
  After the seeded `lf` build, 0.84 GiB of the target had been rewritten and
  1.17 GiB was still shared. A cold build allocates the full 2.0 GiB.
- **Freshness is Cargo's:** fingerprints for registry units carry no checkout
  path, so they stay fresh. Every workspace unit is dirty in a new checkout
  (sources newer than dep-info), so a clone can never serve stale workspace code
  to Cargo.
- **Paths:** the cloned `build/*/output` files still name the source target
  (`rustc-link-search=native=…probe-a/target/…`). Cargo 1.98 rewrites the
  recorded OUT_DIR at use: no rustc command in the seeded build named the source
  checkout. The seeded `lf` ran and reported the right build. Once the source is
  removed, the seed still works because the blocks are shared and there are no
  links.
- **Incremental state doesn't carry across paths:** with the cloned
  `incremental/`, execution's test-profile compile took 126 s; without it,
  110 s. The clone excludes `incremental/`.
- **What's left in a fresh test build:** about 200 CPU-s is workspace
  libraries compiled in the incremental test profile (loopflow-execution
  110–126 s, the `loopflow` lib test 70–92 s, storage 32–45 s). sccache doesn't
  serve those across worktrees. Seeding can't touch them. See Exclusions.
- **Quiescence:** Cargo 1.98 keeps `target/debug/.cargo-lock`,
  `.cargo-build-lock` and `.cargo-artifact-lock`. Holding any one of them
  exclusively with `flock` made a second `cargo build` print "Blocking waiting
  for file lock". The seeder takes each existing lock with `LOCK_EX|LOCK_NB`.
  If any lock is busy it skips that source.

## Design

Source of truth: Cargo's fingerprints. The seeder never edits mtimes, never
marks anything fresh and never hardlinks.

In `scripts/build_cache.py`:

- `seed_target(repo)`. Do nothing if `repo/target` exists. Otherwise list the
  repository's worktrees (`_worktree_roots`) that have `target/debug`, newest
  `target/debug/.fingerprint` mtime first. For the first one whose Cargo locks
  can all be taken non-blocking, clone the seed into `repo/target/debug.seeding`
  while holding the locks, then rename it to `target/debug`. The rename is
  atomic, so a Cargo started mid-seed sees either no target or a whole one.
  Write `target/.loopflow-seed` with the source path and commit for `status`.
- What a seed is: exactly three entries of the source's `target/debug`,
  `.fingerprint`, `build` and `deps` (Cargo's freshness records, build-script
  outputs and compiled units), each cloned into the empty `debug.seeding`.
  Everything else is never copied: `incremental/` (path-bound, measured slower
  when carried), the lock files, and the uplifted top-level files (`lf`, `*.d`,
  `*.rlib`). Name what goes in, not what comes out afterwards; there is no
  delete step. A source missing one of the three is skipped.
- Clone: on macOS, one `clonefile(2)` call per entry through `ctypes`. It
  clones a whole directory tree, symlinks included (probed), and returns an
  error such as EXDEV or ENOTSUP instead of copying. Don't use `cp -c`: its man
  page says it falls back to `copyfile(2)`, a full copy, when it can't clone.
  On Linux, `cp -a --reflink=always` per entry, which fails rather than
  copying. Any failure removes `debug.seeding` and leaves the checkout
  unseeded, which is today's cold build. Never make a full copy.
- The post-checkout hook block already runs `register` on a branch checkout.
  It now runs `seed` first, only when `$1` is the null OID (a new worktree), and
  stays silent and best-effort (`|| :`), like register.
- `status` prints `seed     from <worktree> at <short sha>`, or `seed     none`.
- On whenever installed: `build_cache.py install` is already the opt-in for
  this Machine-level wiring, and there is no knob. A cold target is
  `rm -rf target` after `git worktree add`, which is what the benchmarks
  already do between arms. (Choice made for review: an env var
  `LOOPFLOW_SEED_TARGET=0` was passed over because it duplicates deleting the
  directory and would be one more thing the hook copy and docs must carry.)
- The hook runs a copy of this script (`loopflow-build-cache.py` in the Git
  hooks directory), refreshed only by `install`. Existing installs seed after
  one `uv run python scripts/build_cache.py install`; slice 1 does that here
  and testing.md says so.

Why delete the uplifted files: before its first build, a fresh checkout
holding another branch's `target/debug/lf` would run that branch's code
(`scripts/check_context_launch.py`, the benchmarks' `--lf` default). Every
workspace unit rebuilds and re-uplifts anyway, so deleting them costs nothing.

## Acceptance check at gate

`uv run pytest python/tests/test_build_cache.py` with real git and real cargo
on a tiny two-crate fixture workspace (one path crate plus one with a build
script). It checks that:

1. A seeded new worktree's `cargo build -v` reports the build-script crate
   `Fresh` and the path crate `Compiling`.
2. A source whose `.cargo-build-lock` is held is skipped. With no other source
   there is no `target/`, and the build still passes.
3. No top-level binary and no `incremental/` from the source exists before
   the first build.
4. When the clone fails partway (the clone function patched to clone the
   first entry and then raise), neither `target/debug.seeding` nor
   `target/debug` is left behind, and the next `cargo build` passes.

Breaking the lock check or the freshness claim fails one of these.

Machine check, run once by implement and recorded in the PR: a
`test_loop.py`-style arm, a new worktree then `cargo build -p loopflow --bin lf`
and `cargo test -p loopflow --lib <filter>` passing, seeded vs `rm -rf target`
after the add,
three alternating samples each, reporting wall, CPU, RSS and rewritten bytes.

## Absent and error states

- No source with a target, or all sources busy: no seed, a cold build as
  today, and `status` says `seed     none`.
- Source on another volume or filesystem without clones: the clone fails and
  there's no seed.
- Different Cargo.lock or toolchain in the source: units that don't match
  simply aren't reused (they compile). The stale units share blocks until
  `lf wt clean --builds` reaps the target.
- `build_cleanup` measures `st_blocks`, so it counts a clone at full size.
  It caps bytes removed per run (64 GiB), so it reaps fewer roots per pass and
  overstates what it freed. Nothing is deleted wrongly. This is noted in
  testing.md and left alone.

## Exclusions

- Test-profile workspace libraries (about 200 CPU-s of a fresh focused test
  build). One fix is to make dependency workspace libs non-incremental in
  `[profile.test]` so sccache serves them, but that undoes LOO-921's measured
  edit loop (execution edit 15 s vs 86 s). That decision is LOO-921's; the
  tension is recorded in `scratch/questions.md`, not changed here.
- Release profile, `target/llvm-cov-target` and other profile dirs: only
  `debug` is seeded.
- TLS dual implementations, prebuilt native libraries: with registry build
  scripts fresh after seeding, the native C compiles cost nothing in a seeded
  checkout. Nothing to remove.
- Packaged artifacts and Desktop (LOO-920), worktree placement (LOO-681),
  live-checkout protection (LOO-543).

## What goes

Nothing is replaced. The cold path stays as the fallback. In testing.md, "A
fresh target still compiles and links test executables, binaries, proc-macros
and build scripts" becomes the seeded behaviour plus what still compiles.

## Slices

1. **Seed in the hook, end to end.** Add `seed_target`, the hook block's
   null-OID branch, the `status` line, and the fixture tests 1–4. Reinstall the
   hook here with `build_cache.py install`. Then create
   a real worktree here and watch `cargo build -p loopflow --bin lf` print
   7 `Compiling` lines. If Cargo or the clone disagrees with the probe, this
   slice is where it shows.
2. **Measured arm.** Add the seeded/unseeded arm to
   `scripts/benchmarks/crate-rebuild/test_loop.py` (or a sibling driver), run
   three cohorts, and render results with `report.py` as
   `results/loo-922.html`.
3. **Docs.** Update testing.md's Shared compilation cache section and the
   crate-rebuild README row.

After verify: check LOO-923 status/liveness, `lf task checkout LOO-923` if
absent, then `lf ship LOO-923` with these measurements in its context.
