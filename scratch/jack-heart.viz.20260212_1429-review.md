# Viz Phases 1-3 — Review

Branch: `jack-heart.viz.20260212_1429`

## What was implemented

Phases 1-3 of the viz roadmap. Phase 1+2 were reviewed in the 1347 review doc; this branch adds Phase 3 (theming system) and a compress pass.

### Phase 1: Screenshot workflow
1. **Decouple screenshots from publish** — removed `_generate_screenshots()` from `_release()`, added `lf screenshots` step.
2. **Screenshot coverage** — `directions` tags in manifest, `--direction`/`--mock-config`/`--tab` flags, 3 new states (empty, failed, runs tab), `ScreenshotTabKey` environment.

### Phase 2: Research & audit
3. **Docs inventory** — `reports/viz/` with README, stale reports deleted.
4. **Codebase summary** — `.lf/summary.md` for agent context.
5. **Visual research** — Linear, Notion, Arc, Figma patterns in `reports/viz/visual-research.md`.
6. **Design audit** — deviations catalogued in `reports/viz/design-audit.md` with 4-tier priority.

### Phase 3: Theming system
7. **Status color tokens** — `statusNeutral` added to `StatusColors.swift` and `VISUAL_DESIGN.md`. ~30 system color references replaced with tokens across models and views.
8. **Environment-based palette injection** — `PaletteKey` environment key, static `light`/`dark`/`deepWine` palette variants on `LoopflowPalette`, root injection in `ConcertoApp`.
9. **View migration** — 17 views migrated from `LoopflowPalette.make(for: colorScheme)` to `@Environment(\.palette)`. `DarkButtonStyle` migrated too.
10. **ThemePreview** — `ThemePreview<Content>` renders light + dark + deep wine side-by-side in Xcode previews.
11. **Compress pass** — inlined palette color definitions (removed `Color.loopflowCreamElevated` etc.), deleted dead `make(for:)` factory.

## Key choices

**Environment injection over singleton.** Enables rendering multiple themes simultaneously in previews — the core requirement for fast visual iteration. Follows the existing `ScreenshotTabKey` pattern.

**Three palettes: light, dark, deep wine.** Light and dark are production. Deep wine tests whether burgundy can go moodier — darker background (`#1E1215`), richer accent (`#8B2252`). If it works, it validates the brand direction. If not, we learn the limits.

**`statusNeutral` (#8B8B8B) for idle states.** The design system had no neutral token — `.gray` appeared ~15 times for idle/completed/inactive. Adding it eliminates the most common system color reference.

**`statusInfo` (cyan) for merged PRs.** Merged is informational, not an action. Cyan fits semantically and avoids a one-off `statusMerged` token.

**Inline palette hex values.** The compress pass removed intermediate named colors (`loopflowCreamElevated`, `loopflowCreamMuted`, etc.) that added a layer of indirection without value. Hex values are now directly in the palette definitions, making them self-documenting.

**Migrate all 17 views at once.** Doing it incrementally would leave a half-migrated codebase. The migration is mechanical — delete `@Environment(\.colorScheme)` + computed palette, add `@Environment(\.palette)`.

## How it fits together

**Color token flow**: `VISUAL_DESIGN.md` (spec) -> `StatusColors.swift` (tokens) -> `Wave.swift` / `WaveRun.swift` / `WaveViewModel.swift` (model colors) -> view files (UI).

**Palette flow**: `BrandColors.swift` (palette definitions + `PaletteKey`) -> `ConcertoApp.body` (resolves from `appearanceMode` + `systemScheme`, injects via `.environment(\.palette, ...)`) -> all views read `@Environment(\.palette)`.

**Screenshot pipeline**: `screenshots.yaml` -> `generate_screenshots.py` (filters by direction, passes flags) -> Concerto (`ScreenshotMode.fromArgs()`) -> `ScreenshotWindow` (mock data, tab environment) -> views render with palette.

**Fast iteration**: Change hex values in `BrandColors.swift` palette definitions -> all views update. Use `ThemePreview` in any `#Preview` to see light + dark + deep wine side-by-side.

## Risks and bottlenecks

- **Deep wine contrast untested with real data.** The palette is defined but hasn't been exercised beyond mock screenshots. Legibility of secondary text (`#C8B0A8`) on deep wine surfaces should be verified.

- **Visual research is LLM-generated.** Specific measurements should be verified during hands-on Phase 4 work.

- **Merged PR color (cyan vs GitHub's purple).** `statusInfo` is semantically reasonable but visually different. Could add `statusMerged` later if users find it confusing.

## What's not included

- Font bundling (Tier 2 prerequisite)
- Typography token adoption (Tier 2)
- Button hierarchy — ghost/destructive styles (Tier 3)
- Corner radius / spacing token cleanup (Tier 4)
- Runtime theme switching or user-facing theme picker
