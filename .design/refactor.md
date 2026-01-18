# Refactor

Package reorganization: `cli/` to `lf/`, `lfops.py` to `lfops/` with submodules, move shared types to `lf/models.py`.

## Review

**Verdict:** Ready to ship

Clean refactor. The package structure is clearer, circular imports are resolved, and the test updates are straightforward. No bugs found.

## Design notes

### Package structure

- `lf/` — Core task execution (CLI, context assembly, launchers)
- `lfops/` — Git workflow commands (pr, land, commit, rebase, summarize)
- `lfd/` — Daemon and agents, now includes `work/` queue

### Circular import fix

Session and SessionStatus moved from `lfd/client.py` to `lf/models.py`. The client re-exports for backwards compatibility.

### lfops split pattern

Each submodule exports `register_commands(app)`. Tests use `get_app()` for lazy loading.

### Removed

- `lfwt.py` — Commands moved to `wt` (worktrunk) or removed
- `test_automode.py`, `test_backend.py`, `test_task_args.py`, `test_pipeline.py` — Logic consolidated into existing test files
