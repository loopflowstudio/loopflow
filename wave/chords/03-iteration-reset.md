# 03: Iteration Counter Reset

**Status:** Done (this branch)

**Finish line:** `max_iterations` counts per-cycle, not per-wave-lifetime. A cron wave that completes one QA cycle and starts another gets a fresh counter.

## Solution

Added `cycle_start_iteration` to the Wave struct. When a wave transitions from idle/paused to running, `cycle_start_iteration` is set to the current iteration number. The safety valve (`should_pause_for_max_iterations`) compares cycle-relative iteration count against `max_iterations` instead of the lifetime counter.
