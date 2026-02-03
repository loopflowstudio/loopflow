# Flow Progress Pills

Replace text-based running state progress with visual pill indicators showing all flow steps.

## What was implemented

- **FlowProgressPills component**: New SwiftUI view showing flow steps as horizontal pills with:
  - Completed steps (checkmark + accent tint)
  - Current step (solid accent background + elapsed time)
  - Pending steps (surface background)
  - Chevron separators between pills
  - Full VoiceOver accessibility support

- **API change**: `http_server.py` now returns `flow_steps` array (actual step names) instead of `current_step` + `total_steps`. The daemon loads the flow definition and extracts step names.

- **Model cleanup**: Removed `currentStep`, `totalSteps`, and the computed `progressText`, `elapsedTime()`, `progressDisplay()` methods from `Wave`. These are now handled by the dedicated component.

## Key choices

**Step names from daemon**: The daemon resolves `flow_steps` from the flow definition. This keeps the Swift client simple—it just renders what it receives. Alternative was resolving flows client-side, but that would require shipping flow YAML to clients.

**Elapsed time on current step only**: The elapsed time appears inline with the current step pill rather than as a separate element. This keeps the UI compact and associates timing with the active work.

**Pills vs progress bar**: Pills show individual steps by name, which is more informative than a generic progress bar. Users can see what's coming next.

## How it fits together

```
WaveDetailPanel
  └─ runProgressSection
       └─ FlowProgressPills(steps: [String], currentIndex: Int, startedAt: Date?)
            └─ stepPill × N (with isCurrent/isCompleted states)
```

The daemon sends `flow_steps` and `step_index` with each wave. The component renders them as pills with the current one highlighted.

## Risks and bottlenecks

- **Flow resolution on every wave list**: `_get_flow_step_names` is called per-wave in `_wave_to_dict`. For repos with many waves, this could add latency. Current implementation catches exceptions and falls back to `[flow_name]`.

- **Timer fires every second**: The elapsed time timer runs continuously while the view is mounted. This is standard SwiftUI pattern but could be optimized to pause when the view is off-screen.

## What's not included

- **Step-level progress within a step**: The pills show which step is current but not progress within that step (e.g., how far through `implement`).

- **Step duration estimates**: No predicted completion time—just elapsed time.

- **Click-to-jump**: Pills are display-only. Users cannot click to skip to a specific step.
