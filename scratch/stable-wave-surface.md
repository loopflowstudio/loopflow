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
   list as their strongest qualities.
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

**Lens semantics are owned by W2-123, not this Task.** This slice ships the lens
*rendering* (`WaveLensView`) plus a `WaveLens {color, reason}` value and a
clearly-marked **provisional** projection from the shared fields above. When
W2-123 lands a shared attention field, only the projection function is replaced;
the rendering and the row stay put. The provisional projection:

- **green** — the Wave has a live body (`live == true` / `running`).
- **red** — no live body but outstanding work remains (`active_tasks +
  active_projects > 0`).
- **black** — off and clean (no live body, no outstanding work).

Every lens carries a `reason` string so VoiceOver names the state, not the color.

## Affected surfaces

- `swift/Loopflow/Models/Wave.swift` — carry the four shared fields (defaults
  keep existing call sites compiling; `Wave` is an app model, not a wire DTO).
- `swift/Loopflow/Services/RegistryQuery.swift` — `WaveSnapshot.toWave()` passes
  the fields through.
- `swift/Loopflow/Models/WaveLens.swift` (new) — lens value + provisional
  projection + colors.
- `swift/Loopflow/Models/WaveViewModel.swift` — expose `lens`, `openTaskCount`,
  `parentWaveId`; drop the pill `statusText`/`statusIndicator` surface use.
- `swift/LoopflowMac/Views/WaveLensView.swift` (new) — the recessed glass lens.
- `swift/LoopflowMac/Views/WaveRow.swift` — lens replaces pill; `indentLevel`.
- `swift/LoopflowMac/Views/WavesView.swift` — repo dropdown replaces the rail;
  single stable alphabetical list; compact empty state.
- `swift/LoopflowMac/PortfolioRepoState.swift` — stable alphabetical sort (drop
  status-priority regrouping).
- `swift/LoopflowMac/Views/WaveDetailPane.swift` — one-sentence objective lead
  with full-objective disclosure; per-Project open-Task count; live-status error
  demoted to a quiet footer; drop "WaveChat" header vocabulary.

## End-to-end proof

Build `LoopflowMac`; run `LoopflowTests`. ViewInspector state tests (the repo's
fixture idiom — it has no image-snapshot harness) cover: lens color for each
provisional state, lens `reason` exposed to VoiceOver, row `indentLevel`
(future-child), compact empty state, one-sentence objective lead, and
per-Project open-Task count. Manually: opening Product shows its objective, all
Projects with KR lists + open-Task counts, and Chat, with no horizontal clipping
at narrow and wide widths.

## Absent / error states

- No repos → compact top-aligned note, not a full-height empty block.
- No waves in scope → compact note + New wave.
- `lf status` unavailable → the plan still renders from the cached `WavePlan`;
  the failure is a quiet footer line with the raw reason behind disclosure.
- Unknown/absent shared fields → provisional projection degrades to
  black+reason, never a crash.

## Exclusions

No Active Sessions, external launch adapters, Run History, kanban, or a
Swift-owned Session model. Lens semantics for Projects/Tasks and the final
green/red/black projection stay with W2-123 — this slice renders the lens and
feeds it a provisional wave-level projection only.
