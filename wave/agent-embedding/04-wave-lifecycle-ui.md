# 04: Wave Lifecycle UI

**Finish line:** Create, configure, start, stop, and monitor waves from Concerto. Worktree management integrated. The full wave lifecycle without touching the terminal (unless you want to).

## Context

Currently wave management happens through `lfq` CLI and YAML editing. Concerto should make this visual — especially for the conductor who's managing multiple waves and wants to adjust configuration without context-switching.

## What to build

1. **Wave creation.** New wave wizard: name, area (file picker), direction (picker with descriptions), flow (picker with step preview). Creates the YAML and registers with lfd.

2. **Wave configuration.** Edit any wave parameter from Concerto. Direction, area, flow, agent, triggers. Changes write back to YAML and update lfd. This is also where the chord's proposed mutations surface for human review.

3. **Start/stop controls.** Run a wave, stop a running wave, restart. Visible in both the portfolio view and wave detail.

4. **Worktree management.** Each wave's worktree visible and manageable. Create, switch, prune. Integrated with wave lifecycle — starting a wave ensures its worktree exists.

5. **Run history.** Per wave: recent runs, their status, what shipped, where they got stuck. Drill into a run to see the flow execution (which steps ran, what each produced).

## Done when

- Waves can be created and configured entirely from Concerto
- Start/stop/restart works from the UI
- Worktree lifecycle is integrated
- Run history is visible with step-level detail
