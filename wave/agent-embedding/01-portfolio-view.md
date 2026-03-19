---
asana_id: '1213718096105435'
linear_id: f055918b-3e00-4f1f-ad40-9aa25b72f1a1
---
# 01: Portfolio View

**Finish line:** Multi-repo, multi-wave status at a glance. Wave health, PR state, and active attention items per wave. The 10-second assessment: "where do things stand?"

## Context

The conductor needs a panoramic view. Not drill-in-to-see-details — glance-and-know-what-matters. This serves the tend flow's calibration moment: the human looks at the portfolio, the chord surfaces its assessment, the human calibrates.

The single-repo workspace is now fully built out. `WaveWorkspaceView` routes selected waves to a work surface (native session by default, terminal tab when a session exists), `WaveDetailPanel` shows full lifecycle state (current run, history, diff stats, goals/risks/roadmap, StepRunner controls), and `AttentionQueueView` is the no-selection home with Interactive/Algedonic filtering. `PortfolioWindow` already exists as a repo-card shell: it lists repos, shows wave counts / blocked counts / diff totals, and lets the conductor jump straight into a repo or wave.

The measurement groundwork also exists. `lfd` records local interactive runs in `terminal_sessions`, so portfolio trend work already has one source of truth for in-app completion rate and resume latency. Keep deriving the trend view from those persisted session rows instead of adding a second analytics cache.

## What to build

1. **Wave cards.** Each wave as a card showing: name, health indicator (healthy/stalled/blocked/shallow), last activity, current work item, open attention count, queue pressure, and whether an interactive terminal session is active. Color-coded status — scannable.

2. **Chord grouping.** Waves grouped by chord membership. Visual hierarchy matches the coordination structure.

3. **Cross-wave indicators.** File overlap, trigger relationships, active conflicts. Visible without drilling in — lines or badges between related wave cards.

4. **Repo scope.** Toggle between single-repo and multi-repo view. Single repo shows all waves for this repo. Multi-repo shows the portfolio across related repos (parent/child). Promote repo/chord attention filtering and terminal-session summaries into store queries instead of repo-by-repo HTTP cards before this view goes broad.

5. **Trend lines.** Per wave: velocity (PRs/week), attention frequency, time-to-resolve, and recent terminal-session success/failure trend. Derive the terminal metrics from persisted `terminal_sessions` rows. Not detailed charts — sparklines or directional indicators. "This wave is accelerating" vs "this wave is slowing down."

6. **Shared data model, not a dashboard fork.** Build from the same wave/run/attention/terminal-session stores already used by the queue and terminal sidebar. Treat wave-level cards as summaries over potentially many runs: foreground run, running-run count, waiting-run count, and active terminal presence should all be derivations from run/session state rather than assumptions that a wave only has one execution.

## Done when

- Portfolio view shows all waves with health status
- Chord grouping is visible
- Status is assessable in <10 seconds
- Cross-wave relationships are visible
- Works for both single-repo and multi-repo
