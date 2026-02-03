---
status: todo
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

## Approach

Replace the current spinner-and-text running state with a `RunningStateCard` that surfaces:

1. **Step progress**: "Step 2 of 4 · implement" with a segmented progress bar
2. **Elapsed time**: "Running for 3m 42s" updated live
3. **Connect action**: Distinct button to attach to a running wave's terminal without stopping it

The Connect button opens the wave's worktree in an interactive terminal session, allowing the user to observe or intervene while the wave continues.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Show step progress in sidebar row | Less intrusive | Not enough space—row is already dense with status icon, name, iteration, last activity |
| Auto-expand live output for running waves | More immediate visibility | Users already complained about too much output; adds noise for conductors who just want glanceable status |
| Replace Connect with "Observe" read-only mode | Safer, no accidental intervention | lfd already has Connect endpoint that resumes agent state; observe-only would require new protocol work |

## Key decisions

**Segmented progress bar over linear fill.** Flows have discrete steps. A segmented bar (like macOS installer) shows which step is current, not just "how far." This matches the conductor's mental model of "what's running now?"

**Elapsed time, not ETA.** ETAs are unreliable for AI work. Elapsed time gives the same "is this stuck?" signal without promising completion times.

**Connect opens inline Ghostty terminal.** The embedded terminal already exists (`GhosttyTerminalView`). Connect just switches the detail panel to show it with the running session attached. No external window management.

**Minimal backend changes.** The `session.started` event already includes `step_index`. Flow step count can be derived client-side by loading the flow definition. The `/v1/waves/{wave_id}/connect` endpoint exists.

## Scope

**In scope:**
- `RunningStateCard` SwiftUI component with step progress, elapsed time, Connect button
- Wire `step_index` from session events through to Wave model
- Load flow step count from `.lf/flows/` when wave starts running
- Connect button calls existing lfd endpoint and switches to terminal view

**Out of scope:**
- Backend changes to add step count to events (derive client-side)
- Read-only observation mode (full intervention via Connect is fine)
- Sidebar row changes (keep detail panel focused)
- Mobile/remote connect (Phase 2+)

## Done when

```bash
# Verification
1. Running wave shows "Step 2 of 4 · implement" with segmented progress
2. Elapsed time updates every second while running
3. Connect button appears for running waves with waiting agents
4. Clicking Connect opens Ghostty terminal with the running session
5. Wave continues running after Connect (no stop required)
```
