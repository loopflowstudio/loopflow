# 02: Self-Healing Cascade

**Finish line:** When a block occurs, the system tries to unblock itself before involving a human. Wave tries first. If it can't, the chord tries. If neither can, the human sees it.

## Context

```
Block occurs
  → Wave self-heal attempt (ci-fix, rebase, retry)
    → success: log it, keep running
    → failure: escalate to chord
      → Chord resolution attempt (resequence, pause conflicting wave)
        → success: log it, keep running
        → failure: escalate to human via block queue
```

This is the nervous system. Most blocks should resolve without human intervention. The human only sees what genuinely requires judgment.

## What to build

1. **Wave-level self-healing.** Per block type, a self-heal strategy:
   - `ci_failure` → run `ci-fix` flow on the wave's branch
   - `merge_conflict` → run `rebase` flow (integrate-upstream)
   - Others → no wave-level self-heal (escalate immediately)

   Self-heal attempts are logged on the block. Max attempts configurable (default: 2). After max attempts, escalate.

2. **Chord-level resolution.** The chord's tend flow handles escalated blocks:
   - `file_conflict` → resequence waves (pause one, let the other finish)
   - `stall` → assess why — propose direction/area/flow mutation
   - `ci_failure` (after wave self-heal failed) → broader investigation, propose fix or human escalation
   - `shallow_work`, `human_drift`, `capability_gap` → always escalate to human (these are judgment calls)

3. **Escalation path.** Each block tracks its journey: detected → self-heal attempt 1 → attempt 2 → escalated to chord → chord attempt → escalated to human. Full history visible in block detail view.

4. **Resolution feedback.** When a block is resolved (by wave, chord, or human), record what worked. This feeds into Letta memory — "last time we saw this pattern, X worked." The chord gets smarter about self-healing over time.

## Done when

- ci_failure triggers automatic ci-fix attempt before escalating
- merge_conflict triggers automatic rebase before escalating
- Chord handles escalated file_conflict by resequencing
- Full escalation path is visible on each block
- Resolution outcomes are recorded for Letta memory
