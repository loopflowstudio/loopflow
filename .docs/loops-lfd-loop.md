# lfd loop: Continuous Homeostasis

## Overview

`lfd loop` starts a continuous improvement loop that runs until the PR limit is hit.

## Usage

```bash
lfd loop <goal>
lfd loop <goal> -a <area>           # override area
lfd loop <goal> --limit <n>         # override PR limit
lfd loop <goal> --flow <flow>       # override flow
```

## Examples

```bash
# Start a test coverage loop
lfd loop test-coverage

# Start with area override
lfd loop api-cleanup -a src/api,src/models

# Higher PR limit
lfd loop refactoring --limit 10
```

## Behavior

1. **Load goal** from `.lf/goals/{goal}.md`
2. **Get or create loop** in database
3. **Check PR limit** — if outstanding >= limit, set status=waiting and exit
4. **Ensure personal-main** exists (create from main if not)
5. **Spawn loop subprocess** that runs iterations

### Loop Subprocess

```
while outstanding < limit:
    1. Create worktree from personal-main
    2. Run flow (design → implement → polish)
    3. Create PR → personal-main
    4. Auto-merge PR (if merge_mode=auto)
    5. Record loop_run in DB
    6. iteration++
    7. Clean up worktree

status = WAITING
```

## Goal Loading

```python
def load_goal(repo: Path, goal_name: str) -> Goal | None:
    """Load goal from .lf/goals/{name}.md"""
    path = repo / ".lf" / "goals" / f"{goal_name}.md"
    if not path.exists():
        return None

    content = path.read_text()
    frontmatter, body = parse_frontmatter(content)

    return Goal(
        name=goal_name,
        area=frontmatter.get("area", []),
        flow=frontmatter.get("flow", "design,implement,polish"),
        content=body,
    )
```

## Personal-Main Branch

Each (goal, repo) pair gets a unique personal-main branch:

```
{goal}-main           # e.g., test-coverage-main
{goal}-1-main         # if first exists
{goal}-2-main         # etc.
```

Created from `origin/main` on first loop start.

## Iteration Workflow

```python
def run_iteration(loop: Loop, iteration: int) -> LoopRun:
    # Create worktree
    branch = f"{loop.goal}/{iteration:03d}"
    worktree = create_worktree(loop.repo, branch, base=loop.personal_main)

    # Run flow
    for task in loop.flow.tasks:
        update_current_step(run.id, task)
        run_task(worktree, task, goal=loop.goal_content, area=loop.area)

    # Create PR
    pr_url = create_pr(worktree, base=loop.personal_main)

    # Auto-merge if configured
    if loop.merge_mode == "auto":
        merge_pr(pr_url)

    # Clean up
    remove_worktree(worktree)

    return LoopRun(
        loop_id=loop.id,
        iteration=iteration,
        status="completed",
        pr_url=pr_url,
    )
```

## Outstanding Count

```python
def count_outstanding(loop: Loop) -> int:
    """Count commits on personal-main ahead of main."""
    result = subprocess.run(
        ["git", "rev-list", "--count", f"main..{loop.personal_main}"],
        cwd=loop.repo,
        capture_output=True,
        text=True,
    )
    return int(result.stdout.strip())
```

## Error Handling

- **Goal not found** — Error immediately, don't create loop
- **Flow task fails** — Set loop status=error, record error in loop_run
- **PR creation fails** — Set loop status=error, leave worktree for debugging
- **Merge fails** — Set loop status=error, PR remains open

## Events

Emitted for UI observability:

```python
"loop.started"        # {"loop_id": str, "goal": str, "iteration": int}
"loop.step.started"   # {"loop_id": str, "step": str}
"loop.step.completed" # {"loop_id": str, "step": str, "status": str}
"loop.iteration.done" # {"loop_id": str, "iteration": int, "pr_url": str}
"loop.waiting"        # {"loop_id": str, "outstanding": int, "limit": int}
"loop.error"          # {"loop_id": str, "error": str}
```

## CLI Implementation

```python
@app.command()
def loop(
    goal: str = typer.Argument(..., help="Goal name"),
    area: str = typer.Option(None, "-a", "--area", help="Area override"),
    limit: int = typer.Option(5, "--limit", help="PR limit"),
    flow: str = typer.Option(None, "--flow", help="Flow override"),
):
    """Start a continuous improvement loop."""
    repo = find_repo_root()

    # Load and validate goal
    goal_spec = load_goal(repo, goal)
    if not goal_spec:
        typer.echo(f"Goal not found: .lf/goals/{goal}.md", err=True)
        raise typer.Exit(1)

    # Get or create loop
    loop = get_or_create_loop(
        goal=goal,
        repo=repo,
        type="loop",
        area=area or goal_spec.area,
        flow=flow or goal_spec.flow,
        pr_limit=limit,
    )

    # Check if already running
    if loop.status == LoopStatus.RUNNING:
        typer.echo(f"Loop already running (PID {loop.pid})")
        raise typer.Exit(1)

    # Check outstanding
    outstanding = count_outstanding(loop)
    if outstanding >= loop.pr_limit:
        update_loop_status(loop.id, LoopStatus.WAITING)
        typer.echo(f"Waiting: {outstanding} outstanding PRs (limit {loop.pr_limit})")
        typer.echo(f"Run 'lfops land {loop.id}' to clear")
        raise typer.Exit(0)

    # Start loop
    start_loop_process(loop)
    typer.echo(f"Started loop: {goal} (PID {loop.pid})")
```
