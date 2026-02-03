---
status: in-progress
phase: 1
persona: concerto
order: 4
sources: [conductor, improviser, ceo, product-designer]
---

# Show running progress and provide a clear connect path

Running waves should show progress and offer a lightweight way to inspect or intervene.

## Problem

Running state shows a spinner and "Running ship flow" with little progress detail. Users cannot tell if a wave is healthy or stuck, and cannot quickly inspect what's happening without stopping the wave.

The conductor persona needs glanceable progress. The improviser needs to jump in mid-flow. Both are blocked by the current minimal UI.

## Approach (Revised)

Replace the spinner-and-text running state with horizontal flow pills:

```
design → [implement · 2m] → compress → gate
```

1. **Flow pills**: Horizontal breadcrumb-style sequence showing all steps
2. **Current step highlighted**: Visual "we are here" indicator with accent color
3. **Elapsed time**: Shown inline with current step (e.g., "implement · 2m")
4. **Live output**: Kept below pills for visibility into what's happening
5. **Connect**: Only appears in waiting state (existing WaitingStateCard behavior)

## Implementation

### Backend changes (`src/loopflow/lfd/daemon/http_server.py`)
- Added `_get_flow_step_names()` helper to load flow and extract step names
- Added `flow_steps` to wave API response
- Added `run_started_at` for running waves (from latest wave_run)
- Added `step_index` (already on model, now exposed in API)

### Model changes (`swift/LoopflowCore/Models/Wave.swift`)
- Added `stepIndex: Int` field
- Added `flowSteps: [String]?` field
- Added `runStartedAt: Date?` field

### WaveService changes (`swift/LoopflowCore/Services/WaveService.swift`)
- Parse `step_index`, `flow_steps`, `run_started_at` from API response

### New component (`swift/Concerto/Views/FlowProgressPills.swift`)
- Horizontal HStack of step pills with arrows between them
- Current step highlighted with accent color + elapsed time
- Completed steps show checkmark + muted accent
- Timer updates elapsed time every second

### WaveDetailPanel changes (`swift/Concerto/Views/WaveDetailPanel.swift`)
- Replaced `ProgressView() + "Running flow..."` text with `FlowProgressPills`

## Key decisions (revised)

**Horizontal pills over segmented bar.** User preferred pills showing step names rather than abstract segments. "We are here" is clearer when you can see the actual step names.

**Elapsed time inline, not separate.** Keeps the UI compact. Elapsed time appears next to the current step name.

**Small backend change acceptable.** Added `flow_steps` to API response rather than parsing YAML client-side. Cleaner than Swift YAML parsing.

**Connect unchanged.** Connect button stays in waiting state via existing WaitingStateCard. No changes needed.

## Done when

```bash
# Verification
1. Running wave shows flow pills: design → [implement · 2m] → compress → gate
2. Current step highlighted with accent color
3. Elapsed time updates next to current step (e.g., "implement · 2m")
4. Completed steps show checkmark icon with muted styling
5. Live output still appears below the pills
```

## Files changed

- `src/loopflow/lfd/daemon/http_server.py` - API response
- `swift/LoopflowCore/Models/Wave.swift` - Model fields
- `swift/LoopflowCore/Services/WaveService.swift` - Parse new fields
- `swift/Concerto/Views/FlowProgressPills.swift` - New component
- `swift/Concerto/Views/WaveDetailPanel.swift` - Wire up component
