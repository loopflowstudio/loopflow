# Builtins

Built-in commands that work without `lfops init`. User commands in `.lf/` or `.claude/commands/` override builtins.

## Implementation

Tasks resolve to bundled templates when no user-defined task file exists. The builtin templates live at `src/loopflow/templates/commands/`.

### Resolution order

1. `.claude/commands/{name}.md` (user override)
2. `.lf/{name}.lf` (user override)
3. `.lf/{name}.md` (user override)
4. `.lf/{name}.*` (user override, any extension)
5. `.lf/{name}` (bare name)
6. `templates/commands/{name}.md` (builtin fallback)

### Key functions

In `src/loopflow/context.py`:

- `_get_builtin_task(name)` — returns path to bundled template if it exists
- `list_builtin_tasks()` — returns names of all builtin tasks
- `list_user_tasks(repo_root)` — returns names of user-defined tasks
- `list_all_tasks(repo_root)` — returns (user_tasks, builtin_only_tasks) tuple
- `gather_task(repo_root, name)` — resolves task with builtin fallback

### Discoverability

`lf` with no args lists available tasks and pipelines:

```
Pipelines:
  ship

Tasks:
  custom-task
  design (builtin)
  implement (builtin)
  ...
```

User-defined tasks appear first (no suffix), then builtin-only tasks (marked `(builtin)`).

### Changes to `lfops init`

- Removed `--prompts` flag
- Removed `--all` flag
- Removed prompt copying (commands are built-in now)
- Added `--style` flag to optionally install STYLE.md and PROMPTS.md
- Config template still copied for pipelines and settings

## Builtin commands

- **design** — Create implementation spec in `.design/`. Interactive.
- **implement** — Turn design doc into code.
- **review** — Written assessment, verdict in `.design/`.
- **iterate** — One focused improvement to branch code.
- **polish** — Fix issues, run tests, get to green.
- **debug** — Debug error from clipboard (`-v`).
- **reduce** — Simplify code while preserving user behavior.
- **expand** — Explore ambitious changes beyond current scope.
- **explore** — Interactive Q&A about the current diff.

## Not yet implemented

- Tab completion for builtin names
- Maestro UI changes to show builtins in task selector
