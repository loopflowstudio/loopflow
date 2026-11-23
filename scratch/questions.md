# Open questions

- After seeding, about 200 CPU-s of a fresh `cargo test -p loopflow --lib`
  build is workspace libraries compiled in the incremental test profile
  (execution 110–126 s). sccache doesn't serve those across worktrees. Making
  dependency workspace libs non-incremental in `[profile.test]` would let it,
  but costs the repeated-edit loop LOO-921 measured (execution edit 15 s vs
  86 s). It is left as is; a separate Task could measure per-package
  `incremental = false` for non-edited crates.
- Seeding is on wherever `build_cache.py install` ran, with no knob; a cold
  target is `rm -rf target`. Chosen over opt-in because install is already the
  opt-in, and the probe showed −58% CPU on a fresh `lf` build. An env-var
  opt-out was dropped in concept review as a duplicate of deleting the
  directory.
- The seeded `lf` build's largest process was 3.14–3.18 GB in all three
  samples; cold was 2.05–2.09 GB in two of three (3.20 in cohort 1). A guess:
  the `loopflow` lib compile hits sccache in some cold samples but misses after
  a seed from another revision, whose registry artifacts give different keys.
  The probe can't tell, because shared-cache counters include other agents.
  A private-cache rerun of one seeded sample would show it.
