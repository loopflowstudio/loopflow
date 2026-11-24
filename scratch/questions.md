# Open choices made for review (LOO-1082)

Headless kickoff + concept-review — reversible product choices taken in `scratch/plan.md` (and presented with lettered options in `scratch/concept-review.html`):

1. **Default `max_loop_passes = 6` (`0` or `null` = unlimited), warning from pass 4 onward, final-pass notice at `max`, autonomous deciders only.**
   - Passed over: prompt-only (no runtime curb on custom graphs/stubborn models), YAML `max_passes:` on `loop:` (breaks PR #1962's list-only syntax, breaks older parsers, and freezes the limit inside a Task's `CapturedGraph`), default `4` (tight for `optimize` and legitimate 5–6 pass work).
2. **Preserve the decider's `Iterate` verdict on `cursor.progress.verdict` and enforce `max_loop_passes` in `transitions::finish_step` (with truthful `Blocked` reporting in `task_execution` and `task_updates`).**
   - Passed over: discarding `Iterate` inside `RunningGraph::run_skill` (wastes a cold-cache decider session on resume after raising the cap and traps bare `lf task run` in repeated decider runs).
3. **Silence run-start and intermediate running-step wakeups in `task_updates` for a Task's own `task-session` (matching conducted children).**
   - Passed over: keeping per-step wakeups (399 turns / 72.7M cached tokens across 5 field Tasks), waking on run start only, or time-based throttling.
4. **Skip LLM `commit-message` calls in `commit_step` when every staged path (`--no-renames`) is under `scratch/` (commit+push with deterministic title `lf <step>: update scratch/...`).**
   - Passed over: keeping LLM calls for temporary scratch files cleared before merge, or skipping commits on `scratch/` entirely.
5. **Keep `build.yaml`'s nested two-loop topology unchanged** (update comments, builtin skill guidance, and teach the outer-review-loop pattern in `docs/orchestration.md`).
   - Passed over: collapsing the inner `implement`↔`realign` loop into a single pass.
