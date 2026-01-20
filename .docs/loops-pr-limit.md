# PR Limit: Outstanding Commit Counting

## Overview

Loops pause when too many PRs accumulate unreviewed. This prevents runaway automation and keeps the human in the loop.

## The Limit

**Outstanding** = commits on personal-main that haven't been landed to main.

```
main:           A ← B ← C
                         ↑
personal-main:  A ← B ← C ← D ← E ← F
                             └─────┴─────┘
                             3 outstanding
```

When outstanding >= pr_limit (default 5), the loop enters WAITING state.

## Counting Outstanding

```python
def count_outstanding(loop: Loop) -> int:
    """Count commits on personal-main ahead of main."""
    # Ensure we have latest
    subprocess.run(
        ["git", "fetch", "origin", "main", loop.personal_main],
        cwd=loop.repo,
        capture_output=True,
    )

    # Count commits ahead
    result = subprocess.run(
        ["git", "rev-list", "--count", f"origin/main..origin/{loop.personal_main}"],
        cwd=loop.repo,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        return 0  # Branch doesn't exist yet

    return int(result.stdout.strip())
```

## Check Points

Outstanding is checked at:

1. **Loop start** — Before spawning subprocess
2. **Iteration end** — After PR merges to personal-main
3. **Periodic poll** — Every N minutes while running

```python
def should_continue(loop: Loop) -> bool:
    """Check if loop should continue or pause."""
    outstanding = count_outstanding(loop)

    if outstanding >= loop.pr_limit:
        update_loop_status(loop.id, LoopStatus.WAITING)
        notify_event("loop.waiting", {
            "loop_id": loop.id,
            "outstanding": outstanding,
            "limit": loop.pr_limit,
        })
        return False

    return True
```

## WAITING State

When a loop enters WAITING:

1. Current iteration completes (not interrupted mid-task)
2. Loop subprocess exits cleanly
3. Status set to WAITING in database
4. Event emitted for UI notification

```
$ lfd status abc123
Loop: abc123
  Status: waiting
  Outstanding: 5/5

  Run 'lfops land abc123' to merge to main and resume.
```

## Resuming

A loop resumes when outstanding drops below limit. This happens when:

1. **`lfops land`** — Squash-merges personal-main to main, resets count
2. **Manual merge** — User merges personal-main to main themselves
3. **PR closure** — If PRs are closed without merging (reduces count)

### Resume Flow

```python
def maybe_resume_waiting_loops(repo: Path):
    """Check waiting loops and resume if possible."""
    for loop in list_loops(repo=repo, status=LoopStatus.WAITING):
        outstanding = count_outstanding(loop)
        if outstanding < loop.pr_limit:
            start_loop_process(loop)
            notify_event("loop.resumed", {"loop_id": loop.id})
```

This can be triggered by:
- `lfops land` completion
- Periodic daemon check
- Git hook on main update

## Alternative Throttles (Future)

PR limit is the MVP throttle. Future options:

| Throttle | Trigger | Use Case |
|----------|---------|----------|
| PR limit | Outstanding >= N | Default |
| CI failures | N consecutive failures | Quality gate |
| Time-based | Max N iterations/hour | Rate limiting |
| Cost-based | Token spend limit | Budget control |
| Manual hold | User pauses | Review needed |

## Configuration

```python
@dataclass
class Loop:
    pr_limit: int = 5  # Default
    # Future: ci_failure_limit, hourly_rate_limit, etc.
```

Override at start:

```bash
lfd loop test-coverage --limit 10
```

## Edge Cases

### Personal-main doesn't exist yet

First iteration creates personal-main from main. Outstanding = 0.

### Personal-main is behind main

This shouldn't happen normally. If it does:
- Outstanding = 0 (or negative, treated as 0)
- Loop continues
- `lfops rebase` can fix the divergence

### Main moves forward while loop is running

Outstanding count may decrease (commits landed by others). Loop continues.

### Merge conflicts

If personal-main can't auto-merge to main:
- `lfops land` fails with conflict
- User must resolve manually
- Loop stays in WAITING

## Events

```python
"loop.outstanding.changed"  # {"loop_id": str, "count": int, "limit": int}
"loop.waiting"              # {"loop_id": str, "outstanding": int, "limit": int}
"loop.resumed"              # {"loop_id": str}
```
