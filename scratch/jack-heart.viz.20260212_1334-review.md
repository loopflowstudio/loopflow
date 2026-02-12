# Viz Phase 1 + Phase 2 Research — Review

Branch: `jack-heart.viz.20260212_1334`

## What was implemented

Five roadmap items across Phase 1 and Phase 2:

1. **Decouple screenshots from publish** — removed `skip_screenshots` param and `_generate_screenshots()` call from `_release()` in `publish.py`. Added `lf screenshots` step. The `publish.py screenshots` subcommand still delegates to `_generate_screenshots()`.

2. **Screenshot coverage + persona filtering** — added `directions` tags to screenshot manifest entries, `--direction` flag to `generate_screenshots.py`, 3 new screenshot states (empty, failed, runs tab), `--mock-config` and `--tab` flags, `ScreenshotTabKey` environment key for tab override in `WaveDetailPanel`.

3. **Docs inventory** — created `reports/viz/` with README, deleted stale reports (`reports/cli/ux-polish.md`, `reports/code-quality.md`).

4. **Codebase summary** — added `.lf/summary.md` for agent context.

5. **Visual design research** — researched Linear, Notion, Arc, Figma. Wrote `reports/viz/visual-research.md` with patterns across 5 focus areas. Identified 5 highest-leverage findings for the design audit.

## Key choices

**Tags over directories for persona filtering.** Screenshots stay flat in `docs/screenshots/`. Filtering at generation time via `directions` field in YAML avoids symlinks, duplicates, and path complexity.

**Environment key for tab override.** `ScreenshotTabKey` passes `--tab` through SwiftUI environment to `WaveDetailPanel`. Cleaner than global state or parameter threading through the view hierarchy.

**`arg()` helper in `ScreenshotMode.fromArgs()`.** Local function extracts repetitive `firstIndex(of:)` / bounds-check pattern. Reduces boilerplate without adding abstraction.

**Organized research by focus area, not by app.** The design audit needs to compare Concerto against patterns, not against specific apps. Five focus areas: sidebar, status, color, typography, spacing.

**Linear as primary reference.** Closest analogue — sidebar with status-grouped items, reduced visual noise, LCH-based color system. The other three apps provide supplementary patterns for specific concerns.

## How it fits together

**Screenshot pipeline**: `screenshots.yaml` → `generate_screenshots.py` (filters by `--direction`, passes `--mock-config`/`--tab`) → Concerto binary (`ScreenshotMode.fromArgs()`) → `ScreenshotWindow` (configures `RepoState` mock data, injects tab override via environment) → `WaveDetailPanel` (reads `screenshotTab` on appear).

**Research pipeline**: visual-research.md documents patterns → feeds 04-design-audit (gap analysis) → feeds Phase 3 theming and Phase 4 polish.

## Risks and bottlenecks

- **`wave-deploy-fix` branch doesn't exist in demo repo.** Mock mode bypasses real worktree data, so the failed wave screenshot works. Would break if screenshot mode fell back to real data. Low risk.

- **`hasAppliedScreenshotTab` guard.** Prevents re-applying tab override on view re-appear. Correct for screenshot mode; no impact on normal usage since `screenshotTab` defaults to nil.

- **Visual research is LLM-generated, not from hands-on use.** Patterns are synthesized from public knowledge about these apps, not from direct measurement of pixel values or interaction timing. Specific measurements (e.g., "224px sidebar width", "28-32px row height") should be verified during the design audit.

## What's not included

- Actual screenshot generation (requires building Concerto and running the pipeline)
- Improvise mode launcher screenshot (feature doesn't exist yet)
- Interactive session screenshot (requires Ghostty terminal running)
- UI test variants for new screenshots
- Design audit (Phase 2: 04-design-audit — next item)
