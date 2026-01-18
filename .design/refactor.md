# Refactor

Package reorganization: `cli/` → `lf/`, `lfops.py` → `lfops/` with submodules, move shared types to `lf/models.py`.

## Summary

This branch reorganizes the loopflow package structure for better separation of concerns.

## Changes

### Package structure

```
src/loopflow/
├── lf/                    # Core task execution (was cli/)
│   ├── __init__.py        # CLI entry point
│   ├── run.py             # Task execution logic
│   ├── context.py         # Prompt assembly
│   ├── config.py          # Configuration loading
│   ├── files.py           # File gathering
│   ├── git.py             # Git operations
│   ├── launcher.py        # Agent launchers (Claude, Codex, Gemini)
│   ├── pipeline.py        # Pipeline execution
│   ├── pipelines.py       # Pipeline definitions
│   ├── models.py          # Session/SessionStatus (shared with lfd)
│   ├── messages.py        # LLM HTTP integration (was llm_http.py)
│   ├── frontmatter.py     # Task frontmatter parsing
│   ├── tokens.py          # Token counting
│   ├── voices.py          # Voice/persona loading
│   ├── worktrees.py       # Worktree management
│   ├── design.py          # Design doc gathering
│   ├── logging.py         # Log file management
│   └── builtins/          # Built-in prompts and templates
│
├── lfops/                 # Git workflow commands (was lfops.py)
│   ├── __init__.py        # Lazy entry point
│   ├── commands.py        # Typer app, registers submodules
│   ├── _helpers.py        # Shared utilities
│   ├── init.py            # lfops init, install, doctor, version
│   ├── pr.py              # lfops pr
│   ├── land.py            # lfops land
│   ├── commit.py          # lfops commit
│   ├── rebase.py          # lfops rebase
│   └── summarize.py       # lfops summarize, summary loading
│
├── lfd/                   # Daemon and agents
│   ├── client.py          # Daemon client (re-exports session logging)
│   ├── work/              # Work queue (moved from root)
│   └── ...                # Agent management, server, triggers
│
├── lfwork.py              # Work queue CLI
└── publish.py             # PyPI publishing
```

### Key moves

| From | To |
|------|-----|
| `cli/` | `lf/` |
| `cli/run.py` | `lf/run.py` |
| `llm_http.py` | `lf/messages.py` |
| `lfops.py` (monolith) | `lfops/` (submodules) |
| `summarize.py` | `lfops/summarize.py` |
| `work/` | `lfd/work/` |
| `lfd/pipelines.py` | `lf/pipelines.py` |

### Circular import fix

Session and SessionStatus were in `lfd/client.py`, but `lf/run.py` needed them. Moving to `lf/models.py` breaks the cycle:

- `lf/models.py` — defines Session, SessionStatus, fire-and-forget logging
- `lfd/client.py` — re-exports for backwards compatibility

### lfops split pattern

Each submodule exports `register_commands(app)`:

```python
# lfops/commands.py
from loopflow.lfops import init as init_module
init_module.register_commands(app)
```

This keeps the Typer app clean and allows lazy loading.

### Removed

- `work_queue` field from `PromptComponents` (caused circular import; work queue now accessed directly)
- `lfwt.py` (commands moved or removed)
- `test_automode.py`, `test_backend.py`, `test_task_args.py`, `test_pipeline.py` (obsolete tests)

## Decisions

- **Lazy imports in lfops/__init__.py**: Avoids loading all submodules at import time. Tests use `get_app()` instead of importing `app` directly.
- **Re-exports in lfd/client.py**: Backwards compatibility for code importing `log_session_start` from there.
- **work/ under lfd/**: Work queue is daemon-related functionality, not core lf.
