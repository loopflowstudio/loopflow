# Running State Progress and Connect

## Problem

Running waves show minimal progress information: a spinner and "Running ship flow..." with no indication of whether the wave is healthy, stuck, or how far along it is. Users cannot quickly assess progress without heavier actions like opening a terminal.

The only actions available are Stop and Clone. There's no lightweight way to inspect what's happening without stopping the wave or opening the worktree in an external tool.

## Approach

Add three improvements to the running state display:

1. **Progress indicator**: Show current step and elapsed time (e.g., "implement (2/4) · 3m 12s")
2. **Activity summary**: Surface the most recent terminal output line as a compact status
3. **Connect action**: Add a button to open the terminal in-app without stopping the wave

### Progress Indicator

Replace "Running ship flow..." with specific progress:

```
implement (2/4) · 3m 12s
└─ Step name, position in flow, elapsed time
```

The daemon already tracks `step_index` and `current_step` on WaveRun. Swift loads the flow definition to get total step count. Elapsed time is computed from `wave_run.started_at`.

### Activity Summary

Show one line of recent output below the progress indicator. This gives users a pulse on what's happening without opening a full terminal. The daemon already buffers output per session.

```
Progress
  implement (2/4) · 3m 12s
  "Reading src/api/auth.py..."
```

Truncate long lines at ~60 characters. Update every 2-3 seconds.

### Connect Button

Add a "Connect" button next to Stop that opens the embedded Ghostty terminal attached to the running process. This uses the existing InteractiveSessionView but without launching a new command—just connecting to the existing PTY.

The daemon's `/v1/waves/{id}/connect` endpoint already exists but isn't used. Wire it up.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Percentage progress bar | Requires estimating step duration | Steps vary wildly in length; would often be misleading |
| Show full terminal by default | More visual noise | Most running waves don't need active monitoring |
| External terminal only | Consistent with current IDE/terminal buttons | Breaks flow; users want quick inspection without leaving app |

## Key decisions

**Show step position, not percentage.** "Step 2/4" is honest about what we know. Percentage bars require duration estimates that would often be wrong.

**One line of output, not a scrolling log.** The live output section already shows more; the activity summary is for at-a-glance status without scrolling.

**Connect attaches to existing process.** Pressing Connect doesn't restart or interfere with the running step. It opens a read/write terminal to the same PTY. User can type commands, but the agent continues running.

**Connect distinct from Stop/Clone.** Connect is inspection, not control. It goes in the action bar but has different visual weight than the destructive Stop button.

## Scope

In scope:
- Progress text with step name, position, elapsed time
- Activity summary (single output line)
- Connect button opening embedded terminal
- Wire up daemon's existing `/v1/waves/{id}/connect` endpoint

Out of scope:
- Sub-step progress (e.g., "analyzing" → "implementing")
- Time remaining estimates
- Multiple terminal sessions per wave
- Push notifications for progress milestones

## Done when

1. Running wave shows "implement (2/4) · 3m 12s" instead of "Running ship flow..."
2. One line of recent output appears below progress
3. Connect button opens terminal without stopping the wave
4. Elapsed time updates live (every second)
