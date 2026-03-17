# 04: Chord Triggers

**Finish line:** The redesign chord runs tend automatically — after member wave PRs land, on daily cron, and on block escalation. Not just manually invoked.

## Context

Wave triggers exist: repo (paths changed on main), wave (another wave completed), ci_failure. Chords need their own trigger semantics. A chord's tend flow should fire in response to member wave activity, not file changes.

## What to build

1. **Wave-completion trigger.** When a member wave lands a PR (merges to main), the chord's tend flow fires. This is the natural rhythm — build completes, tend observes.

2. **Cron trigger.** Daily tend cycle regardless of wave activity. Catches stalls and drift that wouldn't trigger wave-completion. Uses the existing wave cron machinery — a chord is a wave.

3. **Block escalation trigger.** When a wave encounters a block it can't self-heal, the chord's tend flow fires immediately (not waiting for cron or PR). The chord tries to resolve before escalating to human.

4. **Debounce.** Multiple wave completions in quick succession shouldn't trigger N tend cycles. Batch within a window (e.g., 5 minutes) and run one tend cycle covering all recent changes.

## Done when

- Chord tend fires automatically after member wave PR merge
- Daily cron tend cycle runs
- Block escalation triggers immediate tend
- Debouncing prevents redundant tend cycles
- All triggers logged — visible in chord run history
