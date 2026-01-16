# Model Racing

Run the same task with multiple models in parallel, automatically pick the winner, merge it back.

**Status: Implemented**

## Context

The glade-motif branch added parallel step execution using temporary worktrees. Currently parallel steps are verification-only (test, lint)—they don't produce changes, just check the current state passes.

The deferred "parallel mutation steps" feature opens a bigger opportunity: **model racing**.

## What this builds

```yaml
# Pipeline syntax
steps:
  - task: implement
    race:
      - model: claude:opus
      - model: codex:o3
      - model: gemini:2.5-pro
  - review
```

Or CLI shorthand:

```bash
lf implement --race claude,codex,gemini
```

Execution:
1. Fork temp worktrees for each model
2. Run the same task in parallel
3. Collect results when all complete
4. Run a judge step to pick the winner
5. Merge winning changes back to main worktree
6. Clean up temp worktrees

## Why this extends the branch

Parallel worktree execution is already implemented for verification steps. Model racing uses the same infrastructure but adds:
- Same task instead of different tasks
- Merge-back instead of exit-code-only
- Judge step to select winner

## Judge step

The judge compares results and picks one. Options:

**Option A: LLM judge**
Run `compare` task with all diffs, ask which is better. This is already built—`lfwt compare` does this interactively.

**Option B: Test-based**
Run tests against each worktree, pick the one that passes. Fast but only works for testable changes.

**Option C: Hybrid**
If tests exist and pass for exactly one, use that. Otherwise, invoke LLM judge.

Default to LLM judge (option A) since it's general-purpose. The `compare` task already exists and can be run in auto mode.

## Data structures

Extend `StepConfig` or `PipelineStep`:

```python
@dataclass
class RaceConfig:
    models: list[str]
    judge: str = "compare"  # task to run for judging
```

```python
@dataclass
class PipelineStep:
    task: str | None = None
    pipeline: str | None = None
    parallel: list["PipelineStep"] | None = None
    race: RaceConfig | None = None  # NEW
    config: StepConfig | None = None
```

YAML:

```yaml
steps:
  - task: implement
    race:
      - model: claude:opus
      - model: codex:o3
```

## Execution model

For a race step:

1. **Fork**: Create temp worktrees per model
2. **Run**: Execute task concurrently (same as `_run_parallel_group`)
3. **Wait**: Collect results
4. **Short-circuit**: If only one succeeds, use it
5. **Judge**: Run compare task in auto mode
6. **Merge**: Cherry-pick winning commits to main worktree
7. **Cleanup**: Remove temp worktrees

## Implementation

Files to modify:

- `src/loopflow/lfd/pipelines.py`: Add `RaceConfig`, update `PipelineStep`
- `src/loopflow/pipeline.py`: Add `_run_race_step()` function
- `src/loopflow/cli/run.py`: Add `--race` flag to `run` command
- `src/loopflow/context.py`: Add compare-for-race prompt template

New function in `pipeline.py`:

```python
def _run_race_step(
    step: ResolvedStep,
    models: list[str],
    repo_root: Path,
    # ... other params
) -> int:
    """Run task with multiple models, pick winner."""
```

## CLI design

```bash
# Pipeline-based
lf ship   # if pipeline has race step, it races

# Ad-hoc racing
lf implement --race claude,codex,gemini
```

The `--race` flag replaces the current `--parallel` behavior (which creates persistent worktrees for manual comparison). The new behavior:
- Temp worktrees, auto cleanup
- Automatic judge + merge
- Single winning result

Keep `--parallel` for "run and leave both for manual comparison" workflow.

## Merge strategy

After judging, merge winner back:

```python
def _merge_winner(main_worktree: Path, winner_worktree: Path) -> None:
    """Cherry-pick commits from winner into main worktree."""
    # Get commits from winner that aren't in main
    # Cherry-pick them
    # Or: checkout winner files directly
```

Simplest approach: check out winner's files into main worktree, let user commit. Avoids merge complexity.

## Edge cases

**All fail**: Return non-zero exit code, no merge. Pipeline stops.

**Multiple tie**: If judge can't decide, pick first model alphabetically. Or fail with "tie" message.

**Partial success**: If 2/3 succeed, judge between successes only.

## Future work

- **Voting**: Run judge multiple times, pick consensus winner
- **Cost tracking**: Log tokens/cost per model for comparison
- **Custom judge**: User-provided judge task

## What was built

1. `--race` CLI flag: `lf implement --race claude,codex,gemini`
2. Pipeline YAML `race:` syntax
3. `RaceConfig` dataclass in `pipelines.py`
4. `_run_race_step()` in `pipeline.py`
5. LLM judge with inline prompt
6. File-checkout merge (no cherry-pick complexity)
7. Temp worktree cleanup

## Deferred

- Voting/consensus judging (run judge multiple times)
- Cost tracking per model
- Custom judge tasks (always uses inline prompt)
