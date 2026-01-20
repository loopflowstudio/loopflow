# agents1

Background loop execution for `lfd`.

## Overview

Four loop types with different triggering and termination:

| Type | Triggers | Stops When |
|------|----------|------------|
| LOOP | Manual start | `pr_limit` reached or stopped |
| FLOW | Manual start | Single iteration completes |
| SUBSCRIBE | File changes on main | Iteration completes, waits for next change |
| SCHEDULE | Cron expression | Iteration completes, waits for next trigger |

All types share the same iteration logic: create worktree, run pipeline, PR to loop-main.

## Examples

```bash
# Continuous loop - runs until pr_limit
lfd loop test-coverage

# One-off flow - runs until project is done
lfd flow api-refactor -p .design/api-refactor.md

# Subscribe - runs when src/api/ changes on main
lfd subscribe src/api/ api-tests

# Schedule - runs every weekday at 9am
lfd schedule "0 9 * * 1-5" security-scan
```

## Data Model

### LoopType

```python
class LoopType(Enum):
    LOOP = "loop"          # Continuous until pr_limit
    FLOW = "flow"          # Until "done when" passes
    SUBSCRIBE = "subscribe"  # On file changes
    SCHEDULE = "schedule"    # On cron trigger
```

### Loop (database)

```python
@dataclass
class Loop:
    id: str
    type: LoopType
    goal: str                # References .lf/goals/{name}.md
    repo: Path
    loop_main: str           # Branch like "test-coverage-main"

    merge_mode: MergeMode    # PR or LAND
    pr_limit: int            # Max outstanding before WAITING

    status: LoopStatus       # IDLE → RUNNING → WAITING/ERROR
    iteration: int

    # Type-specific config
    project_file: str | None   # for FLOW: path to project/design doc
    pathset: str | None        # for SUBSCRIBE: comma-separated paths
    cron: str | None           # for SCHEDULE: cron expression
    area: str | None           # area override

    pid: int | None
    last_main_sha: str | None  # for SUBSCRIBE: last seen main SHA
    created_at: datetime
```

### MergeMode

```python
class MergeMode(Enum):
    PR = "pr"      # Accumulate on loop-main, human reviews and lands
    LAND = "land"  # Auto-land to main after each iteration
```

---

## What Works Now

| Feature | Status |
|---------|--------|
| `lfd loop <goal>` | ✓ Background subprocess, PR limit, WAITING |
| `lfd status` | ✓ Shows all loops |
| `lfd stop <id>` | ✓ SIGTERM/SIGKILL |
| `lfd start` | ✓ Batch-start multiple loops |
| `lfd list-goals` | ✓ Shows available goals |
| `lfops land --squash` | ✓ Squash-merge loop-main to main |
| Goal loading | ✓ Frontmatter parsing |
| Branch allocation | ✓ `{goal}-main`, `{goal}-1-main` |
| Outstanding counting | ✓ `git rev-list --count` |

---

## Feature 1: LAND Merge Mode

### What to build

After each iteration auto-merges to loop-main, also create/update a PR from loop-main→main and enable auto-merge.

### Current state

```python
# loop_runner.py:298-300
if loop.merge_mode.value == "auto":
    _auto_merge_pr(worktree_path)
```

Only handles AUTO mode. LAND is not implemented.

### Key functions

```python
# loop_runner.py

def _land_to_main(loop: Loop) -> str | None:
    """Create or update PR from loop-main → main, enable auto-merge.

    Returns PR URL on success, None on failure.
    Works from main repo (not worktree, which gets deleted).
    """
    repo = loop.repo

    # Push loop-main
    subprocess.run(["git", "push", "origin", loop.loop_main], cwd=repo)

    # Check for existing PR
    result = subprocess.run(
        ["gh", "pr", "list", "--head", loop.loop_main, "--base", "main",
         "--json", "number,url", "--state", "open"],
        cwd=repo, capture_output=True, text=True,
    )
    existing = json.loads(result.stdout) if result.returncode == 0 else []

    if existing:
        # PR exists - ensure auto-merge is enabled
        pr_number = existing[0]["number"]
        subprocess.run(
            ["gh", "pr", "merge", str(pr_number), "--squash", "--auto"],
            cwd=repo, capture_output=True,
        )
        return existing[0]["url"]

    # Create new PR
    result = subprocess.run(
        ["gh", "pr", "create", "--base", "main", "--head", loop.loop_main,
         "--title", f"[{loop.goal}] Land accumulated work",
         "--body", f"Auto-land from loop: {loop.goal}"],
        cwd=repo, capture_output=True, text=True,
    )
    if result.returncode != 0:
        return None

    # Enable auto-merge
    subprocess.run(["gh", "pr", "merge", "--squash", "--auto"], cwd=repo)
    return result.stdout.strip()
```

