# Loop Metrics

**Status:** proposed
**Area:** daemon
**Priority:** medium

## Problem

Loops run in the background, but there's no visibility into their effectiveness. Questions we can't answer today:

- How many iterations did the loop run?
- How long did each iteration take?
- Did it make meaningful changes or spin without progress?
- What's the cost (tokens, API calls)?

Without metrics, we can't tell if loops are working well or wasting resources.

## Proposal

Track basic metrics per loop iteration:

```python
@dataclass
class LoopMetrics:
    iteration: int
    started_at: datetime
    ended_at: datetime
    exit_reason: str  # "completed", "error", "rate_limit", "timeout"
    files_changed: int
    commits_created: int
    # Future: token_count, cost_estimate
```

### Storage

Write metrics to `~/.lf/loops/{area}/metrics.jsonl` (append-only log).

### Display

`lfd status --metrics` shows:
```
Loop: docs-maintenance
  Iterations: 47
  Last run: 2 hours ago (completed)
  Avg duration: 3m 12s
  Files changed: 23 (last 7 days)
```

### Future

- Cost tracking (requires API key introspection)
- Anomaly detection (loop stuck, spinning without progress)
- Dashboards in Maestro

## Success criteria

- Every loop iteration writes a metrics entry
- `lfd status --metrics` shows iteration history
- Can see if a loop is productive or spinning

## Dependencies

- Loop execution infrastructure (exists)
- State file writes (exists)

## Effort

Small: add metrics collection to loop runner, add display command.
