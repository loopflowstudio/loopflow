# Push and PR

Auto-push commits when a branch has an upstream, with optional GitHub PR creation.

> "I'm imagining supporting remote workers one day, and that it may be hard to manage as things scale"

> "I want that to be part of the actual logic of loopflow, not LLM code"

## What to build

Deterministic push/PR behavior: auto-push on commit when branch tracks a remote, plus opt-in PR creation via flag or config.

## Data structures

```python
# In config.py

@dataclass
class Config:
    pipelines: dict[str, Pipeline]
    dangerously_skip_permissions: bool = False
    push: bool = False          # auto-push when upstream exists
    pr: bool = False            # also open PR (implies push)

@dataclass
class Pipeline:
    name: str
    tasks: list[str]
    push: bool | None = None    # override global config
    pr: bool | None = None      # override global config
```

Config file:

```yaml
# .lf/config.yaml

# Global defaults
push: true
pr: false

pipelines:
  ship:
    - implement
    - review
    - draft_commit
    pr: true   # this pipeline opens a PR
```

## APIs

```python
# In git.py (new module)

def has_upstream(repo_root: Path) -> bool:
    """Check if current branch tracks a remote."""
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "@{u}"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def push(repo_root: Path) -> bool:
    """Push current branch to its upstream. Returns success."""
    result = subprocess.run(
        ["git", "push"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def create_and_track_branch(repo_root: Path, name: str) -> bool:
    """Create branch and set up tracking with origin."""
    # git checkout -b {name}
    # git push -u origin {name}
    ...


def open_pr(repo_root: Path, draft: bool = True) -> str | None:
    """Open GitHub PR for current branch. Returns PR URL or None on failure."""
    # Uses gh cli: gh pr create --fill --draft
    # If PR already exists, returns existing URL
    ...
```

Modified autocommit:

```python
# In pipeline.py

def _autocommit(repo_root: Path, task: str, arg: str | None, push: bool = False) -> None:
    """Commit changes, optionally push."""
    # ... existing commit logic ...

    if push and has_upstream(repo_root):
        push(repo_root)
```

## CLI changes

```python
# New flag for branch creation
@app.command()
def run(
    task: str,
    arg: str = None,
    branch: str = typer.Option(None, "-b", "--branch", help="Create and track new branch"),
    # ... existing options ...
):
    if branch:
        create_and_track_branch(repo_root, branch)
    # ... rest of run ...


# Pipeline gets same flag
@app.command()
def pipeline(
    name: str,
    arg: str = None,
    branch: str = typer.Option(None, "-b", "--branch", help="Create and track new branch"),
    pr: bool = typer.Option(None, "--pr", help="Open PR when done"),
    # ... existing options ...
):
    ...
```

## Branch creation options

Three approaches considered:

**Option A: Just `-b` flag**
```bash
lf ship -b feature-x design.md
# Creates feature-x, tracks origin/feature-x, then runs pipeline
```
Simple. Mirrors `git checkout -b`. But couples branch creation to task execution.

**Option B: Separate `lf branch` command**
```bash
lf branch feature-x
lf ship design.md
```
More explicit. But what does `lf branch` do that `git checkout -b` doesn't? Maybe: create + push + track in one step.

**Option C: Both**
```bash
lf branch feature-x          # standalone branch setup
lf ship -b feature-x design.md  # inline branch setup
```
Most flexible. `-b` is sugar for running `lf branch` first.

> "I think maybe we have a flag though like -b foo which creates a new branch named foo and tracks it with origin"

Recommendation: Start with Option A (`-b` flag only). Add `lf branch` later if needed.

## PR behavior

```bash
# Via flag
lf ship --pr design.md

# Via config (per-pipeline)
pipelines:
  ship:
    - implement
    - review
    - draft_commit
    pr: true

# Via config (global default)
pr: true
```

Flag overrides config. PR implies push.

PR creation uses `gh pr create --fill --draft`:
- `--fill` uses commit messages for title/body
- `--draft` because review happens in Cursor before marking ready
- If PR already exists for branch, skip creation (or update? TBD)

## Constraints

- **Deterministic, not LLM.** Push and PR logic lives in Python, not in task files.
- **Requires upstream.** Auto-push only happens if branch already tracks remote. No surprise pushes.
- **`gh` CLI required for PRs.** Fail gracefully if not installed.
- **Draft PRs by default.** The workflow is: run pipeline → review in Cursor → mark ready.

## Done when

```bash
# Setup
git checkout -b test-push-pr
git push -u origin test-push-pr

# Create config
cat > .lf/config.yaml << 'EOF'
push: true
pipelines:
  ship:
    - implement
    - review
    - draft_commit
    pr: true
EOF

# Run pipeline
lf ship dd/some-feature.md

# Should see:
# 1. Each task commits AND pushes
# 2. After final task, draft PR opens
# 3. `gh pr view` shows the PR

# Also test -b flag
git checkout main
lf ship -b new-feature dd/other-feature.md
# Should create new-feature branch, track origin, run pipeline, open PR
```
