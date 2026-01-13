# newrepos: Improve New Repository Experience

## What to build

Better first-run experience: when loopflow is run in an uninitialized repo, show helpful guidance instead of a cryptic error.

## Data structures

```python
@dataclass
class InitStatus:
    """What's configured in the current repo."""
    has_lf_dir: bool         # .lf/ exists
    has_config: bool         # .lf/config.yaml exists
    has_commands: bool       # .claude/commands/ has any .md files
    missing_deps: list[str]  # ["claude", "wt"] etc.

def check_init_status(repo_root: Path) -> InitStatus:
    """Check repo initialization without modifying anything."""
    ...
```

## Key functions

```python
# init_check.py (new file)

def check_init_status(repo_root: Path) -> InitStatus:
    """Return what's configured in this repo."""
    lf_dir = repo_root / ".lf"
    commands_dir = repo_root / ".claude" / "commands"

    return InitStatus(
        has_lf_dir=lf_dir.exists(),
        has_config=(lf_dir / "config.yaml").exists(),
        has_commands=any(commands_dir.glob("*.md")) if commands_dir.exists() else False,
        missing_deps=_check_deps(),
    )

def _check_deps() -> list[str]:
    """Return list of missing required dependencies."""
    missing = []
    if not check_claude_available():
        missing.append("claude")
    if not shutil.which("wt"):
        missing.append("wt")
    return missing

def format_init_hint(status: InitStatus, task_name: str) -> str:
    """Format helpful message for uninitialized repo."""
    lines = [f"No task named '{task_name}' found."]

    if not status.has_commands and not status.has_lf_dir:
        lines.append("")
        lines.append("This repo hasn't been set up for loopflow yet.")
        lines.append("Run: lf ops init")
    elif not status.has_commands:
        lines.append("")
        lines.append("No task files found.")
        lines.append(f"Create: .claude/commands/{task_name}.md")

    if status.missing_deps:
        lines.append("")
        lines.append(f"Missing dependencies: {', '.join(status.missing_deps)}")
        lines.append("Run: lf ops install")

    return "\n".join(lines)
```

## Changes to existing code

### cli/__init__.py

Current behavior when task not found (line 73-76):
```python
else:
    typer.echo(f"Error: No task or pipeline named '{name}'", err=True)
    typer.echo(f"  Create task: .claude/commands/{name}.md (recommended)", err=True)
    typer.echo(f"  Or pipeline: add '{name}' to .lf/config.yaml", err=True)
    raise SystemExit(1)
```

New behavior:
```python
else:
    status = check_init_status(repo_root) if repo_root else None
    if status and not status.has_commands and not status.has_lf_dir:
        # Uninitialized repo - suggest init
        typer.echo(f"No task named '{name}' found.", err=True)
        typer.echo("", err=True)
        typer.echo("This repo hasn't been set up for loopflow yet.", err=True)
        typer.echo("Run: lf ops init", err=True)
    else:
        # Initialized but task missing - suggest creating it
        typer.echo(f"No task or pipeline named '{name}'", err=True)
        typer.echo(f"Create: .claude/commands/{name}.md", err=True)
    raise SystemExit(1)
```

### cli/meta.py doctor command

Add check for repo initialization status:
```python
@app.command()
def doctor():
    """Check loopflow dependencies and repo status."""
    all_ok = True

    # Repo status
    repo_root = find_worktree_root()
    if repo_root:
        status = check_init_status(repo_root)
        if status.has_commands:
            typer.echo("✓ task files found")
        else:
            typer.echo("- no task files (run: lf ops init)")
    else:
        typer.echo("- not in a git repo")

    # ... rest of dependency checks unchanged ...
```

## Constraints

1. **No auto-creation** — Never create files without explicit user action. `lf ops init` is the explicit action.

2. **Works without init** — Users who skip init (using inline prompts only) must not see errors. `lf : "fix typo"` should work in any git repo.

3. **Auto mode compatible** — Messages must be useful even in headless mode. No interactive prompts.

## Files to create/modify

- `src/loopflow/init_check.py` (new) — `InitStatus` dataclass and `check_init_status()` function
- `src/loopflow/cli/__init__.py` — Update error message when task not found
- `src/loopflow/cli/meta.py` — Add repo status to `doctor` output

## Done when

```bash
# In a fresh git repo with no .lf/ directory:
cd /tmp && rm -rf test-repo && mkdir test-repo && cd test-repo && git init

# Running a task shows init guidance:
lf review 2>&1 | grep -q "lf ops init"
echo "Exit code check: $?"  # should be 0

# Inline prompts still work without init:
# (would need claude available, but the command should at least parse)
lf : "hello" --help 2>&1 | grep -q "inline"

# Doctor shows repo status:
lf ops doctor 2>&1 | grep -q "task files"

# Cleanup
rm -rf /tmp/test-repo
```
