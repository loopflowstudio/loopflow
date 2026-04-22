# Beat Sequencer

**Finish line:** A compositional view where play/tune/silence beats are visible as a grid, and the user can edit wave rhythm directly — wake silent waves, adjust garden-to-build ratios, change beat patterns.

## Context

The beat sequencer is the mid-altitude zoom level — the "studio" to the runboard's "cockpit." It's the surface that embodies loopflow's thesis that waves have rhythm and that rhythm is editable.

This is what differentiates loopflow from every other agent orchestration tool. Claude Agent Teams gives you parallel agents. The beat sequencer gives you *compositional* control over how agents work over time.

## What to build

- Grid view: rows are waves, columns are time units, cells are beats (play/tune/silence)
- Visual rhythm patterns across waves
- Direct manipulation: drag to change beat patterns, click to wake/silence a wave
- Garden-to-build ratio visualization and adjustment
- Shared scratchpad for inter-wave handoffs (deferred from Phase 1)

## What to learn first

- Does anyone actually want to edit rhythm, or do they just want to observe it?
- Is the grid metaphor natural, or does it impose structure people don't think in?
- Should the sequencer emerge from usage data (show patterns the system detected) or be user-authored?

## Dependencies

- Runboard (Phase 1) must exist as the foundation
- Beat history data from the wave status API
- Enough real wave usage to know what rhythms actually look like
