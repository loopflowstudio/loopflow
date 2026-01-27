# Branch Review: Context Restructuring and Interactive Flows

## What was implemented

This branch introduces three interconnected improvements:

1. **Area-scoped context gathering** — When `--area` is specified, context assembly now separates area content (what the agent is responsible for) from reference docs (background material). Each has its own token budget.

2. **Interactive steps within flows** — Flows can now contain interactive steps. When `lf design-and-ship` runs, the `design` step opens a full coding agent session; when the user exits (Ctrl+D), the flow continues with `implement → reduce → polish` in auto mode.

3. **Renamed `-p/--path` to `--area`** — The CLI flag now reflects its semantic meaning: scoping the agent's working area, not just adding context files.

## Key choices

### Area vs reference doc separation

The context now assembles in two layers:
- **Area docs**: All files in the area directory + `roadmap/<area>/` items
- **Reference docs**: `scratch/`, ancestral roadmap docs, root `.md` files

This prevents large areas from crowding out essential reference material. Each layer has a configurable token budget (`budgets.area`, `budgets.docs`, `budgets.diff`).

**Alternative rejected**: Single budget for all docs. This worked poorly when areas had many files—reference docs (README, STYLE) got dropped first, leaving the agent without style guidance.

### subprocess.run for interactive flow steps

When running an interactive step within a flow, we use `subprocess.run()` instead of `os.execvp()`. The standalone interactive path uses `execvp` to replace the process entirely, but flows need the flow runner to continue after the step completes.

**Trade-off**: Interactive steps in flows don't get the same "clean replacement" semantics as standalone runs. TTY handling works correctly, but the step runs as a subprocess with a slightly different environment.

### consolidate added to ship flow

The `ship` flow now ends with `consolidate` to clean up `scratch/` before the PR is ready. This keeps PRs tidy without requiring manual cleanup.

## How it fits together

```
lf <step> --area src/api/
    │
    ├── gather_area(src/api/)           → all files in area
    ├── gather_ancestral_docs(src/api/) → parent roadmap items
    ├── gather_design_docs()            → scratch/
    ├── gather_docs()                   → root *.md
    │
    ├── _limit_to_budget(area_docs, budget_area)
    ├── _limit_to_budget(ref_docs, budget_docs)
    │
    └── format_prompt()
        ├── System docs (loopflow)
        ├── Run mode
        ├── Reference material
        ├── Instructions (direction, step)
        └── Working context (diff, clipboard)
```

For flows with interactive steps:

```
lf flow design-and-ship
    │
    ├── _is_step_interactive(design) → true (from frontmatter)
    │   └── _run_interactive_step()  → subprocess.run(), waits for user
    │
    ├── _is_step_interactive(implement) → false
    │   └── _run_step() → collector-based auto execution
    │
    └── ... (reduce, polish)
```

## Risks and bottlenecks

**Token budget defaults may not fit all repos.** The defaults (area: 50k, docs: 30k, diff: 20k) work for loopflow-sized repos but may need tuning for larger codebases. Users can configure via `.lf/config.yaml`.

**Area gathering includes all files.** `gather_area()` reads every file in the area directory, including non-code files. Binary files are skipped, but large text files (logs, data) could consume budget. Consider adding exclusion patterns if this becomes a problem.

**Interactive flow steps share flow's TTY.** Works fine for single interactive steps, but multiple interactive steps in a flow would run sequentially with shared TTY state. Not tested extensively.

## What's not included

- **Dynamic budget allocation** — Budgets are fixed per section. A smarter approach might shift budget from area to docs if area is small.
- **Summary fallback** — The code checks for summaries but doesn't generate them if missing. Users must run `lfops summarize` explicitly.
- **Area validation** — If `--area` points to a nonexistent path, it silently returns empty docs. Could warn the user.

## Files changed

### Core implementation

| File | Change |
|------|--------|
| `src/loopflow/lf/context.py` | Added budget limiting (`_limit_to_budget`), restructured doc gathering with area/docs/diff budgets |
| `src/loopflow/lf/design.py` | Added `gather_area()` (all files in area), `gather_ancestral_docs()` (parent docs) |
| `src/loopflow/lf/flow.py` | Added `_is_step_interactive()`, `_run_interactive_step()`, flow direction propagation |
| `src/loopflow/lf/flows.py` | Added `interactive` field to Step dataclass, direction now supports list |
| `src/loopflow/lf/step.py` | Renamed `-p/--path` to `--area`, added budget config wiring |
| `src/loopflow/lf/config.py` | Added `BudgetConfig` dataclass |

### Flows and steps

| File | Change |
|------|--------|
| `src/loopflow/lf/builtins/flows/code/design-and-ship.yaml` | New flow: design → implement → reduce → polish |
| `src/loopflow/lf/builtins/flows/code/ship.yaml` | Added consolidate as final step |
| `src/loopflow/lf/builtins/steps/ops/consolidate.md` | Revised to focus on cleanup rather than roadmap prep |
| `src/loopflow/lf/builtins/steps/ops/add-to-roadmap.md` | Updated to use `--area` for destination |

### Documentation

| File | Change |
|------|--------|
| `src/loopflow/LOOPFLOW.md` | Restructured for area-centric model |
| `docs/*.md` | Updated CLI flags (`--area`), lfd command names (`watch`, `cron`) |
| `README.md` | Updated flow tables, wave examples |

### Tests

| File | Change |
|------|--------|
| `tests/test_design.py` | Updated tests for `gather_area()` and `gather_ancestral_docs()` |

## Test coverage

All 594 tests pass. Key test files:
- `test_design.py`: Tests for `gather_area()` and `gather_ancestral_docs()` functions
- `test_context.py`: Context assembly tests (unchanged, existing coverage)
- `test_flows.py`: Flow parsing including new `interactive` field
