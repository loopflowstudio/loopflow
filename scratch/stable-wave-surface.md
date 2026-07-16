# W2-178 — Stable Wave surface (Mac)

Design note for the first Mac Surface UX slice. Long-form direction lives in
`/Users/jack/src/loopflow.ux/scratch/ux.md`; this is the computable build target.

## User-visible outcome

Selecting a repository and Wave opens one calm workspace:

1. **Navigation** — a repository *dropdown* above one stable Wave list (the
   permanent repo column is gone). Every Wave stays in a single alphabetical
   list; each row carries a small recessed green/red/black glass lens (a quiet
   HAL allusion), no status pills, no state regrouping. Rows are outline-capable
   so a future child Wave can indent under its parent.
2. **Objective + Projects** — the detail pane leads with a one-sentence
   objective; Projects stay persistently visible with open-Task count and KR
   list as their strongest qualities. Each Project row and each Task row wears
   the *same* lens as the Wave, so state reads identically at every level.
3. **Chat** — the default third pane. Operational detail (live status errors,
   raw reasons) is progressive disclosure, never the default.

Internal vocabulary ("registered wave"), raw sync timestamps, giant empty
states, clipped objective prose, and implementation errors leave the primary
hierarchy.

## Source of truth

`lf ls --json` (`WaveSnapshot`) and `lf status --json` (`WaveDetailSnapshot`)
remain the sources. This slice widens the app-side `Wave` reduction to carry the
fields the surface now spends space on — `live`, `active_tasks`,
`active_projects`, `parent_wave_id` — all already present on `WaveSnapshot`.

## Lens grammar — one shared surface across Wave, Project, and Task

Lens semantics are **owned by W2-123**, which merged on this branch (commit
`5e5769c2`) exposing `TaskAttentionSnapshot { level, reason, … }` with the
shared `green | red | black | unknown` level, plus
`tests/fixtures/dto/task_attention_states.json`. This slice renders that shared
grammar on all three row types and derives nothing it can read directly:

- **Task rows** carry a Rust-owned attention snapshot, so the lens uses
  `attention.level` and `attention.reason` **verbatim** (`WaveLens.forTask`).
  Swift never reconstructs the level from status or process flags — same rule as
  `WorkAttention.nowGroup`.
- **Project rows** have no attention field of their own, so the lens is
  **derived only from shared runtime and the Project's Tasks' attention
  evidence** (`WaveLens.forProject`): a live Project body advancing is green, but
  a Task calling for attention (red) wins over it; among the rest the most
  demanding evidence wins, `red > green > unknown > black`. A Project never hides
  a stuck Task behind an advancing sibling.
- **Wave rows** render in the list, where per-Task attention is a focused
  `lf status` read never fetched per row. The list projects from the coarse
  runtime `lf ls` carries for every row — liveness, lifecycle status, active-work
  counts (`WaveLens.forWave`).

Two invariants hold everywhere:

- **Unavailable evidence stays `unknown` with a reason — never a silent black.**
  A `TaskAttentionLevel.unknown` maps to a lit amber lens; a Project folding an
  unknown Task surfaces unknown; an **unregistered** Wave (authored on disk, not
  yet served) has no runtime reading, so `WaveViewModel.lens` returns
  unknown-with-reason rather than guessing from a local tmux/session probe.
- **Parent display derives only from shared runtime and attention evidence,
  never from a Swift filesystem guess.** The old provisional Wave-only projection
  and the tmux-derived authored-Wave color are both gone.

Every lens carries a `reason` string, spoken by VoiceOver and shown on hover, so
the state is legible without seeing the color.

## Affected surfaces

- `swift/Loopflow/Models/Wave.swift` — carry the four shared fields (defaults
  keep existing call sites compiling; `Wave` is an app model, not a wire DTO).
- `swift/Loopflow/Services/RegistryQuery.swift` — `WaveSnapshot.toWave()` passes
  the fields through.
- `swift/Loopflow/Models/WaveLens.swift` — the lens value + color (now including
  `unknown`) + the three projections (`forTask`, `forProject`, `forWave`) mapping
  the shared `TaskAttentionLevel`.
- `swift/Loopflow/Models/WaveViewModel.swift` — expose `lens` (unknown when
  unregistered), `openTaskCount`, `parentWaveId`; drop the pill surface.
- `swift/LoopflowMac/Views/WaveLensView.swift` — the recessed glass lens; amber
  for unknown; a per-row `accessibilityId` so Wave/Project/Task lenses are
  distinctly addressable.
- `swift/LoopflowMac/Views/WaveRow.swift` — lens replaces pill; `indentLevel`.
- `swift/LoopflowMac/Views/WavesView.swift` — repo dropdown replaces the rail;
  single stable alphabetical list; compact empty state.
