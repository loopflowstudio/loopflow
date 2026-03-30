# Runboard UI

**Finish line:** A visual surface showing all active waves with live status, expandable detail, and mode-appropriate controls (pause/skip/stop for loop, cancel for manual, trigger for cron).

## Context

The wave status API (sibling item) provides the data. This item is the rendering layer. Ship on whichever surface is fastest — web UI in lfd, Concerto view, or both.

## Core interaction

Wave list as expandable rows:

```
┌─────────────────────────────────────────────────────────┐
│ Wave          Mode     Step          Status    Actions   │
│─────────────────────────────────────────────────────────│
│ engbot        loop     implement     ████░░    ⏸ ⏭ ⏹    │
│ auth-fix      manual   gate          ████████  ✓ done    │
│ dep-scan      cron     (next: 6h)    sleeping  ▶ trigger │
└─────────────────────────────────────────────────────────┘
```

Expanding a row shows mode-appropriate detail:
- **Loop:** beat history (play/tune/silence rhythm), live agent output stream, pause/skip-step/stop
- **Manual:** step-by-step flow progress, result, cancel
- **Cron:** run history, next scheduled time, manual trigger

## What to build

- Wave list view with status, mode, step, and progress
- Expandable detail per wave
- Real-time updates via WebSocket/SSE from the status API
- Launch new wave from the runboard (name, flow, area — minimal config)
- tmux status line integration showing summary (wave count, active/blocked/error)

## What to skip

- Beat sequencer grid (Phase 2)
- Portfolio cross-wave view (Phase 3)
- Workstyle configuration UI
- Cross-wave conflict detection
