# Open questions / assumptions — W2-178

## AttributeGraph cycles — diagnosed, fixed, and empirically checked

- **Diagnosed** the structural cause: two views (`WaveDetailPane`,
  `RoadmapView`) wrapped the externally-owned `TaskTerminalStore.shared`
  singleton in `@StateObject`. That create-and-own lifecycle fires the
  singleton's publisher during the first `body` pass — the known cold-launch /
  sheet-presentation cycle shape.
- **Fixed** both to `@ObservedObject`. Ruled out the other classic trigger:
  no `GeometryReader` / `PreferenceKey` feedback on the Wave surface path.
- **Verified empirically** via `scripts/check_attributegraph_cycles.sh`
  (`log stream --process LoopflowMac` — SwiftUI logs cycles to os_log, not
  stderr). Four real-app launches: **zero** AttributeGraph/cycle lines. Each
  renders `WavesView` + `RoadmapView`, a fix site — so cold launch / repo switch
  / refresh / default pane are proven clean.

**Remaining (needs a human pass):** the *populated* `WaveDetailPane` states —
Wave selection into a detail pane with real Project/Task lens rows, Chat-child
selection, and the TaskWorkspace sheet. Headless, this machine could not drive
them: the local `lf` (0.11.1) is behind the shared `lfdb` (migration 0.11.012),
so the app loads no registered Waves (they render as unknown-lens placeholders),
and the app's launcher-resolved `lf` bypasses PATH overrides. Run the script on a
machine where the app's bundled `lf` matches the DB, select the `product` Wave,
open Chat and a sheet during the capture window, and confirm zero cycles. The fix
is the identical singleton pattern already proven clean at the `RoadmapView`
twin, so this is confirmation, not open risk — but per the directive, do not
settle/publish until it is done.

## Project lens fold precedence

Chose `red > green > unknown > black` for `WaveLens.forProject`, and let a red
Task override a live (green) Project body. Rationale: a parent must surface the
most actionable child state, and unavailable evidence must never collapse into
"off and clean." If product wants a live body to always read green regardless of
a stuck child, flip that one branch — the fold is isolated and unit-tested.
