---
status: proposed
area: daemon
created_at: 2026-01-20T15:32:00
---

# Loop Metrics: Track What Loops Are Actually Doing

## The Problem

`lfd status` shows loop state (running/idle/waiting) but not:
- How many iterations have run
- What they produced (commits, PRs)
- Why they stopped
- Cost tracking

Users can't tell if loops are productive or spinning.

## Proposed Solution

Add metrics collection to loop execution:

```python
@dataclass
class LoopMetrics:
    iterations: int
    commits_created: int
    prs_opened: int
    prs_landed: int
    errors: int
    last_error: str | None
    started_at: datetime
    last_iteration_at: datetime | None
    estimated_cost_usd: float
```

Expose via `lfd status --metrics`:

```
area       status   iters  commits  PRs   cost    last run
api        running  47     12       3     $2.40   2m ago
tests      waiting  23     8        2     $1.10   (1 outstanding)
```

## Implementation

1. Add `LoopMetrics` to `lfd/models.py`
2. Collect metrics in `loop_runner.py`
3. Store in SQLite alongside loop state
4. Add `--metrics` flag to `lfd status`
5. Consider: `lfd metrics <loop>` for detailed view

## Future

- Daily/weekly summaries
- Cost alerts when loops exceed budget
- Productivity trends over time
