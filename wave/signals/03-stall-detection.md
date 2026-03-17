# 03: Stall Detection

**Finish line:** The system detects when a wave is nominally running but not producing meaningful output. "Running" and "progressing" are distinguished.

## Context

A wave can be active (agent running, consuming tokens) but not making progress. Loops that retry the same failing approach. Agents that explore without committing. Builds that produce code only to revert it next cycle.

Stall detection is inherently heuristic. False positives erode trust. Start conservative and tune based on real data.

## What to build

1. **Activity metrics.** Per wave, track:
   - Time since last successful PR merge
   - Time since last meaningful commit (exclude reverts, formatting)
   - Token spend since last shipped artifact
   - Number of runs since last status change

2. **Stall heuristics.** A wave is potentially stalled when:
   - No PR merged in >N hours while mode is loop/cron (default N: 24)
   - Token spend exceeds threshold without shipped output
   - Last N runs all failed or produced reverted changes
   - Human-configurable thresholds per wave (some waves are naturally slow)

3. **Stall signal.** When heuristics trigger, create a `stall` block. No self-healing — stalls are coordination problems, not mechanical failures. Escalate to chord tend, then human if chord can't diagnose.

4. **Stall context.** The block includes: what the wave attempted, why it didn't ship, how much was spent. The chord (or human) can see the pattern and decide: redirect, simplify the work item, change the approach, or accept that this work is legitimately hard and needs more time.

## Done when

- Activity metrics are tracked per wave
- Stall heuristic fires when a wave is active but not progressing
- Stall blocks include useful diagnostic context
- False positive rate is acceptable (<20% — tune with real data)
- At least one real stall is detected during redesign work
