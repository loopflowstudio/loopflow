# Running State Progress and Connect - Design Review

## What was implemented

Added three improvements to running wave display:

1. **Progress indicator**: Shows current step name, position in flow, and elapsed time (e.g., "implement (2/4) · 3m 12s")
2. **Activity summary**: Surfaces the most recent terminal output line as a compact status below the progress indicator
3. **Connect button**: Opens a terminal in the worktree directory while the wave runs

## Key choices

**Progress data source**: The daemon's `_wave_to_dict` enriches running waves with `current_step`, `step_index`, `total_steps`, and `run_started_at` from the active `WaveRun`. This keeps the API response self-contained—no additional round trips needed.

**Elapsed time updates via timer**: The view uses a 1-second timer (`elapsedTimeTimer`) that only updates state when the wave is running. This provides live elapsed time without constant API polling.

**Connect opens external terminal**: The design doc envisioned attaching to the running agent's PTY. That requires daemon changes to expose PTY file descriptors and Ghostty support for attaching to existing PTYs. The current implementation opens an external terminal in the worktree directory instead—users can inspect files, run git commands, or observe the worktree while the agent runs. This is documented in `scratch/questions.md` as a known limitation.

**Activity summary from SessionState**: The `recentOutput` method on `SessionState` returns the last output line for a wave, truncated to 60 characters. This uses the existing session output buffer without new daemon APIs.

## How it fits together

```
HTTP API (_wave_to_dict)
  ├── Enriches running waves with WaveRun data
  ├── Loads flow definition for total step count
  └── Returns current_step, step_index, total_steps, run_started_at

Wave model
  ├── Stores running state fields
  └── Provides progressText, elapsedTime, progressDisplay computed properties

WaveDetailPanel
  ├── Timer updates currentTime every second (only when running)
  ├── Displays wave.progressDisplay(now: currentTime)
  ├── Shows sessionState.recentOutput activity summary
  └── Connect button opens external terminal
```

## Risks and bottlenecks

**Flow definition lookup**: `_wave_to_dict` calls `load_flow()` for every running wave. This reads and parses the flow YAML file. For a single running wave this is negligible, but if many waves run simultaneously it could add latency. Consider caching flow definitions if this becomes a problem.

**Connect button limitations**: The design doc intended PTY attachment, but the implementation opens a separate terminal. Users can inspect the worktree but cannot see the agent's actual terminal session. This is documented as an open question.

**Timer memory**: Each WaveDetailPanel creates its own timer. When switching between waves rapidly, old timers should be cleaned up automatically by SwiftUI's view lifecycle, but worth monitoring if memory issues appear.

## What's not included

- **Sub-step progress** (e.g., "analyzing" → "implementing"): Would require agent to emit finer-grained status updates
- **Time remaining estimates**: Steps vary wildly in duration; would often be misleading
- **PTY attachment**: Requires daemon and Ghostty changes; documented in questions.md
- **Multiple terminal sessions**: One Connect button per wave, opens one terminal
- **Push notifications for milestones**: Out of scope for this iteration

## Tests added

Added 9 new tests for the Wave model's running state progress methods:
- `progressText` with and without total steps
- `elapsedTime` formatting (seconds, minutes, hours)
- `progressDisplay` combining progress and elapsed time
