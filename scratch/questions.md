# LOO-664 assumptions — 2026-10-27

- Iteration feedback rejects the previous pre-staged telemetry exception. New
  checkpoints exclude its local changes while retaining staged and working
  contents. Already committed telemetry remains in history; sync does not rewrite
  it or remove its committed baseline.
- Ordinary staged and unstaged edits retain the existing consolidated checkpoint
  behavior. Their former partial-staging partition is not preserved.
- Temporary checkpoint indexes follow the existing selected-document commit
  pattern; existing checkout recovery owns preserving telemetry across the merge.
