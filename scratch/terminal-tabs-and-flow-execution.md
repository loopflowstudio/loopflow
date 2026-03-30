# Terminal tabs and flow execution from the roadmap

## Context

The roadmap list/detail redesign shipped. The play button currently routes through lfd HTTP (`repoState.ingestAndBuild` → `waveService.run`), which introduces failure modes that don't belong in a human-initiated action: "Network unavailable" (bundled daemon not ready), "wave already at worker capacity" (autonomous pool gate applied to manual runs).

The human clicking play is saying "do this now." That should feel like typing a command, not submitting a job to a queue.

## Design

### Native terminal tabs

Terminal panes currently show one tmux window. Add a native tab bar that reflects all tmux windows in the session. Switching tabs sends `tmux select-window`. Creating a new run creates a new tmux window — the tab bar picks it up.

This gives:
- Flow runs don't hijack the user's shell (window 0 stays clean)
- Multiple concurrent runs are visible as tabs
- Finished runs stay around for inspection
- No session sprawl — everything lives in the wave's tmux session

### Play button wiring

When the user clicks play on a roadmap item:

1. `tmux new-window -t <session>` — clean shell in the wave's session
2. Send `lf <flow> --item <filename>` to the new window
3. Terminal pane's tab bar shows the new window, auto-switches to it

The flow's ingest step (kickoff reads scratch/) handles the rest. No lfd round-trip.

### Flow-level `--item` parameter

Thread the selected roadmap item through the flow engine to the ingest step. Same pattern as existing `--area` / `--direction` overrides. Makes `lf build --item <file>` work from the terminal too.

### Flow selector pane

A multiplexer pane showing available flows and steps for the current wave. The selected flow feeds into what the play button runs. Shared state via a `FlowSelection` environment object (same pattern as `RoadmapSelection`).

### Worker capacity gate

Move the capacity check so it only applies to autonomous loop runs, not human-initiated actions. When `roadmap_item` is present in the run request, skip the worker pool gate. (Rust change drafted and tested in `start_wave_run` — reverted from the roadmap branch, belongs here.)

## Sequence

1. Terminal tab bar (native tabs for tmux windows in a session)
2. Play button rewire (tmux new-window + send-keys instead of lfd HTTP)
3. Flow-level `--item` plumbing (CLI → flow engine → ingest step)
4. Flow selector pane
5. Worker capacity gate fix (autonomous-only)

## Open questions

- Tab naming: derive from the command running in the window, or use a counter?
- Tab lifecycle: auto-close on flow completion, or leave for inspection? Probably leave, with a visual "done" indicator.
- Should the flow selector be a full pane or a dropdown on the roadmap detail pane? Leaning pane — it's a first-class workspace concern.
