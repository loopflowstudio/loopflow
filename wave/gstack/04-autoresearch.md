# Stage 4: Autoresearch flow

Import Karpathy's autoresearch loop as a loopflow flow. Single prompt, autonomous loop, git as experiment infrastructure.

## What to build

**A builtin flow, not a workstyle.** Autoresearch is a loop pattern any workstyle can use: edit a file, run a measurement, keep or discard based on a single metric. The steps are written directly as loopflow builtins (like `implement` or `gate`) — no external sync, no workstyle wrapper. Inspired by Karpathy's autoresearch repo but maintained as native loopflow prompts.

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

### Writing the steps

These are builtin loopflow steps, not imports. We write them directly in `rust/loopflow/src/engine/builtins/steps/research/`. Key elements from Karpathy's design to carry forward:
- The autonomous spirit ("NEVER STOP") — lives in the flow's loop construct
- The metric definition — parameterizable per wave config
- The git keep/discard pattern — lives in `decide` step
- The fixed time budget per experiment — lives in `evaluate` step
- The results.tsv logging format — lives in `decide` step
- The single-file constraint — lives in wave `area` config

No sync needed. These are our prompts, inspired by the pattern.

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
