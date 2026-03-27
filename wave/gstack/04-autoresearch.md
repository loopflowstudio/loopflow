# Stage 4: Autoresearch flow

Import Karpathy's autoresearch loop as a loopflow flow. Single prompt, autonomous loop, git as experiment infrastructure.

## What to build

**A flow, not a workstyle.** Autoresearch is a loop pattern any workstyle can use: edit a file, run a measurement, keep or discard based on a single metric. The human writes the program.md (the "what to optimize"), the agent runs the loop.

**Four steps, not one.** Karpathy has it as a monolithic `program.md`, but it's doing four distinct things. Decomposing makes each step composable — swap evaluate for a benchmark, a test suite, a bundle size check. The loop pattern stays the same.

### Flow definition

```yaml
# .lf/flows/autoresearch.yaml
- autoresearch:setup
- loop:
    steps:
      - autoresearch:experiment
      - autoresearch:evaluate
      - autoresearch:decide
```

- **setup** — agree on run tag, create branch, verify data/harness, initialize results.tsv
- **experiment** — read git state and results history, form hypothesis, edit the target file, commit
- **evaluate** — run the measurement command, read metrics from output
- **decide** — compare to best result, keep commit or git reset, log to results.tsv

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

### Step conversion

Karpathy's `program.md` gets decomposed into four loopflow steps. Key elements to preserve across them:
- The autonomous spirit ("NEVER STOP") — lives in the flow's loop, not a single step
- The metric definition (val_bpb, lower is better) — parameterizable per wave
- The git keep/discard pattern — lives in `decide` step
- The fixed time budget per experiment — lives in `evaluate` step
- The results.tsv logging format — lives in `decide` step
- The single-file constraint — lives in wave `area` config

Strip: repo-specific setup (data download, env verification) — that's wave config, not prompt content.

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
