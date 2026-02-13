# Empty State Refinement — Review

## What was implemented

Unified five inconsistent empty states into a consistent pattern across Concerto:

1. **Sidebar empty state**: Replaced overloaded layout (QuickExperiment 2x2 grid + divider + Create Wave + mock section headers) with the same icon + title + subtitle + button pattern used by the disconnected state. Extracted shared `centeredMessage` helper.

2. **Detail panel step cards**: Reduced visual weight — `Typography.body()` + `.medium` at 100px width (was `sectionTitle()` + `.semibold` at 120px). Background opacity lowered to 0.7.

3. **Detail panel hint**: Made conditional — shows "Create a wave for ongoing work" when sidebar is empty, "Select a wave from the sidebar" when waves exist.

4. **Runs tab empty**: Added explanatory subtitle ("Runs appear here when the wave executes a flow.").

5. **Symphonia placeholder**: Migrated from hardcoded system fonts to design token font names (`Cormorant Garamond`, `Lato`) with inline comments noting token equivalents, since Symphonia doesn't depend on Concerto's DesignSystem module.

## Key choices

| Decision | Why |
|----------|-----|
| Extract `centeredMessage` helper in sidebar | Disconnected and empty states share identical layout. One function, two call sites. |
| Delete `QuickExperimentSidebarView` + `SidebarPreviewView` entirely | Zero call sites outside sidebar empty state + their own previews. Dead code. |
| Inline `QuickExperiment.steps` into `QuickExperimentDetailView` | Only consumer after sidebar deletion. Enum wrapper was indirection for one constant. |
| Remove `color` parameter from `sectionHeader` | Icons are uniformly subdued (`.white.opacity(0.3)`), matching Tier 5 sidebar header style. Color per-section was visual noise in a 280px column. |
| Symphonia uses raw `.custom()` fonts, not `Typography` | Symphonia target doesn't depend on Concerto. Inline token values with comments is the right trade-off until Symphonia gets its own design system. |

## How it fits together

The sidebar now has three states, each using the same visual pattern:
- **Disconnected**: `centeredMessage` with "Connect lfd" + connect button
- **Empty**: `centeredMessage` with "No waves yet" + create button
- **Populated**: `waveList` with grouped sections

The detail panel placeholder (`QuickExperimentDetailView`) is the only remaining Quick Experiment surface — appropriate since the detail panel has room for step cards.

## Bonus: WaveDetailPanel compression (from prior commit)

The compress pass deduplicated four identical `Task { do/catch }` action methods into a single `perform(_:_:)` helper, replaced manual `HStack { Image + Text }` patterns with `Label`, and replaced remaining spacing/padding literals with `Spacing` tokens. Also extracted `sectionLabel` and `configLabel` helpers for repeated patterns.

## Risks and bottlenecks

- **Symphonia fonts**: If the bundled fonts aren't loaded for Symphonia's target, `.custom("Cormorant Garamond", ...)` falls back to system font silently. Low risk — Symphonia is a placeholder app.
- **Empty state detection**: `repoState.waves.isEmpty` in the detail panel hint checks the same state as the sidebar. If waves load asynchronously, there's a brief flash where both show "no waves" text. Existing behavior, not introduced by this change.

## What's not included

- Welcome window, setup view, command palette empty states (out of scope per design)
- Custom illustrations or animations
- The design doc suggested two PRs; implementation combined both layers into one branch since all changes are small and cohesive