### Integration point

```python
# loop_runner.py run_iteration()

# After iteration succeeds and PR is created to loop-main:
if loop.merge_mode != MergeMode.PR:
    _auto_merge_pr(worktree_path)

if loop.merge_mode == MergeMode.LAND:
    _land_to_main(loop)
```

### Constraints

- CI must pass before PR merges (auto-merge waits)
- If `_land_to_main` fails, log warning but don't fail iteration
- Idempotent: existing PR just gets auto-merge re-enabled

### Done when

```bash
lfd loop test-goal --merge-mode land
# After iteration completes:
gh pr list --head test-goal-main --base main --state open
# Shows PR with auto-merge enabled
```

---

## Feature 2: `lfd flow` - One-Shot Execution

### What to build

Run a goal exactly once with an optional one-off prompt. Unlike LOOP which runs continuously, FLOW completes after a single iteration.

### Concept

A flow is a single run of a goal + pipeline, optionally with additional context/prompt:

```bash
# Just run the goal once
lfd flow api-refactor

# With a one-off prompt from file
lfd flow api-refactor -p .design/api-refactor.md

# With prompt from clipboard
lfd flow api-refactor -v

# Maestro: type in chat box, run as flow
```

The one-off prompt augments the goal's instructions for this specific run. Sources:
- File (`-p` or `--prompt`)
- Clipboard (`-v`)
- Maestro chat input
- Stdin

### Key differences from LOOP

| | LOOP | FLOW |
|---|---|---|
| Iterations | Until `pr_limit` | Exactly 1 |
| Restarts | Auto-continues | Manual re-run |
| Use case | Continuous improvement | Specific one-off task |

### Key functions

```python
# In loop_runner.py or loops.py

def run_flow(loop: Loop) -> bool:
    """Run a single iteration of the flow. Returns success."""
    # Run exactly one iteration
    success = run_iteration(loop, loop.iteration + 1)

    if success:
        update_loop_iteration(loop.id, loop.iteration + 1)
        update_loop_status(loop.id, LoopStatus.IDLE)
    else:
        update_loop_status(loop.id, LoopStatus.ERROR)

    # Clear pid - flow is done
    update_loop_pid(loop.id, None)
    return success
```

### Prompt injection

The one-off prompt gets injected alongside the goal content:

```python
# In loop_runner.py run_iteration()

# Inject goal content
goal_section = f"<lf:goal:{loop.goal}>\n{goal_spec.content}\n</lf:goal:{loop.goal}>"

# If flow has a prompt file, add it
if loop.type == LoopType.FLOW and loop.project_file:
    prompt_content = Path(loop.project_file).read_text()
    goal_section += f"\n\n<lf:prompt>\n{prompt_content}\n</lf:prompt>"
```

### CLI interface

```bash
lfd flow security-audit                     # Goal only
lfd flow security-audit -p task.md          # With prompt file
lfd flow security-audit -v                  # With clipboard
lfd flow security-audit --prompt "focus on auth"  # Inline prompt
```

### Constraints

- Runs exactly once, then IDLE
- Prompt sources: file, clipboard, inline, stdin
- No "done when" evaluation - just run and finish
- Still creates worktree, PRs to loop-main like other types

### Done when

```bash
lfd flow test-goal -p .design/task.md
lfd status
# Shows: test-goal  flow  IDLE  iteration=1
```

---

