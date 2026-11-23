# Open questions

- After seeding, about 200 CPU-s of a fresh `cargo test -p loopflow --lib`
  build is workspace libraries compiled in the incremental test profile
  (execution 110–126 s). sccache doesn't serve those across worktrees. Making
  dependency workspace libs non-incremental in `[profile.test]` would let it,
  but costs the repeated-edit loop LOO-921 measured (execution edit 15 s vs
  86 s). It is left as is; a separate Task could measure per-package
  `incremental = false` for non-edited crates.
- Seeding is on by default wherever `build_cache.py install` ran, and
  `LOOPFLOW_SEED_TARGET=0` skips it. This was chosen over opt-in because install
  is already the opt-in, and the probe showed −58% CPU on a fresh `lf` build.
