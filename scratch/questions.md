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
- The measured arm is documented as a sibling driver `seed_probe.py` (plus
  `report.py --seed-probe`), not a mode of `test_loop.py`: its rows (an `lf`
  build then one focused test, seeded vs cold) share nothing with test_loop's
  edit cohorts. Slice 2 should confirm or rename in the README.
- How to measure "bytes rewritten instead of shared": the probe reported
  0.84 GiB rewritten / 1.17 GiB shared, but `measure.sh` doesn't show the
  method. `df` deltas are noisy with other agents writing. Slice 2 picks one
  (e.g. per-file `clonefile` identity via `getattrlist` ATTR_CMNEXT_PRIVATESIZE)
  and the README sentence follows it.
