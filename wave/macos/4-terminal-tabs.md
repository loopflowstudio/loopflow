---
---
# Terminal Tabs and Flow Execution

**Finish line:** Clicking play on a roadmap item opens a new tmux window tab and runs `lf <flow> --item <file>` directly — no lfd HTTP round-trip, no worker-capacity gate blocking human-initiated runs.

## Context

The roadmap list/detail redesign shipped. The play button currently routes through lfd HTTP (`repoState.ingestAndBuild` -> `waveService.run`), which introduces failure modes that don't belong in a human-initiated action: "Network unavailable" (bundled daemon not ready), "wave already at worker capacity" (autonomous pool gate applied to manual runs). The human clicking play is saying "do this now."

## What to build

1. **Native terminal tab bar.** Terminal panes currently show one tmux window. Add a native tab bar reflecting all tmux windows in the session. Switching tabs sends `tmux select-window`. Creating a new run creates a new tmux window — the tab bar picks it up. Flow runs don't hijack the user's shell (window 0 stays clean), multiple concurrent runs are visible as tabs, finished runs stay around for inspection.

2. **Play button rewiring.** When the user clicks play on a roadmap item: `tmux new-window -t <session>`, send `lf <flow> --item <filename>` to the new window, terminal pane's tab bar shows the new window and auto-switches. No lfd round-trip.

3. **Flow-level `--item` parameter.** Thread the selected roadmap item through the flow engine to the ingest step. Same pattern as existing `--area` / `--direction` overrides. Makes `lf build --item <file>` work from the terminal too.

4. **Flow selector pane.** A multiplexer pane showing available flows and steps for the current wave. The selected flow feeds into what the play button runs. Shared state via a `FlowSelection` environment object (same pattern as `RoadmapSelection`).

5. **Worker capacity gate fix.** Move the capacity check so it only applies to autonomous loop runs, not human-initiated actions. When `roadmap_item` is present in the run request, skip the worker pool gate. (Rust change drafted and tested in `start_wave_run` — reverted from the roadmap branch, belongs here.)

## Sequencing

Start with terminal tab bar — it's the visual foundation. Then play button rewire (the main payoff). Then `--item` plumbing (CLI parity). Flow selector pane and capacity gate fix can follow in either order.

## Open questions

- Tab naming: derive from the command running in the window, or use a counter?
- Tab lifecycle: auto-close on flow completion, or leave for inspection? Probably leave, with a visual "done" indicator.
- Should the flow selector be a full pane or a dropdown on the roadmap detail pane? Leaning pane — it's a first-class workspace concern.

## Done when

- Terminal pane shows a tab bar for tmux windows in the session
- Play button creates a new tmux window and runs the flow directly
- `lf build --item <file>` works from the CLI
- Human-initiated runs bypass the worker capacity gate
