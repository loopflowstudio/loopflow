# 01: Bootstrap Wave Configs

**Finish line:** The four redesign waves exist as real wave configs in lfd. The redesign chord exists with all four as members. The chord can be queried — `lfq show redesign-chord` returns member wave state.

## Context

Chord CRUD and membership APIs are built and tested. Wave configs exist as YAML in `wave/`. This item wires the two together: create the chord in lfd, register the four waves, verify the relationship is queryable.

## What to build

1. **Register the four redesign waves** in lfd if not already present. Verify each wave config loads correctly.

2. **Create the redesign chord** via API. Add all four waves as members.

3. **Verify queryability.** `lfq list` shows the waves. The chord API returns members with their current state. This is the foundation everything else builds on.

4. **Document the bootstrap** in scratch/ — what commands were run, what the initial state looks like. This becomes the first memory for the chord's Letta agent later.

## Done when

- `lfq show` for each wave returns its config and status
- Chord API returns all four member waves
- The relationship is persisted (survives lfd restart)
