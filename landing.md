# landing: worktree location + PR fixes

## What to build

Move worktrees from nested `.lf/worktrees/` to sibling `.worktrees/` directory, and fix PR workflow to work in branches (not just worktrees) and reliably close PRs.

## Problems being solved

1. **Nested worktrees confuse tools** - agents in `.lf/worktrees/feature/` walk up and find main repo's `pyproject.toml`/`.venv`
2. **PR land doesn't close the PR** - squash-merges locally but never tells GitHub
3. **PR commands only work in worktrees** - should work on any feature branch

## Data structures

No new types. Path calculation changes:

```python
# Old: nested inside repo
def worktree_path(repo_root: Path, branch: str) -> Path:
    return repo_root / ".lf" / "worktrees" / branch

# New: sibling to repo
def worktree_path(repo_root: Path, branch: str) -> Path:
    parent = repo_root.parent
    leaf = repo_root.name
    return parent / ".worktrees" / leaf / branch
```

Example:
```
/Users/jack/src/lf/loopflow/                      # main repo
/Users/jack/src/lf/.worktrees/loopflow/feature/   # worktree
```

## API changes

### git.py

**`create_worktree(repo_root, name)`** - change path calculation:
```python
def create_worktree(repo_root: Path, name: str) -> Path:
    worktree_path = repo_root.parent / ".worktrees" / repo_root.name / name
    # ... rest unchanged
```

**`list_worktrees(repo_root)`** - look in new location:
```python
def list_worktrees(repo_root: Path) -> list[WorktreeInfo]:
    worktrees_dir = repo_root.parent / ".worktrees" / repo_root.name
    # ... rest unchanged
```

**`remove_worktree(repo_root, name)`** - new path:
```python
def remove_worktree(repo_root: Path, name: str) -> bool:
    worktree_path = repo_root.parent / ".worktrees" / repo_root.name / name
    # ... rest unchanged
```

### cli/pr.py

**`land()`** - add PR close after merge:
```python
# After successful push to main
subprocess.run(["gh", "pr", "close", branch, "--delete-branch"], cwd=main_repo)
```

**All PR commands** - work on any feature branch, not just worktrees:
```python
# Change validation from "find worktree" to "not on main"
branch = get_current_branch(repo_root)
if not branch or branch == "main":
    typer.echo("Error: Must be on a feature branch", err=True)
    raise typer.Exit(1)
```

### cli/wt.py

**`_open_ide()`** - update path for finding workspace files.

### meta.py / doctor

**`doctor()`** - check for `.worktrees/` in global gitignore:
```python
def _check_global_gitignore() -> bool:
    """Check if .worktrees/ is in global gitignore."""
    gitignore = Path.home() / ".config" / "git" / "ignore"
    if not gitignore.exists():
        return False
    return ".worktrees" in gitignore.read_text()
```

## Constraints

- **No migration** - old `.lf/worktrees/` directories left as-is; users can manually clean up
- **Global gitignore** - document that users should add `.worktrees/` to `~/.config/git/ignore`
- **Branch detection** - PR commands check branch name, not worktree status

## Done when

1. `lf wt create foo` from `~/src/lf/loopflow` creates `~/src/lf/.worktrees/loopflow/foo/`
2. `lf pr land` closes the GitHub PR after merge
3. `lf pr create` works from a regular feature branch (not in a worktree)
4. `lf meta doctor` suggests adding `.worktrees/` to global gitignore if missing
5. Tests pass: `.venv/bin/pytest tests/ -v`