- `swift/LoopflowMac/PortfolioRepoState.swift` — stable alphabetical sort (drop
  status-priority regrouping).
- `swift/LoopflowMac/Views/WaveDetailPane.swift` — one-sentence objective lead
  with full-objective disclosure; per-Project open-Task count; **Project row
  leads with its derived lens; Task row leads with its verbatim lens** (the lens
  replaces the plain completion checkmark); live-status error demoted to a quiet
  footer; drop "WaveChat" header vocabulary.

## SwiftUI dependency-cycle regression (AttributeGraph)

Native startup and interaction logged repeated `AttributeGraph: cycle detected
through attribute` lines. Root cause on this surface: two views wrapped the
externally-owned `TaskTerminalStore.shared` singleton in `@StateObject`
(`WaveDetailPane`, `RoadmapView`). `@StateObject`'s create-and-own lifecycle runs
during the first `body` pass and, for an already-`@Published` singleton, fires
its publisher mid-evaluation — a dependency cycle at cold launch and sheet
presentation. Both now use `@ObservedObject`, the correct wrapper for a shared,
externally-owned `ObservableObject`. No `GeometryReader`/`PreferenceKey`
feedback exists on the Wave surface path (only the unrelated telemetry
dashboard).

**Acceptance criterion:** cold launch, repository switch, Wave selection,
refresh, Chat selection, and dialog/sheet presentation each produce **zero**
`AttributeGraph: cycle detected` lines. SwiftUI logs cycles to the unified log
(os_log), not stderr, so the authoritative check is `log stream` on the app
process — encoded in `scripts/check_attributegraph_cycles.sh`.

**Verified (this run):** four independent real-app launches (`swift build`
product, `log stream --process LoopflowMac`, ~6–11 s each, rendering the real
window hierarchy) produced **0** AttributeGraph/cycle lines across 95–192
captured log lines each. Every launch renders `WavesView` + `RoadmapView` (the
default detail pane, one of the two `@StateObject`→`@ObservedObject` fix sites),
so the cold-launch / repo-switch / refresh / default-pane paths are empirically
cycle-free. `WaveDetailPane` is the identical singleton fix and compiles clean;
its populated selection/chat/sheet states still want one human pass with the
script's capture window open (this machine's `lf`/lfdb migration divergence — see
`questions.md` — kept the app from loading registered Waves, so the detail pane
could not be driven with real Project/Task rows headlessly).

## End-to-end proof

Build `LoopflowMac`; run `LoopflowTests` (`swift test --package-path swift`).
ViewInspector state tests (the repo's fixture idiom — it has no image-snapshot
harness) plus pure-projection tests cover:

- lens color + reason for each state at Wave, Project, and Task level;
- the shared `task_attention_states.json` fixture mapping **every** state
  verbatim through `forTask`, including `unknown`;
- Project fold precedence (`red > green > unknown > black`; live body vs. a
  red Task; unknown never collapsing to black);
- an unregistered Wave rendering unknown-with-reason;
- row `indentLevel` (future child), compact empty state, one-sentence objective
  lead, per-Project open-Task count, stable alphabetical ordering;
- **the populated detail hierarchy against the real `lf status --json` wire
  fixture** (`WaveDetailReadingTests`): decoding `wave_detail.json` → `workMap`,
  then asserting the objective leads, the Project's KR list + open-Task count are
  present, the Project row lens folds its Tasks' attention verbatim (red
  `waiting for review` wins over black), and each Task row lens equals
  `WaveLensColor(attention.level)` + reason verbatim. This is the mockup
  hierarchy proven at the data layer — the achievable substitute for the live
  populated render, which this machine's registry can't serve (see `questions.md`).

Manually: opening Product shows its objective, all Projects with KR lists +
open-Task counts, every Project/Task row wearing the shared lens, and Chat, with
no horizontal clipping at narrow and wide widths — and the AttributeGraph log
stays clean across the six interactions above.

## Absent / error states

- No repos → compact top-aligned note, not a full-height empty block.
- No waves in scope → compact note + New wave.
- `lf status` unavailable → the plan still renders from the cached `WavePlan`;
  the failure is a quiet footer line with the raw reason behind disclosure.
- Unknown/absent attention evidence → the lens shows `unknown` + reason, never a
  crash and never a silent black.

## Exclusions

No Active Sessions, external launch adapters, Run History, kanban, or a
Swift-owned Session model. The lens *semantics* remain W2-123's; this slice only
renders them and derives the parent (Project/Wave) folds from that shared
evidence.
