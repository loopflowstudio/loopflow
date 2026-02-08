# Concerto/lfd: Interactive Steps, API Cleanup, Executor Safety

Issues found during manual testing of the wave UI.

## 1. Interactive steps run as auto

**Problem:** StepRunner always submits to lfd via `POST /v1/waves/:id/run`. Interactive steps (design, explore, refine) should launch `lf <step>` in the Ghostty terminal at the wave's worktree path instead.

**What works today:** The old FlowPicker had a manual `isInteractive` toggle. WaveDetailPanel already has `launchInteractive()` which calls `outputBuffer.launchInteractiveSession(...)`. InteractiveSessionView wraps GhosttyTerminalView and runs `lf <step>` in the worktree.

**What's missing:**

The `/v1/flows` endpoint returns step names but not the `interactive` flag. StepRunner has no way to know which steps are interactive.

### Fix

**Backend:** Add `interactive: bool` to the step info returned by `/v1/flows`. The Rust `list_directions` / `list_flows_handler` already loads steps — add the interactive flag to the response DTO.

```rust
// flows.rs response
struct StepSummary {
    name: String,
    interactive: bool,
}
```

**Frontend (Swift):**

1. Add `interactive: Bool` to the `Flow` model (or a separate lookup).
2. In `StepRunner.runFlow()`:
   - If selected step/flow is interactive AND wave has a worktree, call `outputBuffer.launchInteractiveSession(...)` instead of `repoState.runWave(...)`.
   - If interactive but no worktree, show an error ("Create a worktree first" or auto-create one).

**For multi-step flows with interactive steps:** The executor already handles `FlowAction::WaitInteractive` — it pauses and sets the wave to `Waiting` status. The frontend should detect the `Waiting` state and offer to launch the terminal for the current step. This already works via `WaitingStateCard` but needs the terminal launch action wired up.

## 2. Stop wave shelled out to `lfd stop` (non-existent command)

**Problem:** `LocalWaveService.stop()` ran `lfd stop <id>`, but `lfd` has no `stop` subcommand. It fell through to server startup, which failed with "Address already in use."

**Fix:** Already done — changed to `POST /v1/waves/:id/stop` HTTP API.

## 3. Directions not loading after lfd restart

**Problem:** `refreshFlowsAsync()` only ran during `openRepo`. If lfd wasn't ready yet, the `try?` swallowed the error and `availableDirections` stayed empty with no retry.

**Fix:** Already done — added `refreshFlowsAsync()` to the WebSocket `.connected` event handler. Now directions load whenever lfd connects/reconnects.

## 4. Delete wave doesn't cancel executor task

**Problem:** `delete_wave_handler` kills agent child processes via `SIGTERM` and removes the wave from the store. But if the executor's async task is between steps (doing git operations, worktree setup, etc.), it keeps running until it tries to read the deleted wave and gets a `NotFound` error.

**Current behavior:** The executor's `execute()` loop reads the wave from the store at the start, then iterates steps. If the wave is deleted mid-execution, the next `store.update_wave_run()` or similar call will fail with a store error, and the task will end. Not clean, but not dangerous — the worktree is cleaned up by the delete handler.

**Ideal fix:** Add a `CancellationToken` per wave to the executor. When deleting, cancel the token before killing processes. The executor checks the token between steps and exits cleanly. This is a nice-to-have, not urgent — the current behavior is safe enough.

## 5. Dead code removed

- `FlowPicker.swift` — replaced by StepRunner + FlowTypeahead
- `ContextChip.swift` — ContextChip, FileChip, AddFileButton all unused
- `FlowStep` struct in Step.swift — never instantiated
- `Step.hasConfig` — never called
- `StepRun.relativeTime` — never called
- `Flow.stepCount` — never called
- `WaveRun` computed properties (shortId, areaDisplay, directionDisplay, flowDisplay, statusText) — duplicated on WaveViewModel which is what views actually use
