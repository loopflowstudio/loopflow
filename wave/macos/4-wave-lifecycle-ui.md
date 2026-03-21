---
asana_id: '1213718081065257'
linear_id: 2c9c7c0b-7b1e-4855-bbc8-18592d1c8f49
notion_id: 32af8f99-3d81-818c-af27-c6add76d0279
---
# 01: Wave Lifecycle UI — Remaining

**Finish line:** Worktree management and step-level run history are first-class in Concerto. The last lifecycle gaps that still require dropping to the terminal.

## Context

The core lifecycle surface shipped. Concerto can create, configure (flow/area/direction/agent/triggers), start/stop, delete/rename, land/next, and view PRs. The workspace multiplexer provides launcher, runs, roadmap, terminal, and other panes in a persistent binary split tree per wave. Roadmap cards are now actionable (inline content preview, priority editing via file rename, play button for targeted ingest + flow run). `WaveSidebar` supports creation, rename, and deletion via context menu.

Multi-run display exists: `WaveRunsTab` shows all runs for a wave sorted by iteration, with combine-PRs action for multiple open PRs. `FlowProgressPills` shows step progress during a running flow. Worktree names appear as read-only metadata in run detail rows.

Two gaps remain before the "full lifecycle without touching the terminal" finish line is crossed:

1. **Worktree management.** Creating, switching, and pruning worktrees still requires `lf ops wt` in the terminal. Concerto should surface worktree state per wave and offer create/switch/prune actions through `WaveService`.

2. **Step-level run history.** Individual step execution within a completed or failed flow isn't broken out. A flow like `build` (kickoff → review-design → code loop → gate) should show which steps ran, their status, duration, and where the flow got stuck. `WaveRun` has `currentStep` and `stepIndex` fields but no historical step-by-step UI for past runs.

## What to build

1. **Worktree management.** Each wave's worktree visible and actionable from the workspace. Create worktrees for waves that don't have one. Switch between worktrees. Prune merged worktrees. Starting a wave should ensure a usable worktree exists. The existing `lf ops wt` commands provide the backend; the UI needs to surface them through the service layer.

2. **Step-level run detail.** Drill into a run to see step-by-step execution: step name, status (running/complete/failed/skipped), duration, and any associated attention items or terminal sessions. Show the wave's default flow separately from each run's actual flow so one-shot overrides stay legible.

## Done when

- Worktree create/switch/prune available from Concerto
- Run detail shows step-level execution history for completed/failed runs
- Starting a wave without a worktree prompts creation
