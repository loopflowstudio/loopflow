# Viz Phase 1 + Phase 2 — Review

Branch: `jack-heart.viz.20260212_1347`

## What was implemented

Phase 1 (screenshot workflow) and Phase 2 (research & audit) of the viz roadmap, plus the first Tier 1 polish fix from Phase 4:

1. **Decouple screenshots from publish** — removed `skip_screenshots` param and `_generate_screenshots()` call from `_release()` in `publish.py`. Added `lf screenshots` step.

2. **Screenshot coverage + persona filtering** — added `directions` tags to screenshot manifest, `--direction` flag to `generate_screenshots.py`, 3 new screenshot states (empty, failed, runs tab), `--mock-config` and `--tab` flags, `ScreenshotTabKey` environment key for tab override.

3. **Docs inventory** — created `reports/viz/` with README, deleted stale reports.

4. **Codebase summary** — added `.lf/summary.md` for agent context.

5. **Visual design research** — researched Linear, Notion, Arc, Figma. Wrote `reports/viz/visual-research.md` organized by 5 focus areas.

6. **Design audit** — catalogued all deviations between Concerto and VISUAL_DESIGN.md. Wrote `reports/viz/design-audit.md` with prioritized fix list and recommended PR sequence.

7. **Status color tokens (Tier 1 polish)** — added `statusNeutral` to `StatusColors.swift` and `VISUAL_DESIGN.md`. Replaced ~30 system color instances (`.gray`, `.blue`, `.orange`, `.purple`, `.cyan`) with design tokens across models and views.

## Key choices

**`statusNeutral` (#8B8B8B) for idle states.** The design system had no neutral token — `.gray` appeared ~15 times for idle/completed/inactive. Adding `statusNeutral` makes the intent explicit and eliminates the most common system color reference.

**`statusInfo` (cyan) for merged PRs.** `.purple` was used for merged state in badges. Merged is informational — it's a completed state, not an action. Cyan (`statusInfo`) fits semantically and avoids introducing a one-off token.

**Consistent PR colors across timeline and badges.** Timeline dots in `IterationTimeline` now use the same color mapping as `WaveDetailPanel.prBadge` and `WaveRunsTab.RunPRBadge`: open = success green, merged = info cyan, closed = error, draft = warning.

**`.secondary` for non-status grays.** CommandPalette's shortcut styling uses `.secondary` (SwiftUI semantic color) rather than a status token — it's secondary text styling, not status indication.

## How it fits together

**Screenshot pipeline**: `screenshots.yaml` (with `directions`, `mock_config`, `select_tab` fields) -> `generate_screenshots.py` (filters by `--direction`, passes `--mock-config`/`--tab`) -> Concerto (`ScreenshotMode.fromArgs()` with `arg()` helper) -> `ScreenshotWindow` (configures mock data, injects tab via environment) -> `WaveDetailPanel` (reads `screenshotTab` on appear).

**Audit pipeline**: visual-research.md (reference patterns) -> design-audit.md (deviation map with 4 tiers) -> 07-polish.md (updated with tier structure) -> Phase 4 PRs.

**Color token flow**: VISUAL_DESIGN.md (spec) -> StatusColors.swift (tokens) -> Wave.swift / WaveRun.swift / WaveViewModel.swift (model colors) -> 10 view files (UI usage). All system color references in status contexts now route through tokens.

## Risks and bottlenecks

- **`wave-deploy-fix` branch doesn't exist in demo repo.** Mock mode bypasses real worktree data, so the failed wave screenshot works. Low risk.

- **Visual research is LLM-generated.** Specific measurements (sidebar widths, row heights) should be verified during hands-on design work.

- **PR merged color is a judgment call.** Using `statusInfo` (cyan) is semantically reasonable but visually different from GitHub's purple. If users find this confusing, a dedicated `statusMerged` token could be added later.

## What's not included

- Actual screenshot generation (requires building Concerto)
- Font bundling (Tier 2 prerequisite)
- Typography token adoption (Tier 2)
- Button hierarchy (Tier 3)
- Corner radius / spacing token cleanup (Tier 4)
