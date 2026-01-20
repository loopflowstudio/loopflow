# Get Agents Running

Run 5 parallel agent loops for loopflow development.

## What to build

Fix gaps in `lfd` to support multiple loops with the same goal but different areas, then wrapper scripts to manage them.

## The agents

```
product-engineer -a Maestro/
product-engineer -a src/loopflow/
designer -a Maestro/
designer -a src/loopflow/
infra-engineer (whole repo)
```

## Gap 1: Area not part of uniqueness

**Problem**: Can't run two `product-engineer` loops with different areas. Second call just returns the existing loop.

```sql
-- Current: area ignored in uniqueness
UNIQUE(type, goal, repo)

-- Needed: area is part of identity
UNIQUE(type, goal, area, repo)
```

**Files to change**:

`db.py`:
```python
# Schema change - use COALESCE for nullable area
UNIQUE(type, goal, COALESCE(area, ''), repo)

# Lookup change
def get_loop_by_goal_repo(
    loop_type: LoopType, goal: str, repo: Path, area: str | None = None
) -> Loop | None:
    cursor = conn.execute(
        "SELECT * FROM loops WHERE type = ? AND goal = ? AND COALESCE(area, '') = ? AND repo = ?",
        (loop_type.value, goal, area or '', str(repo)),
    )
```

`loops.py`:
```python
def create_loop(..., area: str | None = None) -> Loop:
    existing = get_loop_by_goal_repo(loop_type, goal_name, repo, area)  # Pass area
    ...
    loop_main = _allocate_loop_main(repo, goal_name, area)  # Pass area

def _allocate_loop_main(repo: Path, goal_name: str, area: str | None = None) -> str:
    if area:
        slug = area.rstrip("/").split("/")[-1].lower()
        base = f"{goal_name}-{slug}"
    else:
        base = goal_name
    # ... existing allocation
```

## Gap 2: Status doesn't show area

**Problem**: Can't tell which loop is which when same goal has different areas.

```
# Current output
ID        TYPE       GOAL             STATUS     ITER   REPO
abc123    loop       product-engineer running    3      ~/src/loopflow

# Needed output
ID        TYPE       GOAL                           STATUS     ITER
abc123    loop       product-engineer (Maestro/)    running    3
def456    loop       product-engineer (loopflow/)   running    2
```

`__init__.py`:
```python
def _goal_display(lp: Loop) -> str:
    if lp.area:
        slug = lp.area.rstrip("/").split("/")[-1]
        return f"{lp.goal_name} ({slug}/)"
    return lp.goal_name
```

## Gap 3: No `lfd stop --all`

**Problem**: Have to stop each loop by ID individually.

`__init__.py`:
```python
@app.command()
def stop(
    loop_id: str = typer.Argument(None, help="Loop ID (omit with --all)"),
    all_loops: bool = typer.Option(False, "--all", help="Stop all loops"),
    force: bool = typer.Option(False, "-f", "--force", help="Force kill"),
):
    if all_loops:
        for lp in list_loops():
            if lp.status == LoopStatus.RUNNING:
                stop_loop(lp.id, force=force)
                typer.echo(f"Stopped {_goal_display(lp)}")
        return
    # ... existing single-loop logic
```

## Gap 4: No machine-readable output

**Problem**: Can't script around `lfd status`.

`__init__.py`:
```python
@app.command()
def status(
    loop_id: str = typer.Argument(None),
    json_output: bool = typer.Option(False, "--json", help="JSON output"),
    ids_only: bool = typer.Option(False, "--ids", help="Print IDs only"),
):
    if ids_only:
        for lp in list_loops():
            typer.echo(lp.id)
        return
    # ... existing human output
```

## Wrapper scripts

After gaps are fixed:

### bin/agents-start

```bash
#!/bin/bash
set -e
cd "$(git rev-parse --show-toplevel)"

lfd loop product-engineer -a Maestro/
lfd loop product-engineer -a src/loopflow/
lfd loop designer -a Maestro/
lfd loop designer -a src/loopflow/
lfd loop infra-engineer

lfd status
```

### bin/agents-stop

```bash
#!/bin/bash
lfd stop --all
```

## Done when

```bash
# Can start 5 agents with 3 goals + areas
lfd loop product-engineer -a Maestro/
lfd loop product-engineer -a src/loopflow/
# Both create separate loops (not same loop)

# Status shows area
lfd status
# Shows "product-engineer (Maestro/)" and "product-engineer (loopflow/)"

# Can stop all at once
lfd stop --all
```
