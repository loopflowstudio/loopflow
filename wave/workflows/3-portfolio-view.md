---
linear_id: f055918b-3e00-4f1f-ad40-9aa25b72f1a1
notion_id: 32af8f99-3d81-817f-96ff-d7e5bca1c64d
---
# Portfolio view

**Needs:** workflows/3-wave-scheduling

**Finish line:** Multi-repo, multi-wave status at a glance. Wave health, PR state, and active attention items per wave. The 10-second assessment: “where do things stand?”

## Context

The conductor needs a panoramic view. Not drill-in-to-see-details — glance and know what matters. This serves the garden flow's calibration moment: the human looks at the portfolio, the system surfaces its assessment, the human calibrates.

The single-repo home screen already exists: `WaveWorkspaceView` routes selected waves to a workspace multiplexer, `AttentionQueueView` is the no-selection home, and the empty state already shows wave overview cards. Portfolio is the next scale step — deepen those surfaces with the same queue, run, and terminal signals instead of replacing them.

`lfd` now records local interactive runs in `terminal_sessions`, so portfolio trend work already has one source of truth for in-app completion rate and resume latency. Keep deriving the trend view from those persisted rows instead of adding a second analytics cache.

A terminal-per-wave dashboard is plausible later, but only after daemon-owned tmux sessions land end to end. Until then, portfolio should treat terminal presence as summary signal and drill-in surface, not spin up a second client-owned terminal stack.

## What to build

1. **Wave cards.** Each wave as a card showing name, health indicator, last activity, current work item, open attention count, and queue pressure.
2. **Root grouping.** Group waves by the active root layout so `desktop`, `mobile`, and `workflows` read as one coordinated set.
3. **Cross-wave indicators.** Show file overlap, trigger relationships, and active conflicts without drilling in.
4. **Repo scope.** Toggle between single-repo and multi-repo views.
5. **Trend lines.** Show velocity, attention frequency, time-to-resolve, and recent terminal-session success/failure trend per wave.
6. **Shared data model.** Build the view from the same wave, run, attention, and terminal-session stores already used by the queue and terminal sidebar.

## Done when

- Portfolio shows all waves with health status
- Root grouping is visible
- Status is assessable in under 10 seconds
- Cross-wave relationships are visible
- The same queries support single-repo and multi-repo views
