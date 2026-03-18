---
asana_id: '1213718081065257'
linear_id: 2c9c7c0b-7b1e-4855-bbc8-18592d1c8f49
---
# 04: Wave Lifecycle UI

**Finish line:** Create, configure, start, stop, and monitor waves from Concerto. Worktree management integrated. The full wave lifecycle without touching the terminal unless you choose to enter the workspace.

## Context

The workspace and queue surfaces now cover “I saw a wave” and “I am acting inside this wave.” `WaveWorkspaceView` composes native session + optional terminal tab; `AttentionQueueView` handles repo-level triage. The shared state and HTTP layer already expose more lifecycle machinery than the UI currently admits: `RepoState` / `WaveService` can create waves, create-and-run, clone, delete, run, stop, and restart steps, and `WaveDetailPanel` already shows current state plus run history. The missing piece is the control plane: editing configs, exposing the existing lifecycle actions coherently, and making worktree/run monitoring first-class without dropping to `lfq` or raw YAML.

Concerto should make wave management visual — especially for the conductor managing multiple waves. It also needs to stay honest about product boundaries: local repos hand interactive steps off into embedded terminals or native sessions; remote repos may only show queue and run state until remote PTY support exists. Item 02 is the transport dependency here: lifecycle work should consume that `lfd`-owned PTY path once it exists instead of teaching Swift more about wrapped local launch commands.

## What to build

1. **Wave creation.** Promote the existing create/create-and-run capabilities into a dedicated UI: name, area (file picker), direction (picker with descriptions), flow (picker with step preview). Creates the YAML and registers with `lfd` through the current service path instead of shelling out separately.

2. **Wave configuration.** Edit any wave parameter from Concerto. Direction, area, flow, agent, triggers. Changes write back to YAML and update `lfd`. This is also where the chord's proposed mutations surface for human review.

3. **Start/stop controls.** Run a wave, stop a running wave, restart. Starting a wave that later blocks on an interactive step should hand off into the existing terminal workspace / attention queue flow instead of opening a parallel session UI. As item 02 lands, these controls should attach to daemon-owned PTY runs rather than extending the current callback-based local launch shim.

4. **Worktree management.** Each wave's worktree visible and manageable. Create, switch, prune. Integrated with wave lifecycle — starting a wave ensures its worktree exists, and interactive waves surface the matching worktree in the terminal sidebar.

5. **Run history.** Per wave: recent runs, their status, what shipped, where they got stuck, and any associated attention items or terminal sessions. Start from the current Current/Runs split in wave detail, then add drill-in on flow execution and the human checkpoints it triggered.

## Done when

- Waves can be created and configured entirely from Concerto
- Start/stop/restart works from the UI
- Worktree lifecycle is integrated
- Run history is visible with step-level detail
