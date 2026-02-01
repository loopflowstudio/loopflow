# Design Review: Concerto Attention UI & Screenshot Pipeline

## What was implemented

1. **Attention badge and section counts** - Added visual indicators to the wave sidebar showing how many waves need attention, with counts on each section header (Needs Attention, Open PRs, Active, Idle).

2. **Screenshot pipeline overhaul** - Replaced the `screencapture`-based system (requiring Screen Recording permission) with in-app view snapshots using `bitmapImageRepForCachingDisplay`. Added UI test screenshot support via XCUITest.

3. **Codex launcher fix** - Changed from deprecated `--dangerously-bypass-approvals-and-sandbox` flag to `--sandbox danger-full-access` for Codex yolo mode.

4. **Persona rename** - Renamed `returner` direction to `listener` for clarity.

5. **Phase 1 backlog items** - Created ordered backlog files in `roadmap/concerto/` covering history/recency, waiting states, running progress, empty states, and quick experiments.

6. **File reorganization** - Moved shared views (AreaPicker, StepRunner, DirectionPills, TransitionBar→NextActionsBar) out of `Views/Improvise/` to `Views/` since they're used across modes.

## Key choices

| Decision | Why | Alternatives rejected |
|----------|-----|----------------------|
| Gold/amber for attention badge | Follows design principle: "attention" is normal workflow, not emergency. Red would create anxiety. | Red notification badges (too alarming) |
| In-app snapshots over screencapture | No Screen Recording permission needed; works in CI headlessly | screencapture (requires permission grant) |
| Separate UI test screenshots | XCUITest captures full screen including system chrome; useful for flow-focused shots | Single method for all screenshots |
| `listener` over `returner` | "Listener" captures the check-in posture better | Kept `returner` (less evocative) |
| Section counts inline | Answers "how many?" without extra UI elements | Badges on section headers (too noisy) |

## How it fits together

```
Screenshot Pipeline
├── Snapshot mode (--snapshot-only)
│   └── SnapshotService.snapshotWindow() → bitmapImageRep
└── UI test mode (--ui-test-only)
    └── ScreenshotPipelineTests → XCUITest screenshot

Wave Sidebar
├── Header badge (attentionCount > 0)
├── Section headers with counts
│   ├── Needs Attention (blockedWaves)
│   ├── Open PRs (prWaves)
│   ├── Active (activeWaves)
│   └── Idle (idleWaves)
└── Progressive disclosure (empty sections hidden)
```

## Risks and bottlenecks

1. **Snapshot timing** - The `ScreenshotWindow` waits 2 seconds for SwiftUI to render. If state setup is slow, the screenshot may show loading state. The `skipBackgroundRefresh` flag prevents mock data from being overwritten by real daemon calls.

2. **UI test reliability** - XCUITest screenshots depend on system state. The test outputs the screenshot path via `print("UI_TEST_SCREENSHOT_PATH=...")` which the Python script parses. If stdout changes format, the pipeline breaks.

3. **Codex flag deprecation** - The `--dangerously-bypass-approvals-and-sandbox` flag may return or Codex may change flags again. Tests verify the current behavior.

## What's not included

- **History/activity timeline** - Backlog item 02 proposes this but implementation is deferred
- **Actionable waiting states** - Backlog item 03; needs deeper integration with PR service
- **Real attention counts from lfd** - Currently counts are computed client-side from wave status; no server-side "attention needed" signal yet
- **Filtering/search in sidebar** - Out of scope per design doc

## Test coverage

- Python tests: 673 passed (including updated Codex launcher tests)
- Swift package tests: 70 passed
- No new UI test coverage for attention counts (view-only change)
