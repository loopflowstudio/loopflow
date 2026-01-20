# lfops land: Squash-Merge to Main

## Overview

`lfops land` squash-merges a loop's personal-main branch to main, clearing the outstanding count and allowing the loop to resume.

## Usage

```bash
lfops land <loop-id>
lfops land <loop-id> --no-delete    # Keep personal-main after landing
lfops land <loop-id> --dry-run      # Show what would happen
```

## Behavior

1. **Validate** — Loop exists, personal-main has commits ahead of main
2. **Fetch** — Get latest main and personal-main from origin
3. **Check mergeable** — Ensure no conflicts
4. **Squash-merge** — Create single commit on main with combined changes
5. **Push** — Push main to origin
6. **Reset personal-main** — Point personal-main to new main HEAD
7. **Resume** — If loop was WAITING, restart it

### Why Squash?

- Clean main history (one commit per landing)
- Easy to revert entire feature set
- Hides iteration noise from main
- Consistent with "ship when ready" workflow

## Implementation

```python
@app.command()
def land(
    loop_id: str = typer.Argument(..., help="Loop ID"),
    no_delete: bool = typer.Option(False, "--no-delete", help="Keep personal-main"),
    dry_run: bool = typer.Option(False, "--dry-run", help="Show what would happen"),
):
    """Squash-merge personal-main to main."""
    loop = get_loop(loop_id)
    if not loop:
        typer.echo(f"Loop not found: {loop_id}", err=True)
        raise typer.Exit(1)

    # Check outstanding
    outstanding = count_outstanding(loop)
    if outstanding == 0:
        typer.echo("Nothing to land (personal-main is even with main)")
        raise typer.Exit(0)

    if dry_run:
        typer.echo(f"Would land {outstanding} commits from {loop.personal_main} to main")
        raise typer.Exit(0)

    # Fetch latest
    subprocess.run(
        ["git", "fetch", "origin", "main", loop.personal_main],
        cwd=loop.repo,
        check=True,
    )

    # Check for conflicts
    merge_base = get_merge_base(loop.repo, "origin/main", f"origin/{loop.personal_main}")
    if has_conflicts(loop.repo, "origin/main", f"origin/{loop.personal_main}"):
        typer.echo("Cannot land: merge conflicts exist", err=True)
        typer.echo(f"Run 'lfops rebase {loop_id}' to resolve conflicts first")
        raise typer.Exit(1)

    # Squash-merge
    result = squash_merge(loop)
    if not result.success:
        typer.echo(f"Land failed: {result.error}", err=True)
        raise typer.Exit(1)

    typer.echo(f"Landed {outstanding} commits to main")
    typer.echo(f"  Commit: {result.commit_sha[:8]}")

    # Resume waiting loop
    if loop.status == LoopStatus.WAITING:
        start_loop_process(loop)
        typer.echo(f"Resumed loop: {loop_id}")
```

### Squash-Merge Logic

```python
def squash_merge(loop: Loop) -> LandResult:
    """Squash personal-main onto main."""
    repo = loop.repo

    # Checkout main
    subprocess.run(["git", "checkout", "main"], cwd=repo, check=True)
    subprocess.run(["git", "pull", "origin", "main"], cwd=repo, check=True)

    # Squash merge
    result = subprocess.run(
        ["git", "merge", "--squash", f"origin/{loop.personal_main}"],
        cwd=repo,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        return LandResult(success=False, error=result.stderr)

    # Commit with summary
    message = generate_land_message(loop)
    subprocess.run(
        ["git", "commit", "-m", message],
        cwd=repo,
        check=True,
    )

    # Push main
    subprocess.run(["git", "push", "origin", "main"], cwd=repo, check=True)

    # Get commit SHA
    commit_sha = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo,
        capture_output=True,
        text=True,
    ).stdout.strip()

    # Reset personal-main to new main
    subprocess.run(
        ["git", "push", "origin", f"main:{loop.personal_main}", "--force"],
        cwd=repo,
        check=True,
    )

    return LandResult(success=True, commit_sha=commit_sha)
```

### Commit Message

