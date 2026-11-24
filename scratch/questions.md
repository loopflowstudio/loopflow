# Open choices made for review (LOO-1082)

Headless kickoff — reversible product choices taken in `scratch/plan.md`:

1. **Default `max_loop_passes = 6` (`0` or `null` = unlimited), warning from pass 4 onward, autonomous deciders only.**
   - Passed over: prompt-only (no runtime curb on custom graphs/stubborn models), YAML `max_passes:` on `loop:` (complicates PR #1962's block-only syntax and breaks older parsers), default `4` (tight for `optimize` and legitimate 5–6 pass work).
2. **Silence intermediate running-step wakeups in `task_updates` for a Task's own `task-session` (matching conducted children).**
   - Passed over: keeping per-step wakeups (399 turns / 72.7M cached tokens across 5 field Tasks) or time-based throttling.
3. **Skip LLM `commit-message` calls in `commit_step` when every staged path is under `scratch/` (commit+push with deterministic title `lf <step>: update scratch/...`).**
   - Passed over: keeping LLM calls for temporary scratch files cleared before merge, or skipping commits on `scratch/` entirely.
4. **Keep `build.yaml`'s nested two-loop topology unchanged** (only update comments and builtin skill guidance).
   - Passed over: collapsing the inner `implement`↔`realign` loop into a single pass.
