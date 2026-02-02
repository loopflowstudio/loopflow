# Running State Progress and Connect

Running waves now show detailed progress information and have a Connect button for inspection.

## What's implemented

1. **Progress indicator**: Current step name, position in flow, elapsed time (e.g., "implement (2/4) · 3m 12s")
2. **Activity summary**: Most recent terminal output line below progress, truncated to 60 characters
3. **Connect button**: Opens external terminal in worktree directory

### Architecture

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

## Known limitation

**Connect opens external terminal, not the agent's PTY.** The design originally called for attaching to the running agent's PTY. That requires:

1. Daemon to track/expose PTY file descriptors for running agents
2. GhosttyKit extension for attaching to existing PTYs (not just spawning new processes)

The current implementation opens a separate terminal in the worktree. Users can inspect files and run git commands while the agent runs, but cannot see the agent's actual terminal output directly.

Future work if needed:
- `/v1/waves/{id}/attach` endpoint returning PTY connection info
- GhosttyKit support for PTY attachment

## Risks to monitor

**Flow definition lookup**: `_wave_to_dict` calls `load_flow()` for every running wave. If many waves run simultaneously, consider caching flow definitions.

**Timer memory**: Each WaveDetailPanel creates its own timer. SwiftUI should clean these up on view lifecycle changes, but worth monitoring.
