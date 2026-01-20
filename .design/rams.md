# RAMS Integration

Support running RAMS (and other globally-installed commands) via loopflow.

## What to build

Discover tasks from `~/.claude/commands/` so `lf rams` works after installing RAMS.

## Background

**RAMS** ([rams.ai](https://rams.ai)) is a design review tool that checks for accessibility issues, visual inconsistencies, and UI polish. It installs as a single prompt file:

```bash
curl -fsSL https://rams.ai/install | bash
# Installs ~/.claude/commands/rams.md
```

After installation, `/rams` works in Claude Code. But `lf rams` doesn't find it because loopflow only searches:
1. External skills (`sp:brainstorm`)
2. `.claude/commands/` (repo-local)
3. `.lf/` (repo-local)
4. Builtins

## Approach

Add `~/.claude/commands/` to the task search path. RAMS is a single command (not a skill library), so it fits naturally as a "global task" rather than needing a `rams:` prefix.

## Data structures

```python
# In context.py

# Global command location
_GLOBAL_COMMANDS_PATH = Path.home() / ".claude" / "commands"


def list_global_tasks() -> list[str]:
    """Return names of globally-installed tasks."""
    if not _GLOBAL_COMMANDS_PATH.exists():
        return []
    return sorted(p.stem for p in _GLOBAL_COMMANDS_PATH.glob("*.md"))
```

## Key functions

```python
def gather_task(repo_root: Path | None, name: str, config=None) -> TaskFile | None:
    """Search order:
    1. External skills (prefix:name)
    2. .claude/commands/{name}.md (repo-local)
    3. .lf/{name}.md (repo-local)
    4. ~/.claude/commands/{name}.md (global)  # NEW
    5. Builtins
    """
```

```python
def list_all_tasks(...) -> tuple[list[str], list[str], list[str], list[tuple[str, str]]]:
    """Return (user_tasks, global_tasks, builtin_only_tasks, external_skills)."""
```

## UI changes

`lf --list` shows global tasks in a new section:

```
Global tasks:
  rams
```

## Constraints

- **Precedence:** Repo-local overrides global. Global overrides builtins.
- **No config required.** Auto-detect like we auto-detect `~/.superpowers`.
- **Context included.** Global tasks get loopflow context (docs, diff, files)—that's the whole point.

## Done when

```bash
# Install RAMS
curl -fsSL https://rams.ai/install | bash

# Works
lf rams src/App.tsx

# Listed
lf --list | grep -A1 "Global"
# Global tasks:
#   rams

# Precedence: repo-local wins
echo "# Custom" > .claude/commands/rams.md
lf rams  # uses repo-local, not global
```
