# Fix `lfpr land` to show PRs as merged

## What to build

Consolidate landing into `lfpr land` with two modes:

1. **`lfpr land`** (default): Requires a PR. Uses `gh pr merge --squash` so GitHub shows it as merged.
2. **`lfpr land --local`**: No PR required. Local squash-merge + push to origin. For quick work that skipped PR workflow.

Config option to set default:
```yaml
# .lf/config.yaml
pr: gh      # default - lfpr land uses GitHub PR merge
pr: local   # lfpr land defaults to --local behavior
```

Also delete the old `lf ops land` and `lf ops pr land` - everything goes through `lfpr land`.

## Root cause (current bug)

Current `lfpr land` does local `git merge --squash` then `git push`, which makes GitHub mark PRs as "closed" not "merged" - GitHub never sees a merge.

## Data structures

Update `Config` dataclass to support `pr` field:

```python
@dataclass
class Config:
    # ... existing fields ...
    pr: str = "gh"  # "gh" or "local"
```

## Key functions

```python
@app.command()
def land(
    add: bool = typer.Option(False, "-a", "--add", help="Commit and push changes first"),
    worktree: str = typer.Option(None, "-w", "--worktree", help="Target worktree by name"),
    local: bool = typer.Option(None, "-l", "--local/--gh", help="Local merge vs GitHub PR merge"),
) -> None:
    """Squash-merge branch to main and clean up.

    Default: uses gh pr merge (requires PR via lfpr create).
    With --local: local merge + push (no PR needed).
    Config: set `pr: local` to default to --local.
    """
    config = load_config(find_main_repo())
    use_local = local if local is not None else (config and config.pr == "local")

    if use_local:
        _land_local(add, worktree)
    else:
        _land_pr(add, worktree)


def _land_pr(add: bool, worktree: str | None) -> None:
    """Land via GitHub PR merge."""
    # 1. Resolve repo_root and main_repo
    # 2. Validate: clean working tree, branch pushed
    # 3. Get PR info (title, body, baseRefName) via gh pr view
    # 4. gh pr merge --squash --delete-branch --subject {title} --body {body}
    # 5. Clear .design artifacts in main_repo
    # 6. git fetch + checkout + pull --ff-only in main_repo
    # 7. Remove worktree if applicable
    ...


def _land_local(add: bool, worktree: str | None) -> None:
    """Land locally without PR."""
    # 1. Resolve repo_root and main_repo
    # 2. Validate: clean working tree
    # 3. Generate commit message from diff (LLM)
    # 4. Squash commits on branch
    # 5. In main_repo: fetch origin/main, checkout main, reset --hard origin/main
    # 6. git merge --squash origin/{branch}
    # 7. Clear .design artifacts
    # 8. git commit, git push
    # 9. Delete remote branch, remove worktree
    ...
```

## Changes required

### 1. Rewrite `_land_pr` in `lfpr.py` to use `gh pr merge`

```python
# Use gh pr merge instead of local merge
merge_cmd = ["gh", "pr", "merge", branch, "--squash", "--delete-branch", "--subject", title]
if body:
    merge_cmd.extend(["--body", body])
result = subprocess.run(merge_cmd, cwd=repo_root, capture_output=True, text=True)
if result.returncode != 0:
    typer.echo(f"Error: {result.stderr.strip() or 'merge failed'}", err=True)
    raise typer.Exit(1)

# Sync main repo
subprocess.run(["git", "fetch", "origin", base_branch], cwd=main_repo, check=True)
subprocess.run(["git", "checkout", base_branch], cwd=main_repo, check=True)
subprocess.run(["git", "pull", "--ff-only"], cwd=main_repo, check=True)
```

### 2. Keep `_land_local` for `--local` mode

The existing `_land_local` function (lines 465-557) is mostly correct. Clean up:
- Remove `--force` and `--no-pr` flags (no longer needed)
- Remove worktrunk dependency - do the merge directly
- Ensure .design cleanup happens

### 3. Simplify the `land` command signature

Remove flags that are no longer relevant:
- `--force` (was for overriding PR warning)
- `--no-pr` (replaced by `--local`)
- `--require-clean-design` (always clean .design on land)
- `--base` (read from PR or default to main)

### 4. Delete dead code

Remove these files/functions:
- `src/loopflow/cli/pr.py` - duplicate of lfpr.py, not used
- `src/loopflow/cli/land.py` - old `lf ops land`, replaced by `lfpr land --local`
- Any `lf ops pr` subcommand registration

### 5. Add `pr` field to Config

In `src/loopflow/config.py`:
```python
@dataclass
class Config:
    # ... existing ...
    pr: str = "gh"  # "gh" or "local"
```

### 6. Update pyproject.toml

Entry points should be:
```toml
[project.scripts]
lf = "loopflow.cli:main"
lfpr = "loopflow.lfpr:main"
lfops = "loopflow.lfops:main"
lfwt = "loopflow.lfwt:main"
```

Remove any `lf ops land` or `lf ops pr` subcommand registrations.

## Constraints

- `lfpr land` (no --local) must use `gh pr merge` - only way GitHub marks as merged
- `--local` mode must not require gh CLI or a PR
- Both modes must work from worktrees
- Both modes must sync main repo after merge
- Always clear .design contents on land

## Done when

```bash
# Test PR mode (default)
wt switch --create test-pr
echo "test" > test.txt && git add -A && git commit -m "test"
git push -u origin test-pr
lfpr create
lfpr land
gh pr view test-pr --json state -q '.state'  # => MERGED

# Test local mode (explicit flag)
wt switch --create test-local
echo "test" > test.txt && git add -A && git commit -m "test"
lfpr land --local  # works without PR

# Test config default
# Set pr: local in .lf/config.yaml
wt switch --create test-config
echo "test" > test.txt && git add -A && git commit -m "test"
lfpr land  # uses local mode because of config
lfpr land --gh  # override config, use GitHub

# Old commands should not exist
lf ops land  # => error: no such command
lf ops pr land  # => error: no such command
```
