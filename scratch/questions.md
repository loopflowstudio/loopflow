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

**Remaining (CANNOT be exercised on this machine — exact evidence below).** The
*populated* `WaveDetailPane` states — Wave selection into a detail pane with real
Project/Task lens rows, Chat-child selection, and the TaskWorkspace sheet — need
a Wave whose registry data carries the W2-123 lens/attention fields. No
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

Net: the structural fix (`@StateObject`→`@ObservedObject`) and the cold-launch /
repo-switch / refresh / default-pane (`RoadmapView` fix site) zero-cycle result
are proven; the *populated* detail-pane / Chat / sheet matrix and its
mockup-parity are **not yet exercised** and cannot be here. Do NOT mark the Task
complete or publish the PR. A maintainer on a machine whose `lf` matches the DB
(so registered Waves load with lens data) runs
`scripts/check_attributegraph_cycles.sh`, selects a populated Wave, opens Chat
and a sheet during the capture window, and confirms zero cycles + mockup parity.

## Project lens fold precedence

Chose `red > green > unknown > black` for `WaveLens.forProject`, and let a red
Task override a live (green) Project body. Rationale: a parent must surface the
most actionable child state, and unavailable evidence must never collapse into
"off and clean." If product wants a live body to always read green regardless of
a stuck child, flip that one branch — the fold is isolated and unit-tested.
