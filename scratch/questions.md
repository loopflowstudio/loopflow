# Open questions / assumptions — W2-178

## AttributeGraph zero-cycle proof needs a GUI session

The design note's cycle-regression **acceptance criterion** is a runtime
observation: launch the Mac app, exercise cold launch / repo switch / Wave
selection / refresh / Chat selection / sheet presentation, and confirm the
console logs zero `AttributeGraph: cycle detected` lines.

This run is headless — no display, no attached macOS GUI — so I could not
*observe* the log. What I did instead:

- **Diagnosed** the concrete structural cause on the Wave surface: two views
  (`WaveDetailPane`, `RoadmapView`) wrapped the externally-owned
  `TaskTerminalStore.shared` singleton in `@StateObject`. That create-and-own
  lifecycle fires the singleton's publisher during the first `body` pass — the
  known cold-launch / sheet-presentation cycle shape.
- **Fixed** both to `@ObservedObject` (the correct wrapper for a shared,
  externally-owned `ObservableObject`).
- **Ruled out** the other classic trigger: no `GeometryReader` /
  `PreferenceKey` / `anchorPreference` feedback exists on the Wave surface path
  (only the unrelated `TelemetryDashboardView`).

**Assumption:** the `@StateObject`→`@ObservedObject` fix removes the observed
cycles. A human (or a windowed CI run) should do the one-pass console check
before the serial PR is published, per the note's acceptance criterion. If any
cycle survives, capture the offending attribute and re-diagnose — do not
normalize the noise.

## Project lens fold precedence

Chose `red > green > unknown > black` for `WaveLens.forProject`, and let a red
Task override a live (green) Project body. Rationale: a parent must surface the
most actionable child state, and unavailable evidence must never collapse into
"off and clean." If product wants a live body to always read green regardless of
a stuck child, flip that one branch — the fold is isolated and unit-tested.
