# 03: Portfolio View

**Finish line:** Multi-repo, multi-wave status at a glance. Wave health, PR state, and active attention items per wave. The 10-second assessment: "where do things stand?"

## Context

The conductor needs a panoramic view. Not drill-in-to-see-details — glance-and-know-what-matters. This serves the tend flow's calibration moment: the human looks at the portfolio, the chord surfaces its assessment, the human calibrates.

Absorbs and replaces the existing scale/05 (cross-repo UI) concept with the conductor framing.

## What to build

1. **Wave cards.** Each wave as a card showing: name, health indicator (healthy/stalled/blocked/shallow), last activity, current work item, open attention count, and queue pressure. Color-coded status — scannable.

2. **Chord grouping.** Waves grouped by chord membership. The redesign chord's four waves together. Ungrouped waves separate. Visual hierarchy matches the coordination structure.

3. **Cross-wave indicators.** File overlap, trigger relationships, active conflicts. Visible without drilling in — lines or badges between related wave cards.

4. **Repo scope.** Toggle between single-repo and multi-repo view. Single repo shows all waves for this repo. Multi-repo shows the portfolio across related repos (parent/child). Promote repo/chord attention filtering into store queries instead of wave-by-wave HTTP filtering before this view goes broad.

5. **Trend lines.** Per wave: velocity (PRs/week), attention frequency, time-to-resolve. Not detailed charts — sparklines or directional indicators. "This wave is accelerating" vs "this wave is slowing down."

## Done when

- Portfolio view shows all waves with health status
- Chord grouping is visible
- Status is assessable in <10 seconds
- Cross-wave relationships are visible
- Works for both single-repo and multi-repo
