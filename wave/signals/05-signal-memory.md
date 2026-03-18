---
asana_id: '1213717741050188'
linear_id: 71e98eb6-ae12-4ad5-8112-3c5ab9ae9ef7
---
# 05: Signal Memory

**Finish line:** Block resolutions feed back into Letta memory. The chord learns from how blocks were resolved and applies that judgment to future similar situations.

## Context

Every block resolution is a data point. "CI failure on auth code → ci-fix worked" is mechanical. "Stall on chord-model wave → human said 'the work item was too vague, rewrite it with more specifics' → wave unstalled" is judgment. The chord should accumulate this judgment.

This is where chords become more than orchestration. They develop pattern recognition across time. "Last time a wave stalled after three PRs on the same item, narrowing scope worked." "This kind of shallow work tends to resolve when we add a research step."

## What to build

1. **Resolution capture.** When a block resolves, record:
   - Block type and context
   - Resolution method (self-heal, chord, human)
   - What was tried and what worked
   - Human's reasoning (from calibration notes)
   - Outcome — did the resolution actually help? (Checked in next tend cycle)

2. **Pattern storage in Letta.** Resolutions go into recall memory. After N similar resolutions, the chord's assess step can promote a pattern to core memory: "When we see X, try Y."

3. **Pattern application.** The chord's propose step checks Letta for relevant patterns before proposing mutations. "We've seen this before — last time, narrowing area and adding a research step resolved it."

4. **Pattern validation.** Track whether pattern-based proposals succeed. Patterns that lead to good outcomes get reinforced. Patterns that don't get demoted or deleted. The chord's judgment improves over time, not just accumulates.

## Done when

- Block resolutions are captured with context and reasoning
- Patterns emerge in Letta memory after repeated similar resolutions
- Chord propose step references patterns when available
- At least one pattern has been applied from memory to a new situation
- Pattern success/failure tracking exists
