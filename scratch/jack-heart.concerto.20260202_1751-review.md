# Flow Progress Pills — Design Review

This branch adds visual progress indicators for running waves, replacing the minimal spinner-and-text UI with horizontal flow pills that show step progression and elapsed time.

## What was implemented

**Backend (Python):** Added `flow_steps` and `run_started_at` to the wave API response. The `_get_flow_step_names()` helper loads the flow YAML and extracts step names. Running waves include the start time from the latest wave_run record.

**Model (Swift):** Added `stepIndex`, `flowSteps`, and `runStartedAt` fields to the Wave model. These are parsed from the API response in WaveService.

**UI (Swift):** New `FlowProgressPills` component renders a horizontal breadcrumb-style sequence:
- Completed steps: checkmark icon + muted accent background
- Current step: accent-filled capsule + elapsed time (e.g., "2m")
- Future steps: surface-colored capsule

The component uses a 1-second timer to update elapsed time display.

## Key choices

**Server-side flow parsing.** Step names come from the API rather than parsing YAML client-side. Keeps Swift code simple and avoids adding a YAML parsing dependency.

**Elapsed time inline.** Time appears next to the current step name rather than separately. Keeps the UI compact and associates duration with the running step.

**Accessibility via combined element.** The pill row is combined into a single accessibility element with a descriptive label like "Step 2 of 4: implement, 2m". Arrow separators are hidden from screen readers.

**Timer lifecycle.** The timer runs continuously via `autoconnect()`. For this use case (running waves are actively monitored), this is acceptable. The view is typically short-lived and dismissed when the wave completes.

## How it fits together

```
Wave API                    Swift Model                  FlowProgressPills
────────                    ───────────                  ─────────────────
flow_steps: [String]   →    flowSteps: [String]?    →   steps parameter
step_index: Int        →    stepIndex: Int          →   currentIndex parameter
run_started_at: ISO    →    runStartedAt: Date?     →   startedAt parameter
```

WaveDetailPanel's `runProgressSection` passes these three values to `FlowProgressPills` when the wave is running. For completed/error states, the original status indicators remain.

## Risks and bottlenecks

**Flow loading on every API call.** `_get_flow_step_names()` loads and parses the flow YAML each time `_wave_to_dict()` is called. For list operations with many waves, this adds overhead. Mitigated by the fact that flows are small YAML files and Python's file system caching.

**Timer memory.** The Timer.publish autoconnect pattern keeps the timer running while the view exists. Not a concern for typical usage but could matter if many FlowProgressPills views are created simultaneously.

**Long flow names.** Very long step names or flows with many steps may overflow horizontally. The current design doesn't scroll or truncate. Acceptable for typical 3-5 step flows.

## What's not included

**Connect button for running waves.** The design doc originally mentioned connect-while-running. This branch keeps connect in waiting state only (via WaitingStateCard). Connect during running would require interrupting the agent, which has UX complexity.

**Detailed step timing.** Each step's duration isn't tracked—only the run start time. Per-step timing would require additional backend work (step_started_at records).

**Animation on step transition.** Steps complete abruptly without transition animation. Could be added later if the visual feels jarring.

## Files changed

| File | Change |
|------|--------|
| `src/loopflow/lfd/daemon/http_server.py` | Added `_get_flow_step_names()`, `flow_steps`, `run_started_at` to API |
| `swift/LoopflowCore/Models/Wave.swift` | Added `stepIndex`, `flowSteps`, `runStartedAt` fields |
| `swift/LoopflowCore/Services/WaveService.swift` | Parse new fields from JSON |
| `swift/Concerto/Views/FlowProgressPills.swift` | New component with accessibility support |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Wire FlowProgressPills into running state |
| `scratch/concerto-running-state-progress-and-connect.md` | Design doc (moved from roadmap/) |
