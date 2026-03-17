# 01: Bootstrap Wave Configs

**Finish line:** The four redesign waves exist as real wave configs in lfd. The redesign chord-wave exists as a regular wave with `area` pointing at those four members. `lfq show redesign` returns its flow and area.

## Context

Wave configs exist as YAML in `wave/`. This item wires them into lfd: register the four member waves, register the redesign chord-wave, verify the relationship is queryable through the wave API.

## What to build

1. **Register the four redesign waves** in lfd if not already present. Verify each wave config loads correctly.

2. **Create the redesign chord-wave** via the normal wave API. Membership lives in `wave/redesign/redesign.yaml` as area paths.

3. **Register them dormant first.** The redesign wave configs use `mode: manual` so bootstrap creates the structure without immediately starting build/tend loops.

4. **Verify queryability.** `lfq list` shows the waves. `lfq show redesign` returns the area list pointing at the four member waves. This is the foundation everything else builds on.

5. **Document the bootstrap** in scratch/ — what commands were run, what the initial state looks like. This becomes the first memory for the chord-wave's Letta agent later.

## Done when

- `lfq show` for each wave returns its config and status
- `lfq show redesign` includes the four area paths
- The relationship is persisted in YAML and survives lfd restart
