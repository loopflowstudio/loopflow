# Stage 4: Autoresearch flow

**Finish line:** `lf autoresearch:experiment` works as a step and a `flow: autoresearch, mode: loop` wave runs Karpathy's experiment loop autonomously against his `train.py`/`prepare.py` setup.

Import Karpathy's autoresearch loop as a loopflow flow. Single prompt, autonomous loop, git as experiment infrastructure.

## What to build

**A synced flow, not a workstyle.** Syncing autoresearch produces steps and a flow YAML — no workstyle is created. The sync tool doesn't decide what the thing is. It converts whatever it finds and writes the pieces. A workstyle only exists when someone publishes the full bundle and names it one (like gstack). Karpathy published a loop and some prompts — that's a flow.

**Four steps, not one.** Karpathy has it as a monolithic `program.md`, but it's doing four distinct things. Decomposing makes each step composable — swap evaluate for a benchmark, a test suite, a bundle size check. The loop pattern stays the same.

### Flow definition

```yaml
# .lf/flows/autoresearch.yaml
- loop:
    steps:
      - autoresearch:experiment
      - autoresearch:evaluate
      - autoresearch:decide
```

- **experiment** — read git state and results history, form hypothesis, edit the target file, commit
- **evaluate** — run the measurement command, read metrics from output
- **decide** — compare to best result, keep commit or git reset, log to results.tsv

Setup (branch creation, data verification, results.tsv initialization) is stripped — loopflow handles that through wave config and `lf init`.

### Wave usage

```yaml
# wave/ml-experiment/ml-experiment.yaml
flow: autoresearch
mode: loop
area:
  - train.py
```

The user provides:
- `train.py` (or whatever file the agent edits)
- `prepare.py` (the fixed evaluation harness)
- A `program.md` equivalent as the step content or in scratch/

## Sync approach

Stage 3 shipped sync as a gstack-specific subcommand (`lf op gstack sync/diff/list` in `rust/loopflow/src/ops/gstack.rs`), not the generic `lf op workstyle sync <name>` originally designed. Two paths forward for autoresearch:

1. **Bespoke `lf op autoresearch sync`** — mirror the gstack implementation. Fastest path to working, follows the precedent on disk.
2. **Generalize first** — refactor `lf op gstack` into `lf op workstyle sync gstack` + `lf op workstyle sync autoresearch`, with per-workstyle config in `workstyle.yaml`. Pays the generalization cost once; removes a second bespoke subcommand.

Recommend option 1 for the first cut — validate that autoresearch works as a synced flow before investing in the abstraction. Generalize when a third workstyle appears.

### Converter reuse

The Python converter (`python/loopflow/workstyle/convert.py`) is gstack-specific today — step rename tables, browser-skill set, telemetry-stripping rules are hardcoded for gstack's SKILL.md format. Autoresearch is a single `program.md`, not a SKILL.md tree, so the converter needs:

- A separate entry point (e.g. `convert_autoresearch`) that reads one `program.md` and writes three step files plus a flow YAML, or
- An abstraction over "source format → step/flow outputs" that both gstack and autoresearch call into.

Either way, the direction-extraction plumbing (writing to `rust/loopflow/src/engine/builtins/directions/`) already works generically and can be reused.

### Writing the steps

What the converter extracts into steps:
- **experiment** — how to form a hypothesis from prior results, what kind of edits to try
- **evaluate** — how to run the measurement and extract the metric
- **decide** — the keep/discard logic (compare to best, commit or reset)

The sync tool writes directly to loopflow's native locations:

```
.lf/steps/autoresearch/
  experiment.md
  evaluate.md
  decide.md
.lf/flows/
  autoresearch.yaml
```

No workstyle directory, no wrapper. Just steps and a flow.

What's stripped (loopflow already provides):

| Karpathy's program.md | Already in loopflow |
|---|---|
| "NEVER STOP", "don't ask the human" | Headless surface mode |
| Branch creation, run tagging | `lf ops` / wave config |
| Results logging | scratch/ or wave state |
| "Only edit train.py" | Wave `area` config |
| Fixed time budget | Step/wave config |

### What makes this general

The autoresearch pattern works beyond ML training:
- Performance optimization (edit code, benchmark, keep if faster)
- Size reduction (edit code, measure bundle size, keep if smaller)
- Prompt engineering (edit prompt, evaluate output quality, keep if better)

The step should be parameterizable — the metric, the file to edit, the evaluation command, the time budget. But start with Karpathy's exact setup as the concrete first version.

## Done when

1. `lf autoresearch:experiment` runs the autoresearch loop step
2. A wave with `flow: autoresearch, mode: loop` runs experiments autonomously
3. Git history shows keep/discard pattern (commits advance, failures reset)
4. results.tsv accumulates experiment results
5. The step works with Karpathy's exact train.py/prepare.py setup
