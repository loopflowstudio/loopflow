# lf: Robust Non-Repo Support

Make `lf` work seamlessly outside git repositories while improving task discoverability with a redesigned `--list` output.

## What to build

1. `lf` with no args launches interactive claude with available context
2. `lf --list` shows formatted task/pipeline listing
3. Graceful degradation outside git repos (no crashes, sensible defaults)

## Data structures

No new data structures. Uses existing `TaskFile`, `Config`, `PromptComponents`.

Add a category mapping for built-in tasks:

```python
BUILTIN_CATEGORIES: dict[str, list[str]] = {
    "Planning & Design": ["design", "explore"],
    "Implementation": ["implement", "iterate", "expand", "reduce"],
    "Quality": ["review", "polish", "debug"],
    "Git": ["commit", "rebase"],
}

BUILTIN_DESCRIPTIONS: dict[str, str] = {
    "design": "Plan what to build",
    "explore": "Investigate current diff",
    "implement": "Build from design doc",
    "iterate": "Improve code on branch",
    "expand": "Explore ambitious extensions",
    "reduce": "Simplify while preserving behavior",
    "review": "Assess code, write verdict",
    "polish": "Fix issues, run tests",
    "debug": "Fix errors from clipboard",
    "commit": "Commit with generated message",
    "rebase": "Rebase onto main",
    "refine": "Iteratively refine text",
}
```

## Key functions

### CLI changes (`cli/__init__.py`)

```python
def _format_task_list(repo_root: Path | None) -> str:
    """Format tasks and pipelines with colors and categories."""
    ...

def main():
    # Change: 'lf' with no args launches interactive claude
    if len(sys.argv) == 1:
        sys.argv = ["lf", "run", "--interactive"]  # or dedicated command

    # Add --list flag handling
    if "--list" in sys.argv or "-l" in sys.argv:
        typer.echo(_format_task_list(repo_root))
        raise SystemExit(0)
```

### New default behavior (`cli/run.py`)

```python
@app.command()
def run(
    task: str = typer.Argument(None, help="Task name"),  # Now optional
    ...
):
    # If no task specified, launch interactive claude with context
    if task is None:
        return _launch_interactive_default(repo_root, ...)
    ...

def _launch_interactive_default(repo_root: Path, ...) -> int:
    """Launch interactive claude with available docs context."""
    components = gather_prompt_components(
        repo_root,
        task=None,
        inline=None,
        include_diff=False,      # No diff without explicit task
        include_diff_files=False,
        include_summaries=True,
    )
    # Launch interactive claude
    ...
```

### Formatted list output

```python
CYAN = "\033[36m"
BOLD = "\033[1m"
DIM = "\033[90m"
YELLOW = "\033[33m"
GREEN = "\033[32m"
RESET = "\033[0m"

def _format_task_list(repo_root: Path | None) -> str:
    config = load_config(repo_root) if repo_root else None
    user_tasks, builtin_only = list_all_tasks(repo_root)

    lines = []

    # Pipelines section
    if config and config.pipelines:
        lines.append(f"{CYAN}{BOLD}PIPELINES{RESET}")
        for name, p in sorted(config.pipelines.items()):
            chain = f" {DIM}→{RESET} ".join(p.tasks)
            lines.append(f"  {BOLD}{name:<14}{RESET} {DIM}{chain}{RESET}")
        lines.append("")

    # Tasks section
    lines.append(f"{CYAN}{BOLD}TASKS{RESET}")
    lines.append("")

    # Built-ins by category
    for category, task_names in BUILTIN_CATEGORIES.items():
        category_tasks = [t for t in task_names if t in builtin_only or t in user_tasks]
        if not category_tasks:
            continue

        lines.append(f"{DIM}{category}{RESET}")
        for name in category_tasks:
            desc = BUILTIN_DESCRIPTIONS.get(name, "")
            info = _get_task_info(repo_root, name)
            badge = f"  {YELLOW}interactive{RESET}" if info.get("interactive") else ""
            customized = f" {DIM}(customized){RESET}" if name in user_tasks else ""
            lines.append(f"  {BOLD}{name:<14}{RESET} {DIM}{desc:<36}{RESET}{badge}{customized}")
        lines.append("")

    # Custom tasks (user-defined, not overriding builtins)
    custom = [t for t in user_tasks if t not in BUILTIN_DESCRIPTIONS]
    if custom:
        lines.append(f"{GREEN}Custom{RESET}")
        for name in custom:
            info = _get_task_info(repo_root, name)
            desc = info.get("produces", "")[:36] if info.get("produces") else ""
            badge = f"  {YELLOW}interactive{RESET}" if info.get("interactive") else ""
            lines.append(f"  {BOLD}{name:<14}{RESET} {DIM}{desc:<36}{RESET}{badge}")
        lines.append("")

    # Footer
    lines.append(f"{DIM}Built-ins work anywhere. Run lf <task> or lf <task>: args{RESET}")

    return "\n".join(lines)
```

## Non-repo robustness

Current state (already working):
- `gather_diff()` returns `None` outside git
- `gather_diff_files()` returns `[]` outside git
- `gather_task(None, name)` falls back to builtins
- `list_all_tasks(None)` returns `([], builtins)`

Changes needed:

1. **Remove pipeline git requirement** (`cli/run.py:602`):
```python
# Before
if not repo_root:
    typer.echo("Error: Pipelines require a git repository", err=True)
    raise typer.Exit(1)

# After: just use cwd, pipelines are sequential task execution
if not repo_root:
    repo_root = Path.cwd()
```

2. **Context outside repo** - already works:
   - `repo_root = Path.cwd()` fallback exists
   - `gather_docs(cwd, cwd)` collects `.md` files at cwd level
   - No git operations attempted when diff flags are false

## Constraints

- Colors must degrade gracefully (check `sys.stdout.isatty()`)
- `--list` should work identically inside and outside repos (just shows different tasks)
- No breaking change to `lf <task>` behavior

## Done when

```bash
# Outside any git repo
cd /tmp
lf --list              # Shows built-in tasks with colors
lf                     # Launches interactive claude
lf design              # Runs built-in design task

# Inside repo
cd ~/src/loopflow
lf --list              # Shows pipelines + categorized tasks + custom
lf                     # Launches interactive claude with repo docs
lf design              # Runs (possibly customized) design task
```

All commands work without errors. Colors display in terminal, plain text in pipes.