## Feature 3: `lfd subscribe` - File Watcher

### What to build

Trigger an iteration when specific files change on main.

### Concept

```bash
lfd subscribe src/api/ api-tests
```

When commits land on main that modify files under `src/api/`, trigger the `api-tests` goal.

### Tracking changes

Store `last_main_sha` on the Loop. On each poll:

1. `git fetch origin main`
2. Check if `origin/main` differs from `last_main_sha`
3. If yes, check if changes touch the pathset
4. If yes, trigger iteration

### Key functions

```python
# subscribe.py (new)

def check_subscription(loop: Loop) -> bool:
    """Check if subscription should trigger. Returns True if triggered."""
    repo = loop.repo

    # Fetch main
    subprocess.run(["git", "fetch", "origin", "main"], cwd=repo, capture_output=True)

    # Get current main SHA
    result = subprocess.run(
        ["git", "rev-parse", "origin/main"],
        cwd=repo, capture_output=True, text=True,
    )
    current_sha = result.stdout.strip()

    if current_sha == loop.last_main_sha:
        return False  # No change

    if loop.last_main_sha is None:
        # First run - set baseline, don't trigger
        update_loop_last_sha(loop.id, current_sha)
        return False

    # Check if pathset was modified
    paths = [p.strip() for p in loop.pathset.split(",")]
    result = subprocess.run(
        ["git", "diff", "--name-only", loop.last_main_sha, current_sha, "--"] + paths,
        cwd=repo, capture_output=True, text=True,
    )

    changed_files = result.stdout.strip()
    if not changed_files:
        # Main changed but not our paths
        update_loop_last_sha(loop.id, current_sha)
        return False

    # Trigger iteration
    update_loop_last_sha(loop.id, current_sha)
    return True


def run_subscription_check() -> None:
    """Check all subscriptions and trigger as needed. Called by daemon."""
    for loop in list_loops():
        if loop.type != LoopType.SUBSCRIBE:
            continue
        if loop.status == LoopStatus.RUNNING:
            continue  # Already running

        if check_subscription(loop):
            start_loop(loop.id)
```

### Database change

Add `last_main_sha` column to loops table:

```sql
ALTER TABLE loops ADD COLUMN last_main_sha TEXT;
```

Update `Loop` model and db functions.

### Daemon integration

The daemon's periodic check (currently every 30s) should call `run_subscription_check()`.

```python
# server.py _periodic_check()
async def _periodic_check(self) -> None:
    while self._running:
        await asyncio.sleep(30)
        update_dead_runs()
        await check_and_run_triggers()
        run_subscription_check()  # NEW
```

### CLI interface

```bash
lfd subscribe src/api/,src/models/ api-sync
lfd subscribe "*.proto" proto-gen
```

### Constraints

- Polls every 30s (configurable)
- Won't re-trigger while iteration is running
- Pathset uses git pathspec syntax
- First registration sets baseline SHA (no immediate trigger)

### Done when

```bash
lfd subscribe src/ test-goal
# Modify a file and push to main
git commit --allow-empty -m "trigger" && git push
# Within 30s, loop status shows RUNNING
lfd status
```

---

## Feature 4: `lfd schedule` - Cron Trigger

### What to build

Trigger iterations on a cron schedule.

### Concept

```bash
lfd schedule "0 9 * * 1-5" security-scan
```

Every weekday at 9am, trigger the `security-scan` goal.

### Cron evaluation

Use `croniter` library to evaluate cron expressions:

```python
from croniter import croniter
from datetime import datetime

def should_trigger_cron(cron_expr: str, last_run: datetime | None) -> bool:
    """Check if cron should trigger based on last run time."""
    now = datetime.now()
    cron = croniter(cron_expr, now)

    # Get previous scheduled time
    prev_time = cron.get_prev(datetime)

    if last_run is None:
        # First check - trigger if we're past the scheduled time
        return True

    # Trigger if prev_time is after last_run
    return prev_time > last_run
```

### Key functions

