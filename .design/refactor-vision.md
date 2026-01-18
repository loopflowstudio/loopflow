# Code Structure Reorganization

## Status

**Implemented.** Tests pass (381/381).

## New Structure

```
src/loopflow/
├── __init__.py          # version only
│
├── lf/                  # Task runner (core) + shared infrastructure
│   ├── __init__.py      # Typer app, --list, routing
│   ├── run.py           # Task execution, inline prompts
│   ├── context.py       # Prompt assembly
│   ├── launcher.py      # Claude/Codex/Gemini execution
│   ├── pipeline.py      # Pipeline execution
│   ├── pipelines.py     # Pipeline definition data structures
│   ├── design.py        # Design doc helpers
│   ├── config.py        # .lf/config.yaml loading
│   ├── git.py           # Git operations
│   ├── files.py         # File gathering, binary detection
│   ├── tokens.py        # Token counting
│   ├── frontmatter.py   # YAML frontmatter parsing
│   ├── logging.py       # Session logging
│   ├── worktrees.py     # Worktree utilities
│   ├── messages.py      # LLM message generation (commit, PR)
│   ├── models.py        # Session data structures
│   ├── voices.py        # Voice loading
│   └── builtins/        # Built-in prompt templates
│
├── lfops/               # Git workflow
│   ├── __init__.py      # Re-exports main, app
│   └── commands.py      # All commands (pr, land, commit, etc.)
│
├── lfd/                 # Agent daemon
│   ├── work/            # Work queue (moved from top-level)
│   └── ...
│
├── lfwork.py            # Work queue CLI (standalone)
├── summarize.py         # Codebase summarization
└── publish.py           # PyPI publishing
```

## Changes Made

1. Renamed `cli/` to `lf/`, consolidating all core functionality
2. Created `lfops/` package from `lfops.py`
3. Moved `work/` under `lfd/`
4. Moved `pipelines.py` from `lfd/` to `lf/` (used by core, not daemon-specific)
5. Renamed `llm_http.py` to `messages.py`
6. Created `lf/models.py` for Session and fire-and-forget session logging
7. Deleted `lfwt.py` (redundant with `wt`)
8. Updated all imports (src + tests)
9. Fixed circular import (lfd → lf.context → lfd.work)
10. Updated pyproject.toml entrypoints

## Test Consolidation

Refactored test structure to match the new package layout:

1. Merged `test_backend.py` → `test_launcher.py` (runner classes + command building)
2. Merged `test_automode.py` → `test_config.py` (interactive mode config)
3. Merged `test_pipeline.py` → `test_config.py` (PipelineConfig tests)
4. Merged `test_task_args.py` → `test_context.py` (template substitution tests)
5. Removed mock-wiring tests from `test_collector.py` (STYLE.md: "Mock side effects, but don't test mock wiring")

Result: 22 test files, 381 tests (was 25 files, 387 tests).

## Open Questions

1. **lfops splitting** — commands.py is ~1550 lines. Could split into `pr.py`, `land.py`, `commit.py`, etc.
2. **lfwork** — Still a top-level file. Could become `lfd work` subcommand.
3. **summarize.py** — Still at top level. Could move to `lfops/`.
4. **publish.py** — Still at top level. Used for PyPI publishing.

## Polish Notes

- Fixed: Removed duplicate `rebase` command definition (shadowed by later definition)
