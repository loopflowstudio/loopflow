---
asana_id: '1213718096105435'
linear_id: f055918b-3e00-4f1f-ad40-9aa25b72f1a1
notion_id: 32af8f99-3d81-817f-96ff-d7e5bca1c64d
---
# 02: Portfolio View

**Finish line:** Multi-repo, multi-wave status at a glance. Wave health, PR state, and active attention items per wave. The 10-second assessment: "where do things stand?"

## Context

The conductor needs a panoramic view. Not drill-in-to-see-details — glance-and-know-what-matters. This serves the garden flow's calibration moment: the human looks at the portfolio, the chord surfaces its assessment, the human calibrates.

The single-repo home screen now exists: `WaveWorkspaceView` routes selected waves to a workspace multiplexer with roadmap, runs, terminal, and other panes. `AttentionQueueView` is the no-selection home. The queue empty state shows wave overview cards — each wave with a status icon, display name, vision tagline, and diff indicator — clickable to select. This is a lightweight portfolio surface within the repo window. There is also already a repo-card portfolio shell in `PortfolioWindow`: it lists repos, shows wave counts / blocked counts / diff totals, and lets the conductor jump straight into a repo or wave. Portfolio is the next scale step — deepen both existing surfaces with the same queue, run, and terminal signals instead of replacing them.

The measurement groundwork also exists now. `lfd` records local interactive runs in `terminal_sessions`, so portfolio trend work already has one source of truth for in-app completion rate and resume latency. Before the shipped workspace milestone, that in-app rate was effectively 0% because every interactive step escaped to chat UI or an external terminal. Keep deriving the trend view from those persisted session rows instead of adding a second analytics cache.

A terminal-per-wave dashboard is plausible later, but only after `wave/lfd/` lands daemon-owned tmux sessions end to end. Until then, portfolio should treat terminal presence as summary signal and drill-in surface, not spin up a second client-owned terminal stack.

Absorbs and replaces the existing scale/05 (cross-repo UI) concept with the conductor framing.

## What to build

1. **Wave cards.** Each wave as a card showing: name, health indicator (healthy/stalled/blocked/shallow), last activity, current work item, open attention count, and queue pressure. Color-coded status — scannable.
2. **Chord grouping.** Waves grouped by chord membership. The redesign chord's four waves together. Ungrouped waves separate. Visual hierarchy matches the coordination structure.
3. **Cross-wave indicators.** File overlap, trigger relationships, active conflicts. Visible without drilling in — lines or badges between related wave cards.

4. **Repo scope.** Toggle between single-repo and multi-repo view. Single repo shows all waves for this repo. Multi-repo shows the portfolio across related repos (parent/child). Promote repo/chord attention filtering and terminal-session summaries into store queries instead of repo-by-repo HTTP cards before this view goes broad.

5. **Trend lines.** Per wave: velocity (PRs/week), attention frequency, time-to-resolve, and recent terminal-session success/failure trend. Derive the terminal metrics from persisted `terminal_sessions` rows so the same data can back both reviewer measurement and in-product trend lines. Not detailed charts — sparklines or directional indicators. "This wave is accelerating" vs "this wave is slowing down."

6. **Shared data model, not a dashboard fork.** Build the view from the same wave/run/attention/terminal-session stores already used by the queue and terminal sidebar. If the portfolio needs a new summary query, add it at the store/service layer rather than introducing a portfolio-only cache or extending `PortfolioRepoState` into a second source of truth. Treat wave-level cards as summaries over potentially many runs: foreground run, running-run count, waiting-run count, and active terminal presence should all be derivations from run/session state rather than assumptions that a wave only has one execution.

## Done when

* Portfolio view shows all waves with health status
* Chord grouping is visible
* Status is assessable in <10 seconds
* Cross-wave relationships are visible
* Works for both single-repo and multi-repo
