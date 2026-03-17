# 04: Chord-Wave Triggers

**Finish line:** The redesign chord-wave runs `tend` automatically — after member wave work lands, on daily cadence, and on block escalation. Tending is not a purely manual command.

## Context

Wave triggers already exist for repo changes, wave completion, and CI failure. After bootstrap, the missing piece is chord-wave semantics: the redesign chord-wave needs to react to changes in its member waves, not to file diffs in its own directory. Membership is already derived from `area`, so trigger routing should build on that instead of introducing chord-only bookkeeping.

## What to build

1. **Wave-completion trigger.** When a member wave lands work on main, the redesign chord-wave's `tend` flow fires. Build completes; tend observes.

2. **Cron trigger.** Run a daily tend cycle even without recent wave completions so stalls and drift still surface. Use existing cron machinery — a chord-wave is still a wave.

3. **Block-escalation trigger.** When a member wave hits a block it cannot self-heal, fire `tend` immediately so the chord-wave can try to resolve or resequence before escalating to a human.

4. **Debounce.** Multiple member-wave events in a short window should batch into one tend cycle. The redesign chord-wave should see the whole recent change set, not a burst of redundant runs.

## Done when

- The redesign chord-wave tends automatically after member-wave completion
- Daily cron tend cycles run
- Block escalation triggers immediate tending
- Debouncing prevents redundant tend cycles
- Trigger reasons are visible in chord-wave run history
