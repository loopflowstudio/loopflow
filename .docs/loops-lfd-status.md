# lfd status/stop/prs: Observability

## Overview

Commands to monitor and manage running loops.

## lfd status

Show all loops or details for one.

### Usage

```bash
lfd status                    # All loops
lfd status <loop-id>          # One loop (detailed)
lfd status --repo <path>      # Filter by repo
```

### Output: All Loops

```
ID       TYPE        GOAL           REPO              STATUS    ITER  OUTSTANDING
abc123   loop        test-coverage  ~/myapp           running   3     2/5
def456   loop        api-cleanup    ~/myapp           waiting   7     5/5
ghi789   subscribe   security       ~/myapp           idle      12    0/5
jkl012   schedule    reporter       ~/other           idle      45    1/5
```

### Output: Single Loop

```
Loop: abc123
  Type: loop
  Goal: test-coverage
  Repo: ~/myapp
  Status: running
  Iteration: 3
  Outstanding: 2/5
  Personal-main: test-coverage-main
  PID: 12345

  Current: implement (step 2/3)

  Recent runs:
    #3  running   2m ago     -
    #2  completed 15m ago    https://github.com/user/repo/pull/42
    #1  completed 45m ago    https://github.com/user/repo/pull/41
```

### Implementation

```python
@app.command()
def status(
    loop_id: str = typer.Argument(None, help="Loop ID (optional)"),
    repo: Path = typer.Option(None, "--repo", help="Filter by repo"),
):
    """Show loop status."""
    if loop_id:
        loop = get_loop(loop_id)
        if not loop:
            typer.echo(f"Loop not found: {loop_id}", err=True)
            raise typer.Exit(1)
        print_loop_detail(loop)
    else:
        loops = list_loops(repo=repo)
        print_loops_table(loops)
```

## lfd stop

Stop a running loop or unregister a subscription.

### Usage

```bash
lfd stop <loop-id>
lfd stop <loop-id> --force    # Kill immediately (SIGKILL)
```

### Behavior

1. **If running:** Send SIGTERM, wait for graceful shutdown
2. **If waiting/idle:** Mark as stopped, remove from active
3. **With --force:** Send SIGKILL immediately

### Implementation

```python
@app.command()
def stop(
    loop_id: str = typer.Argument(..., help="Loop ID"),
    force: bool = typer.Option(False, "--force", help="Force kill"),
):
    """Stop a loop."""
    loop = get_loop(loop_id)
    if not loop:
        typer.echo(f"Loop not found: {loop_id}", err=True)
        raise typer.Exit(1)

    if loop.status == LoopStatus.RUNNING and loop.pid:
        signal = 9 if force else 15  # SIGKILL or SIGTERM
        try:
            os.kill(loop.pid, signal)
        except OSError:
            pass

    update_loop_status(loop_id, LoopStatus.IDLE)
    typer.echo(f"Stopped: {loop_id}")
```

## lfd prs

Show PRs created by a loop.

### Usage

```bash
lfd prs <loop-id>
lfd prs <loop-id> --limit 20  # More results
lfd prs <loop-id> --all       # Include closed/merged
```

### Output

```
Loop: abc123 (test-coverage)

#   ITERATION  STATUS   CREATED     URL
1   3          open     2m ago      https://github.com/user/repo/pull/44
2   2          merged   15m ago     https://github.com/user/repo/pull/42
3   1          merged   45m ago     https://github.com/user/repo/pull/41
```

### Implementation

```python
@app.command()
def prs(
    loop_id: str = typer.Argument(..., help="Loop ID"),
    limit: int = typer.Option(10, "--limit", "-n", help="Number of PRs"),
    all: bool = typer.Option(False, "--all", help="Include closed/merged"),
):
    """Show PRs for a loop."""
    loop = get_loop(loop_id)
    if not loop:
        typer.echo(f"Loop not found: {loop_id}", err=True)
        raise typer.Exit(1)

    runs = get_loop_runs(loop_id, limit=limit, include_closed=all)
    print_prs_table(loop, runs)
```

## lfd list-goals

Show available goals in current repo.

### Usage

```bash
lfd list-goals
```

### Output

```
Goals in ~/myapp/.lf/goals/:

  test-coverage      area: [src/]           flow: @ship
  api-cleanup        area: [src/api]        flow: design,implement,polish
  security           area: [src/auth]       flow: @security-review

3 goals found
```

## Data Structures

```python
@dataclass
class LoopStatus:
    loop: Loop
    current_run: LoopRun | None
    recent_runs: list[LoopRun]
    outstanding: int

def get_loop_status(loop_id: str) -> LoopStatus:
    """Assemble full status for a loop."""
    loop = get_loop(loop_id)
    current = get_current_run(loop_id)
    recent = get_loop_runs(loop_id, limit=5)
    outstanding = count_outstanding(loop)

    return LoopStatus(
        loop=loop,
        current_run=current,
        recent_runs=recent,
        outstanding=outstanding,
    )
```

## Events

Status changes emit events for UI:

```python
"loop.status.changed"  # {"loop_id": str, "old": str, "new": str}
```
