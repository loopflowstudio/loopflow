# RAMS Integration

Support running RAMS (and other globally-installed commands) via loopflow.

## What to build

Add `~/.claude/commands/` to the task search path so `lf rams` works after installing RAMS.

## Data structures

```python
# In context.py

# Global command locations to check (in order)
_GLOBAL_COMMAND_PATHS = [
    Path.home() / ".claude" / "commands",
    # Future: ~/.cursor/commands, ~/.codex/prompts, etc.
]


def list_global_tasks() -> list[str]:
    """Return names of globally-installed tasks."""
    tasks = set()
    for global_dir in _GLOBAL_COMMAND_PATHS:
        if global_dir.exists():
            for p in global_dir.glob("*.md"):
                tasks.add(p.stem)
    return sorted(tasks)
```

## Key functions

### `gather_task()` in context.py

Update search order (currently lines 203-240):

```python
def gather_task(repo_root: Path | None, name: str, config=None) -> TaskFile | None:
    """Search order:
    1. External skills (prefix:name format)
    2. .claude/commands/{name}.md (repo-local)
    3. .lf/{name}.md (repo-local)
    4. ~/.claude/commands/{name}.md (global)  # NEW
    5. templates/commands/{name}.md (builtin)
    """
```

Add after the `.lf/` check (around line 232) and before the builtin fallback:

```python
# Check global user commands
for global_dir in _GLOBAL_COMMAND_PATHS:
    content = _read_file_if_named(global_dir, f"{name}.md")
    if content:
        return parse_task_file(name, content)
```

### `list_all_tasks()` in context.py

Change return type from 3-tuple to 4-tuple (line 186):

```python
def list_all_tasks(
    repo_root: Path | None,
    config=None
) -> tuple[list[str], list[str], list[str], list[tuple[str, str]]]:
    """Return (user_tasks, global_tasks, builtin_only_tasks, external_skills)."""
    builtins = set(list_builtin_tasks())
    user = set(list_user_tasks(repo_root)) if repo_root else set()
    global_tasks = set(list_global_tasks())

    # Global tasks not overridden by repo-local
    global_only = global_tasks - user
    # Builtins not overridden by user or global
    builtin_only = builtins - user - global_tasks

    sources = discover_skill_sources(config.skill_sources if config else None, repo_root)
    external_skills = list_all_skills(sources)

    return sorted(user), sorted(global_only), sorted(builtin_only), external_skills
```

### `_format_task_list()` in __init__.py

Update call to unpack 4 values (line 140):

```python
user_tasks, global_tasks, builtin_only, external_skills = list_all_tasks(repo_root, config)
```

Add "Global" section after "Custom" and before "External Skills" (around line 186):

```python
# Global tasks section (e.g., ~/.claude/commands/rams.md)
if global_tasks:
    lines.append(f"{c['green']}Global{c['reset']}")
    for name in sorted(global_tasks):
        info = _get_task_info(repo_root, name)
        desc = ""
        if info.get("produces"):
            desc = str(info["produces"])[:34]
        badge = f"  {c['yellow']}interactive{c['reset']}" if info.get("interactive") else ""
        lines.append(f"  {c['bold']}{name:<14}{c['reset']} {c['dim']}{desc:<34}{c['reset']}{badge}")
    lines.append("")
```

### `_get_task_source()` in __init__.py

Add global check (around line 86):

```python
def _get_task_source(repo_root: Path | None, name: str) -> str:
    """Return source location: .claude, .lf, global, or builtin."""
    if repo_root:
        if (repo_root / ".claude" / "commands" / f"{name}.md").exists():
            return ".claude"
        lf_dir = repo_root / ".lf"
        if lf_dir.exists():
            for p in lf_dir.iterdir():
                if p.is_file() and (p.stem == name or p.name == name):
                    return ".lf"
    # Check global
    for global_dir in _GLOBAL_COMMAND_PATHS:
        if (global_dir / f"{name}.md").exists():
            return "global"
    return "builtin"
```

This requires importing `_GLOBAL_COMMAND_PATHS` from context.py:

```python
from loopflow.lf.context import find_worktree_root, gather_task, list_all_tasks, _get_builtin_task, _GLOBAL_COMMAND_PATHS
```

### `_get_task_info()` in __init__.py

Add global fallback (around line 119):

```python
# Check global commands
if content is None:
    for global_dir in _GLOBAL_COMMAND_PATHS:
        global_path = global_dir / f"{name}.md"
        if global_path.exists():
            content = global_path.read_text()
            break
```

## UI changes

`lf --list` output shows global tasks:

```
TASKS

Planning & Design
  design         Plan what to build                    interactive
...

Custom
  my-task        My custom task

Global
  rams           Design review for accessibility

EXTERNAL SKILLS

superpowers
  sp:brainstorm
```

## Constraints

- **Precedence matters.** Repo-local > global > builtins. Same name in repo overrides global.
- **No config required.** Auto-detect `~/.claude/commands/` like we auto-detect `~/.superpowers`.
- **Full context.** Global tasks get loopflow context (docs, diff, files)—the whole point.
- **Single-file format.** Global commands are `.md` files, not skill directories.

## Done when

```bash
# Setup test
mkdir -p ~/.claude/commands
echo "# RAMS test" > ~/.claude/commands/rams.md

# Verify discovery
lf rams
# → Runs with loopflow context

# Verify listing
lf --list | grep -A2 "Global"
# Global
#   rams

# Verify precedence
mkdir -p .claude/commands
echo "# Local rams" > .claude/commands/rams.md
lf --list | grep rams
# → Shows under user tasks, not global

# Cleanup
rm .claude/commands/rams.md
rm ~/.claude/commands/rams.md
```

## Files to modify

| File | Changes |
|------|---------|
| `src/loopflow/lf/context.py` | Add `_GLOBAL_COMMAND_PATHS`, `list_global_tasks()`, update `gather_task()`, update `list_all_tasks()` |
| `src/loopflow/lf/__init__.py` | Import `_GLOBAL_COMMAND_PATHS`, update `_format_task_list()`, `_get_task_source()`, `_get_task_info()` |
