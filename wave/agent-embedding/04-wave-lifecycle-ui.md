---
asana_id: '1213718081065257'
linear_id: 2c9c7c0b-7b1e-4855-bbc8-18592d1c8f49
---
# 04: Wave Lifecycle UI

**Finish line:** Create, configure, start, stop, and monitor waves from Concerto. Worktree management integrated. The full wave lifecycle without touching the terminal unless you choose to enter the workspace.

## Context

Queue triage and terminal workspaces now cover “I saw a wave” and “I am acting inside this wave.” The missing piece is the control plane around them: creating waves, editing configs, starting and stopping runs, and seeing run history without dropping to `lfq` or raw YAML.

Concerto should make wave management visual — especially for the conductor who's managing multiple waves and wants to adjust configuration without context-switching. It also needs to stay honest about product boundaries: local repos can hand interactive steps off into embedded terminals; remote repos may only show queue and run state until remote PTY support exists.

## What to build

1. **Wave creation.** New wave wizard: name, area (file picker), direction (picker with descriptions), flow (picker with step preview). Creates the YAML and registers with `lfd`.

2. **Wave configuration.** Edit any wave parameter from Concerto. Direction, area, flow, agent, triggers. Changes write back to YAML and update `lfd`. This is also where the chord's proposed mutations surface for human review.

3. **Start/stop controls.** Run a wave, stop a running wave, restart. Starting a wave that later blocks on an interactive step should hand off into the existing terminal workspace / attention queue flow instead of opening a parallel session UI.

4. **Worktree management.** Each wave's worktree visible and manageable. Create, switch, prune. Integrated with wave lifecycle — starting a wave ensures its worktree exists, and interactive waves surface the matching worktree in the terminal sidebar.

5. **Run history.** Per wave: recent runs, their status, what shipped, where they got stuck, and any associated attention items or terminal sessions. Drill into a run to see the flow execution and the human checkpoints it triggered.

## Done when

- Waves can be created and configured entirely from Concerto
- Start/stop/restart works from the UI
- Worktree lifecycle is integrated
- Run history is visible with step-level detail