```python
def generate_land_message(loop: Loop) -> str:
    """Generate squash commit message."""
    outstanding = count_outstanding(loop)

    # Get iteration range
    runs = get_loop_runs(loop.id, limit=outstanding)
    first_iter = runs[-1].iteration if runs else 1
    last_iter = runs[0].iteration if runs else outstanding

    return f"""{loop.goal}: land iterations {first_iter}-{last_iter}

Squash-merged {outstanding} iterations from {loop.personal_main}.

Loop: {loop.id}
Goal: {loop.goal}
Iterations: {first_iter}-{last_iter}
"""
```

## lfops rebase

Companion command to keep personal-main current with main.

```bash
lfops rebase <loop-id>
```

### Behavior

1. Fetch latest main
2. Rebase personal-main onto main
3. Force-push personal-main
4. Handle conflicts interactively if needed

```python
@app.command()
def rebase(loop_id: str = typer.Argument(..., help="Loop ID")):
    """Rebase personal-main onto main."""
    loop = get_loop(loop_id)
    if not loop:
        typer.echo(f"Loop not found: {loop_id}", err=True)
        raise typer.Exit(1)

    # Stop loop if running
    if loop.status == LoopStatus.RUNNING:
        stop_loop(loop_id)
        typer.echo(f"Stopped running loop")

    # Fetch and rebase
    subprocess.run(
        ["git", "fetch", "origin", "main"],
        cwd=loop.repo,
        check=True,
    )

    subprocess.run(
        ["git", "checkout", loop.personal_main],
        cwd=loop.repo,
        check=True,
    )

    result = subprocess.run(
        ["git", "rebase", "origin/main"],
        cwd=loop.repo,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        typer.echo("Rebase has conflicts. Resolve manually:")
        typer.echo("  git status")
        typer.echo("  # fix conflicts")
        typer.echo("  git add <files>")
        typer.echo("  git rebase --continue")
        typer.echo(f"  git push origin {loop.personal_main} --force")
        raise typer.Exit(1)

    # Push rebased branch
    subprocess.run(
        ["git", "push", "origin", loop.personal_main, "--force"],
        cwd=loop.repo,
        check=True,
    )

    typer.echo(f"Rebased {loop.personal_main} onto main")
```

## Edge Cases

### Conflicts during land

If squash-merge has conflicts:
- `lfops land` fails with instructions
- User runs `lfops rebase` first
- Then retries `lfops land`

### Loop running during land

- `lfops land` should warn if loop is RUNNING
- Optional: `--wait` flag to wait for current iteration
- Optional: `--force` to stop and land immediately

### Personal-main behind main

This happens if main moved forward (other PRs landed).

```
main:           A ← B ← C ← D ← E
                         ↑
personal-main:  A ← B ← C ← X ← Y
```

Land will fail due to divergence. Solution:
1. Run `lfops rebase` first
2. Then `lfops land`

### Nothing to land

If personal-main equals main (outstanding = 0):
- Exit cleanly with message
- No error, just nothing to do

### Multiple loops on same repo

Each loop has its own personal-main branch.
Landing one doesn't affect others.

## Events

```python
"lfops.land.started"   # {"loop_id": str, "outstanding": int}
"lfops.land.completed" # {"loop_id": str, "commit_sha": str}
"lfops.land.failed"    # {"loop_id": str, "error": str}
"lfops.rebase.started" # {"loop_id": str}
"lfops.rebase.completed" # {"loop_id": str}
"lfops.rebase.conflict" # {"loop_id": str}
```

## Workflow Example

```bash
# Start a loop
$ lfd loop test-coverage
Started loop: test-coverage (PID 12345)

# Loop runs iterations, creating PRs
# ...eventually hits PR limit

$ lfd status abc123
Loop: abc123
  Status: waiting
  Outstanding: 5/5

# Review the changes
$ git log origin/main..origin/test-coverage-main --oneline
abc1234 test-coverage/005: Add tests for auth module
def5678 test-coverage/004: Add tests for api module
...

# Land to main
$ lfops land abc123
Landed 5 commits to main
  Commit: 9876fedc
Resumed loop: abc123

# Loop continues
$ lfd status abc123
Loop: abc123
  Status: running
  Outstanding: 0/5
```