```python
# schedule.py (new)

def check_schedule(loop: Loop) -> bool:
    """Check if scheduled loop should trigger. Returns True if triggered."""
    if not loop.cron:
        return False

    # Get last completed run
    last_run = get_latest_loop_run(loop.id)
    last_time = last_run.ended_at if last_run else None

    return should_trigger_cron(loop.cron, last_time)


def run_schedule_check() -> None:
    """Check all schedules and trigger as needed. Called by daemon."""
    for loop in list_loops():
        if loop.type != LoopType.SCHEDULE:
            continue
        if loop.status == LoopStatus.RUNNING:
            continue

        if check_schedule(loop):
            start_loop(loop.id)
```

### Dependencies

Add `croniter` to pyproject.toml:

```toml
dependencies = [
    ...
    "croniter>=2.0",
]
```

### Daemon integration

Same as subscribe - add to periodic check.

### CLI interface

```bash
lfd schedule "0 9 * * *" daily-tests           # Every day at 9am
lfd schedule "0 */4 * * *" health-check        # Every 4 hours
lfd schedule "0 0 * * 0" weekly-cleanup        # Sundays at midnight
```

### Constraints

- Cron expressions use standard 5-field format
- Checks happen every 30s (may miss exact trigger time by up to 30s)
- Won't re-trigger while running
- Uses local timezone

### Done when

```bash
lfd schedule "* * * * *" test-goal  # Every minute (for testing)
# Wait up to 90s
lfd status  # Shows RUNNING or recent run
```

---

## Implementation Tasks (In Progress)

### 1. Delete legacy code

- [ ] Delete `src/loopflow/lf/loop.py`
- [ ] Remove registration from `src/loopflow/lf/__init__.py` (line 68, 78)
- [ ] Delete `src/loopflow/lfd/agents.py` (old AgentSpec system)
- [ ] Delete `src/loopflow/lfd/naming.py` (emoji naming for old system)
- [ ] Delete `src/loopflow/lfd/scheduler.py` (unused)

### 2. Rename fields

- [ ] `goal` → `goal_name` in Loop model
- [ ] `personal_main` → `loop_main` (mostly done, cleanup comments)
- [ ] Update DB schema column name

### 3. Simplify MergeMode

- [ ] Remove `AUTO` value (iteration→loop-main is always auto)
- [ ] Keep only `PR` and `LAND`
- [ ] Update loop_runner to handle LAND mode (auto-land after iteration)

### 4. Fix code issues

- [ ] Move `import sys` to top of `loops.py`
- [ ] Error on missing task file in `loop_runner.py:167`

### 5. Cleanup docs

- [ ] Update `.docs/loops-*.md` with new terminology
- [ ] Remove references to `~/.lf/agents/`

---

## New Features Implementation Order

1. **LAND merge mode** - Smallest change, high value
2. **`lfd flow`** - Simple: run once and stop (already mostly works)
3. **`lfd schedule`** - Cron evaluation, daemon polling
4. **`lfd subscribe`** - Git diff logic, daemon polling

## Files to modify

| File | Changes |
|------|---------|
| `models.py` | Add `last_main_sha` to Loop |
| `db.py` | Add `last_main_sha` column, update functions |
| `loop_runner.py` | Add `_land_to_main()`, fix merge_mode handling, flow prompt injection |
| `loops.py` | Add `update_loop_last_sha()`, `run_flow()` |
| `server.py` | Add subscription/schedule checks to periodic loop |
| `__init__.py` | Add `-v`, `--prompt` flags to flow command |
| `pyproject.toml` | Add `croniter` dependency |

New files:
- `src/loopflow/lfd/subscribe.py` - Subscription checking
- `src/loopflow/lfd/schedule.py` - Cron evaluation

## Verification

```bash
# Run tests
uv run pytest tests/test_lfd.py -v

# Manual verification for each feature
# (see "Done when" sections above)
```

## Terminology

| Term | Meaning |
|------|---------|
| Goal | Prompt file at `.lf/goals/{name}.md` |
| Loop | Runtime config for running a goal repeatedly |
| loop-main | Branch where iterations accumulate |
| Outstanding | Commits on loop-main ahead of main |
| Iteration | One run of the pipeline |
