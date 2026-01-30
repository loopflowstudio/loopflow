# Branch Review: jack-heart.concerto-next.20260130_1227.glade-canon

## What was implemented

This branch delivers the **Improvise UX** for Concerto—a guided onboarding flow for new waves that adapts based on wave state. Two major feature areas:

1. **Improvise mode components** — Four new Swift views that guide users through wave setup:
   - `AreaPicker`: Full-panel component for selecting a working area (recent, browse, or infer from diff)
   - `StepRunner`: Step/flow execution UI with direction pills, step grid, and prompt field
   - `DirectionPills`: Inline direction editing with add/remove capsules
   - `TransitionBar`: Sticky footer for transitioning to autonomous mode (set stimulus, create PR, archive)

2. **Local notifications** — `NotificationService` sends macOS notifications when:
   - Wave enters waiting state (interactive step needs input)
   - Wave errors
   - PR is created after a run completes

3. **Python fixes** — `parse_branch_base()` now recursively strips timestamp suffixes (handles nested timestamps from branch naming schema).

## Key choices

**ViewMode state machine in WaveDetailPanel.**

The detail panel now uses a four-state enum (`interactive`, `areaPicker`, `stepRunner`, `conduct`) to determine what to render. Priority order: interactive sessions always win, then area picker if no area, then step runner if idle, otherwise conduct mode. This replaces nested conditionals with a clean switch statement.

**Steps as grid, not dropdown.**

The StepRunner shows common steps (review, design, implement, debug) as a 4-column grid with tooltips. This teaches users what's available—the FlowPicker's dropdown is fine for power users but invisible for discovery.

**Direction as pills.**

Directions are composable (e.g., "product-engineer, security"). Visual pills make this clear. Click pill to remove, click "+" to add from a popover list. This replaces the text field approach which hid the compositional nature.

**Dynamic prompt placeholders.**

The prompt field's placeholder changes based on selected step: "debug" shows "Paste the error message...", "design" shows "What are you trying to build?". This teaches users what kind of input is useful.

**Notifications on status transitions.**

RepoState tracks `previousWaveStatuses` to detect transitions. Only fires notifications on actual state changes, not on every refresh. PR notifications only fire when transitioning from `.running` to `.idle` with a new PR.

## How it fits together

```
User creates wave (sidebar +)
    ↓
WaveDetailPanel.viewMode == .areaPicker (wave.area == nil)
    ↓
User picks area via AreaPicker
    ↓
WaveDetailPanel.viewMode == .stepRunner (wave.area != nil && wave.status == .idle)
    ↓
User picks step, enters prompt, hits Run
    ↓
Interactive session launches OR auto step runs via daemon
    ↓
Step completes → TransitionBar appears with stimulus/PR/archive options
    ↓
User sets stimulus → wave becomes autonomous → viewMode == .conduct
```

The RecentAreasService persists area selections per-repo in UserDefaults (keyed by path hash, max 5 entries). This makes repeated use faster.

## Risks and bottlenecks

**Notification authorization.** The app requests notification permission at launch in ConcertoApp.init(). If denied, notifications silently fail—no fallback currently. Users on older macOS may need to manually enable in System Settings.

**Inferred paths from diff.** AreaPicker calls `getDiffStats("main...HEAD")` which could be slow on repos with large diffs. Currently capped at 3 directories.

**isInteractive check in StepRunner.** The logic `allSteps.first(where: { $0.name == selectedStep }) != nil` treats all steps as interactive. This may need refinement if flows should run differently.

## What's not included

**StepSummaryCard.** The design doc mentions showing context-aware next steps after a step completes. Deferred—the basic TransitionBar shows step count and action buttons, but doesn't suggest what to do next.

**Common directories with file counts.** The design doc suggested showing `src/ (847 files)` in AreaPicker. Not implemented—adds complexity and requires file system scanning.

**Remote execution (Phase 2).** All execution is local. Mobile remote terminal streaming is future work.

## Files changed

| Type | Files |
|------|-------|
| New Swift views | `swift/Concerto/Views/Improvise/{AreaPicker,StepRunner,DirectionPills,TransitionBar}.swift` |
| New Swift service | `swift/LoopflowCore/Services/NotificationService.swift`, `swift/Concerto/Services/RecentAreasService.swift` |
| Modified | `WaveDetailPanel.swift` (ViewMode state machine), `RepoState.swift` (notification transitions), `WaveSidebar.swift` (selectWave notification), `ConcertoApp.swift` (request auth), `Direction.swift` (public init) |
| Python | `src/loopflow/lf/naming.py` (recursive timestamp stripping) |
| Tests | `tests/test_naming.py` (new cases), `tests/test_next.py` (updated mock structure) |
| Design doc | `scratch/concerto-next-improvise-ux.md` (new) |

## Test coverage

- 668 Python tests pass
- 61 Swift tests pass (all suites)
- Swift builds clean with only unhandled file warnings (Info.plist, .icns, .md files)
