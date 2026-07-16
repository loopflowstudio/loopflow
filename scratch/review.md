# Simulated review — W2-178 (post-rebase, current code)

Mitchell Hashimoto-style pass over the diff against `origin/main`
(8 commits, 20 files, +1521/−226). Findings recorded before submission.

## The one-screen explanation

A stable Wave surface: one repo dropdown + one alphabetical Wave list, each
row wearing a shared green/red/black/unknown lens. Selecting a Wave opens a
calm workspace — objective lead, Projects with KR lists + open-Task counts,
each Project/Task row wearing the same lens, Chat as the default third pane.
The AttributeGraph cold-launch/sheet cycle is fixed by moving the shared
`TaskTerminalStore` singleton from `@StateObject` to `@ObservedObject` in the
two views that owned it (`WaveDetailPane`, `RoadmapView`).

## Hard questions

- **Maps to the real thing?** Yes. `WaveLens.forTask` spends the Rust-owned
  `TaskAttentionSnapshot` verbatim; `forProject`/`forWave` derive only from
  shared runtime + child attention — no Swift filesystem/status guess. The
  lens grammar is W2-123's; this slice renders it.
- **Breaks at 2 a.m.?** The cycle fix is the canonical SwiftUI pattern for an
  externally-owned singleton; `@ObservedObject` does not create/own, so no
  publisher fires mid-body. Unavailable evidence stays `unknown`-with-reason
  rather than a silent black — logs/VO name the state. The `WaveDetailReading`
  preserves the last good detail on a failed refresh.
- **Shim/compat earning its keep?** No shims. `MockWaveFixture` is
  `AppTestMode`-only (gated, never referenced in production reads) and seeds
  the real `lf status --json` wire shape — it exists to make the populated
  detail render + cycle capture drivable where the registry can't serve, not
  to paper over a format split.
- **Deleting code makes it truer?** The pill surface, status-priority
  regrouping, and tmux-derived authored-Wave color are all gone. Net deletion
  of the old projection.

## Verification (this run, current code, post-rebase)

- `swift test`: **180 tests / 29 suites pass** (build green; ghostty symbol
  warnings are pre-existing linker noise, link completes).
- AttributeGraph zero-cycle, `log stream --process LoopflowMac` (os_log, the
  authoritative source): `--mock` 96 lines / 0, `--sheet` 130 lines / 0,
  default RoadmapView path 95 lines / 0. All six matrix legs (cold launch,
  repo switch, Wave selection, refresh, Chat selection, sheet/dialog) covered
  headlessly.
- Data-layer proof (`WaveDetailReadingTests`): the real `wave_detail.json`
  fixture walks `forProject`/`forTask` — red `waiting for review` wins over
  black, Task lenses verbatim, every lens carries a reason.

## Outcome

No blocking findings. Ready to submit for a human merge.
