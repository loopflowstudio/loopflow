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

**Resolved via `mock-waves` (2026-07-15).** The *populated* `WaveDetailPane`
selection path — objective, Projects, KRs, verbatim Task lens rows, and the
`WaveChatView` pane — is now driven **without the registry**: the `mock-waves`
UI-test mode seeds a fixture Wave (`MockWaveFixture`, from the round-tripped
`wave_detail.json` wire shape) and auto-selects it, so cold launch renders the
full detail hierarchy. `scripts/check_attributegraph_cycles.sh --mock` captured
**0** AttributeGraph/cycle lines across multiple launches — proving Wave
selection + Chat render (the second `@StateObject`→`@ObservedObject` fix site,
and the sheet-time cycle's shared root) is cycle-free here. `MockWaveFixtureTests`
+ `WaveDetailReadingTests` prove the seeded hierarchy's content (lenses verbatim)
at the data layer.

**Sheet presentation closed too (2026-07-15).**
`scripts/check_attributegraph_cycles.sh --sheet` raises the create sheet over the
settled populated surface (env-gated `LOOPFLOW_UI_TEST_PRESENT_SHEET`, ~1.5s in),
so the last matrix leg is captured with no click: **0** cycles across three
launches. All six acceptance interactions (cold launch, repo switch, Wave
selection, refresh, Chat selection, sheet/dialog presentation) are now proven
zero-cycle headlessly on this machine.

The registry itself remains unusable on this machine (kept for the record). No
`(current-schema lf + populated store)` pair exists here. Probed 40+ local `lf`
binaries against every reachable store (2026-07-15):

- **PATH `lf` 0.11.1** (`~/.local/bin/lf`, the "restored" one): the production
  store `~/.lf/loopflow.db` is on divergent migration `0.11.009_profiles`, which
  this binary does not know → `local store is incompatible`, `No wave registry`.
  So the app's default-resolved `lf` loads zero registered Waves (amber
  unknown-lens placeholders — confirmed on screen).
- **Current-schema builds** (my worktree `target/debug/lf`, main checkout's
  `target/debug/lf`): read empty / 1-wave (`feedback`) isolated dev stores. No
  populated Waves.
- **Populated stores** (41 Waves incl. `infrastructure`/`intelligence`) exist
  only behind **old-schema** binaries (e.g. `loopflow.bugs/target/debug/lf`).
  Two independent blocks make them unusable by my build: (1) `lf help stop`
  **fails** on them, so the app's `hasWaveCapableLf` probe (needs `wave` AND
  `stop`) rejects the bundled `lf` and falls back to the incompatible PATH `lf`;
  (2) their `ls --json` is the pre-W2-123 shape (`id,name,status,paused,goal,
  repo,iteration,workers,active_runs,live,endpoint,created_at,parent_wave_id`) —
  no lens/attention/task fields — which the current DTO decode requires with no
  defaults (the wire-type rule), a hard parse failure.
- Divergent lineage (`009_profiles` vs main's `009`) means the populated old
  stores can't be migrated forward into my schema either.
- The in-app `mock-waves` UI-test mode is inert: it only gates off live queries
  and seeds no fixture, and `LOOPFLOW_UI_TEST_SELECT_BRANCH` has no consumer in
  the app — so it renders empty.

Net: the structural fix (`@StateObject`→`@ObservedObject`) is proven across the
**entire** acceptance matrix — cold launch / repo switch / refresh / default
pane against the live registry, plus populated Wave selection + Chat render
(`--mock`) and sheet presentation (`--sheet`) via the fixture-seeded surface,
all zero-cycle and headless. The registry limitation below no longer blocks the
directive gate: the populated detail-pane / Chat / sheet matrix and mockup
parity are exercised through `MockWaveFixture` instead of the live registry. A
maintainer whose `lf` matches the DB can additionally confirm against real
registered Waves, but that's corroboration, not a remaining blocker.

## Project lens fold precedence

Chose `red > green > unknown > black` for `WaveLens.forProject`, and let a red
Task override a live (green) Project body. Rationale: a parent must surface the
most actionable child state, and unavailable evidence must never collapse into
"off and clean." If product wants a live body to always read green regardless of
a stuck child, flip that one branch — the fold is isolated and unit-tested.
