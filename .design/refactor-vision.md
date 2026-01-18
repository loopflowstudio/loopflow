# Code Structure Reorganization

## Status

**Implemented.** Tests pass (381/381).

## New Structure

```
src/loopflow/
├── __init__.py          # version only
│
├── lf/                  # Task runner (core)
│   ├── __init__.py      # Typer app, --list, routing
│   ├── run.py           # Task execution, inline prompts
│   ├── context.py       # Prompt assembly
│   ├── launcher.py      # Claude/Codex/Gemini execution
│   ├── pipeline.py      # Pipeline execution
│   ├── design.py        # Design doc helpers
│   └── builtins/        # Built-in prompt templates
│
├── lfops/               # Git workflow
│   ├── __init__.py      # Re-exports main, app
│   └── commands.py      # All commands (pr, land, commit, etc.)
│
├── lfd/                 # Agent daemon (already structured)
│   ├── work/            # Work queue (moved from top-level)
│   └── ...
│
└── lib/                 # Shared infrastructure
    ├── config.py        # .lf/config.yaml loading
    ├── git.py           # Git operations
    ├── files.py         # File gathering, binary detection
    ├── tokens.py        # Token counting
    ├── frontmatter.py   # YAML frontmatter parsing
    ├── logging.py       # Session logging
    ├── worktrees.py     # Worktree utilities
    ├── llm_http.py      # LLM API calls
    └── voices.py        # Voice loading
```

## Changes Made

1. Created `lib/` for shared infrastructure
2. Renamed `cli/` to `lf/`, moved `context.py`, `launcher.py`, `pipeline.py`, `design.py`, `builtins/`
3. Created `lfops/` package from `lfops.py`
4. Moved `work/` under `lfd/`
5. Deleted `lfwt.py` (redundant with `wt`)
6. Updated all imports (src + tests)
7. Fixed circular import (lfd → lf.context → lfd.work)
8. Updated pyproject.toml entrypoints

## Test Consolidation

Refactored test structure to match the new package layout:

1. Merged `test_backend.py` → `test_launcher.py` (runner classes + command building)
2. Merged `test_automode.py` → `test_config.py` (interactive mode config)
3. Merged `test_pipeline.py` → `test_config.py` (PipelineConfig tests)
4. Merged `test_task_args.py` → `test_context.py` (template substitution tests)
5. Removed mock-wiring tests from `test_collector.py` (STYLE.md: "Mock side effects, but don't test mock wiring")

Result: 22 test files, 381 tests (was 25 files, 387 tests).

## Open Questions

1. **lfops splitting** — commands.py is still 1779 lines. Could split into `pr.py`, `land.py`, `commit.py`, etc.
2. **lfwork** — Still a top-level file. Design said make it `lfd work` subcommand.
3. **summarize.py** — Still at top level. Could move to `lfops/` or `lib/`.
4. **publish.py** — Still at top level. Used for PyPI publishing.
