---
status: in-progress
phase: 1
persona: concerto
order: 4
sources: [conductor, improviser, ceo, product-designer]
---

# Show running progress and provide a clear connect path

Running waves show progress via horizontal flow pills with elapsed time.

## What was built

Replaced the spinner-and-text running state with flow pills:

```
✓ design → [implement · 2m] → compress → gate
```

- **Flow pills**: Horizontal breadcrumb showing all steps
- **Current step highlighted**: Accent-filled capsule with elapsed time
- **Completed steps**: Checkmark + muted accent background
- **Future steps**: Surface-colored capsule

## Files changed

| File | Change |
|------|--------|
| `src/loopflow/lfd/daemon/http_server.py` | Added `_get_flow_step_names()`, `flow_steps`, `run_started_at` to API |
| `swift/LoopflowCore/Models/Wave.swift` | Added `stepIndex`, `flowSteps`, `runStartedAt` fields |
| `swift/LoopflowCore/Services/WaveService.swift` | Parse new fields from JSON |
| `swift/Concerto/Views/FlowProgressPills.swift` | New component with accessibility |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Wire FlowProgressPills into running state |

## Key decisions

**Server-side flow parsing.** Step names come from the API rather than parsing YAML client-side. Avoids Swift YAML dependency.

**Elapsed time inline.** Time appears next to the current step name, keeping UI compact.

**Accessibility via combined element.** The pill row is a single accessibility element with label like "Step 2 of 4: implement, 2m".

**Connect unchanged.** Connect button stays in waiting state only (WaitingStateCard). Connect during running would require interrupting the agent.

## Known limitations

**Flow loading on every API call.** `_get_flow_step_names()` loads YAML each time. Acceptable for small flows, could add caching if list operations become slow.

**Long flows may overflow.** No horizontal scroll or truncation. Acceptable for typical 3-5 step flows.

**No per-step timing.** Only run start time is tracked. Per-step duration would need additional backend work.

## Done when

```bash
# Verification
1. Running wave shows flow pills: design → [implement · 2m] → compress → gate
2. Current step highlighted with accent color
3. Elapsed time updates next to current step
4. Completed steps show checkmark icon
5. Live output still appears below pills
```
