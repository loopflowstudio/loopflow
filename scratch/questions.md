# Questions

## update-wave: scratch has only validation procedures

Both scratch files (`chord-model-tend-flow-steps.md`, `jack-heart.chord-model.20260318_1226-review.md`) contain only validation commands — no forward-looking analysis, proposals, or design content. The shipped item (old `02-tend-flow-steps`) was already folded out during the build steps and replaced with `02-vsm-flow.md` and `02a-worker-pools.md`. Nothing to move into wave.

Decision: leave scratch as-is. The files serve the reviewer.

## update-wave: post-ship pass (2026-03-19)

Deleted shipped `02-vsm-flow.md`. Folded review risk (scan prompts need graceful skip when data sources are unavailable) into `02c-vsm-chord-configs.md`. Trimmed scratch design doc and review to validation-only. Items 02a–02d, 03–08 verified — all future work, coherent against current codebase.
