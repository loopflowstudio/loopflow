# Restart Current Step

Restart the currently running step of a wave without restarting the entire flow. Kill the agent, relaunch the same step, continue the flow normally.

## What to build

A "restart step" action that kills the running agent for the current step and relaunches it. The flow continues from that step through the remaining steps. Triggered from the active pill in the Concerto progress UI — no confirmation dialog.

## How it works today

- `stop_wave` kills agents, marks run as Failed, marks wave as Failed/Paused
- `run_wave` creates a NEW WaveRun with `step_index = 0`
- No way to restart mid-flow — you lose all progress

## Restart step behavior

1. Kill running agents (same as stop)
2. Keep the WaveRun alive — status stays `Running`, `step_index` unchanged
3. Re-acquire scheduler slot
4. Re-spawn the run task — executor picks up from the same step_index
5. Flow continues normally after that step completes

## Data structures

No new types. Uses existing `WaveRun`, `WaveRunStatus`.

```rust
// New response DTO
#[derive(Debug, Serialize)]
pub struct RestartStepResponse {
    pub restarted: bool,
    pub wave_id: String,
    pub wave_run_id: String,
    pub step_index: u32,
}
```

## Key functions

### Rust — `restart_step_handler`

```rust
// POST /v0/waves/{wave_id}/restart-step
pub async fn restart_step_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<RestartStepResponse>
```

Logic:
1. Resolve wave_id, get active run (must be Running)
2. `terminate_active_agents` — kill running agent processes
3. End active agents in store (same as stop)
4. Acquire scheduler slot
5. `spawn_run_task_with_slot` with same run (same step_index)
6. Emit `Event::wave_updated`

### Swift — Service layer

```swift
// WaveServiceProtocol
func restartStep(_ id: String) async throws

// LocalWaveService
public func restartStep(_ id: String) async throws {
    // POST /v0/waves/{id}/restart-step
}
```

### Swift — RepoState

```swift
func restartStep(wave: WaveViewModel) async throws {
    try await optimisticAction(wave.id, mutation: { _ in }) {
        try await self.waveService.restartStep(wave.id)
    }
}
```

No optimistic mutation needed — status stays Running.

### Swift — FlowProgressPills

Add restart icon to the active (current) pill:

```swift
struct FlowProgressPills: View {
    let steps: [String]
    let currentIndex: Int
    let startedAt: Date?
    var onRestartStep: (() -> Void)?  // new

    // In stepPill, for isCurrent:
    // Add arrow.counterclockwise icon, tappable
}
```

The restart icon appears inside the active pill, after the elapsed time. Tap triggers `onRestartStep`.

### Swift — WaveDetailPanel

Wire the callback:

```swift
FlowProgressPills(
    steps: wave.flowSteps.isEmpty ? [wave.flow] : wave.flowSteps,
    currentIndex: wave.stepIndex,
    startedAt: wave.activeRun?.startedAt ?? wave.runStartedAt,
    onRestartStep: { restartStep() }
)
```

## Constraints

- Only works on Running runs — returns error for Waiting/Failed/Completed
- Does NOT reset step_index — that's the whole point
- Does NOT create a new WaveRun — reuses existing
- Does NOT touch stimuli — no pause/unpause side effects
- The restart icon only shows on the current (active) pill

## Files to change

| File | Change |
|------|--------|
| `rust/loopflow/src/lfd/http/routes/waves.rs` | `restart_step_handler` |
| `rust/loopflow/src/lfd/http/dto.rs` | `RestartStepResponse` |
| `rust/loopflow/src/lfd/http/mod.rs` | Route registration |
| `swift/LoopflowCore/Services/WaveServiceProtocol.swift` | `restartStep` protocol method |
| `swift/LoopflowCore/Services/LocalWaveService.swift` | HTTP implementation |
| `swift/Concerto/State/RepoState.swift` | `restartStep(wave:)` |
| `swift/Concerto/Views/FlowProgressPills.swift` | Restart icon on active pill |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Wire callback |

## Done when

1. `cargo test --all` passes
2. `swift test --package-path swift` passes
3. Clicking the restart icon on an active pill kills the agent and relaunches the step
4. The flow continues to subsequent steps after the restarted step completes
