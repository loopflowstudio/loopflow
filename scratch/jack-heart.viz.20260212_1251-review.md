# Viz Phase 1 — Review

Branch: `jack-heart.viz.20260212_1251`

## What was implemented

Three roadmap items completing Phase 1 (screenshot workflow) and one Phase 2 prerequisite:

1. **Decouple screenshots from publish** — removed `skip_screenshots` param and `_generate_screenshots()` call from `_release()` in `publish.py`. Added `lf screenshots` step for the intended workflow.

2. **Screenshot coverage + persona filtering** — added `directions` tags to screenshot manifest entries, `--direction` flag to `generate_screenshots.py`, 3 new screenshot states (empty, failed, runs tab), and supporting Swift changes (mock data, `--tab` flag, `--mock-config` flag, environment-based tab override).

3. **Docs inventory** — created `reports/viz/` for future research/audit output, deleted stale TODO docs (`reports/cli/ux-polish.md`, `reports/code-quality.md`).

4. **Codebase summary** — added `.lf/summary.md` for agent context.

## Key choices

**Tags over directories for persona filtering.** Screenshots stay flat in `docs/screenshots/`. Filtering happens at generation time via `directions` field in YAML, not filesystem hierarchy. Avoids symlinks, duplicates, and path complexity.

**Environment key for tab override.** `ScreenshotTabKey` passes the `--tab` value through SwiftUI's environment to `WaveDetailPanel`. Cleaner than global state or direct parameter threading through the view hierarchy.

**Separate mock configs.** `mock_config: empty` triggers `configureMockWavesEmpty()` instead of `configureMockWaves()`. Extensible pattern for future mock variants without proliferating boolean flags.

**`arg()` helper in `ScreenshotMode.fromArgs()`.** Extracted repetitive argument parsing into a local function. Reduces 20+ lines of boilerplate to a clean pattern.

## How it fits together

`screenshots.yaml` is the manifest. Each entry has optional `directions`, `mock_config`, and `select_tab` fields. `generate_screenshots.py` reads the manifest, filters by `--direction` if given, and passes `--mock-config` and `--tab` through to the Concerto binary. `ScreenshotWindow` reads those args, configures `RepoState` with the right mock data, and injects the tab override via environment. `WaveDetailPanel` reads the environment on appear and switches to the requested tab.

## Risks and bottlenecks

- **`wave-deploy-fix` branch doesn't exist in demo repo.** `_create_demo_worktrees` only creates `wave-auth-feature` and `wave-api-refactor`. The failed wave screenshot selects by matching mock `WaveViewModel.branch`, so it works — but if screenshot mode ever falls back to real worktree data, this would break. Low risk since mock mode bypasses real data.

- **`hasAppliedScreenshotTab` guard.** Prevents re-applying the tab override if the view re-appears. Correct for screenshot mode; no impact on normal usage since `screenshotTab` defaults to nil.

## What's not included

- Actual screenshot generation (requires building Concerto and running the pipeline)
- Improvise mode launcher screenshot (feature doesn't exist yet)
- Interactive session screenshot (requires Ghostty terminal running)
- UI test variants for new screenshots
- Populating `reports/viz/` with research (that's Phase 2: 03-visual-research)
