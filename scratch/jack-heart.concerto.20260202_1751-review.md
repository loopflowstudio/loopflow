# Quick Experiment Path - Design Review

## What was implemented

Added a "Quick Experiment" feature to Concerto that lets users run loopflow steps (`design`, `review`, `debug`, `implement`) directly on their codebase without creating a wave. This appears in two places:

1. **Sidebar empty state** - When no waves exist, users see quick experiment buttons prominently displayed
2. **Detail panel placeholder** - When no wave is selected, users see the same quick experiment UI

Also replaced the old text-based running state progress display with visual `FlowProgressPills` - horizontal step indicators showing completed/current/pending steps with elapsed time.

## Key choices

**Quick experiments run on main repo, not worktrees.** The design doc specified this explicitly - worktree isolation isn't needed for one-off exploration. If the step makes changes, users can create a wave afterward.

**Single source of truth for step definitions.** The `QuickExperiment` enum holds the four common steps and their descriptions once, used by both sidebar and detail views.

**Flow progress data moved server-side.** The API now returns `flow_steps: [String]` (resolved from the flow definition) instead of just `total_steps: Int`. This lets the UI show actual step names in the pills instead of just progress numbers.

**Removed `progressText`, `elapsedTime`, `progressDisplay` from Wave model.** These were replaced by `FlowProgressPills` which handles its own formatting and live timer updates. The tests for these removed methods were also deleted.

**Duplicate `launchQuickExperiment` functions.** Both `ContentView` and `WaveSidebar` have their own copy. This is intentional - each view encapsulates its own error handling and state. The function is short (6 lines) and extracting it would require passing error handlers around for minimal benefit.

## How it fits together

```
┌─ WaveSidebar ──────────────────────────────────────────┐
│  [Quick Experiment buttons] → launchQuickExperiment()  │
│  [Create Wave]              → createWaveDirectly()     │
└────────────────────────────────────────────────────────┘
                        ↓
┌─ TerminalLauncher ─────────────────────────────────────┐
│  launchStep(step, terminal, at: repo)                  │
│  → launchTerminal(terminal, at: repo, command: "lf X") │
└────────────────────────────────────────────────────────┘

┌─ WaveDetailPanel ──────────────────────────────────────┐
│  runProgressSection:                                   │
│    Running → FlowProgressPills(steps, currentIndex,    │
│                                startedAt)              │
└────────────────────────────────────────────────────────┘
                        ↓
┌─ http_server.py ───────────────────────────────────────┐
│  _wave_to_dict():                                      │
│    flow_steps ← _get_flow_step_names(wave.flow, repo)  │
└────────────────────────────────────────────────────────┘
```

## Risks and bottlenecks

**Terminal app detection.** `TerminalLauncher.launchStep` relies on AppleScript for Warp (the default terminal). Warp requires Accessibility permissions for UI scripting. If permissions aren't granted, the error is surfaced but users may not immediately understand why.

**Flow step resolution at API time.** Each wave list request now calls `load_flow()` to resolve step names. This adds slight overhead but flows are typically small (3-5 steps) and cached by the file system.

## What's not included

- **Clipboard paste detection** - The design mentioned improvisers wanting to "paste an error and run `lf debug`" but this auto-detection was explicitly out of scope
- **Auto-wave promotion** - Quick experiments don't track history or auto-promote to waves
- **Session persistence** - Quick experiment terminal sessions aren't tracked; they're truly fire-and-forget
