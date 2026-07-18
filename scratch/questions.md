# Open questions

- `work_placements` is correctly authored as a draft, but #1073 deliberately
  keeps drafts out of the Rust migration registry until `lf release run`.
  The feature code cannot run against a fresh store before that release-owned
  canonicalization. Do not add draft execution or a fallback placement model;
  either cut the canonical migration through the release workflow before this
  implementation lands, or land them together in that release PR.

  Verified in a disposable release-shaped copy: canonicalization produced only
  `0.12.001_work_placements`; migration validation passed; all eight durable
  Store behavior tests and the durable Wave identity test passed, including
  wrong-Home reservation refusal, local-identity protection, and live-Run move
  fencing. The remaining issue is publication order, not schema or runtime
  correctness.
