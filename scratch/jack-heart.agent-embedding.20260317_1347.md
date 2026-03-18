# Refactor plan: wave workspace first, embedded terminals additive

## Goal

Make this branch ready for `lf build` by reframing the macOS UI around a primary **wave workspace** instead of a terminal takeover or an edit/settings-style detail view.

The immediate ship goal is simple:

- embedded terminals work
- native chat / TUI-driven sessions remain first-class
- Loopflow tracks whichever surface the human uses
- the selected wave opens a work surface, not a settings surface

"Claude Code" here means the broader class of agent TUIs, not one specific UI.

## Product model

### Primary wave view

A selected wave should open a **workspace** centered on the current work:

- current interactive session when one exists
- roadmap / current item context
- monitoring for the active run
- optional embedded CLI surface

This is the zoomed-in mode.

### Repo-wide command mode

The repo window also needs a zoomed-out mode that shows all active interactive work at once.
That mode is not the main deliverable of this branch. For now the attention queue can remain the stub for that future command surface.

### Settings and editing

Wave configuration is a secondary concern. Rename/reframe the current detail/edit surface into settings/inspector territory later. Do not make it the primary selected-wave experience.

## Scope for this build

### In scope

1. Restore the selected-wave path so the human lands in a wave workspace, not `TerminalWorkspaceView`.
2. Keep native session/chat UI as the default active-wave experience.
3. Preserve embedded terminal plumbing and make it additive:
   - tab
   - pane
   - or experimental toggle
4. Keep terminal session tracking in Concerto so embedded terminals are visible and resumable.
5. Fix attention decoding so the new attention queue / terminal workspace state can load real backend data.
6. Keep the repo-level attention queue as the no-selection / future-command-mode surface.

### Out of scope

- full tmux-like pane management
- user-custom layout persistence
- multi-wave command-grid with live terminal previews
- removing or renaming every existing view in one pass
- redesigning the full wave edit/config flow

## Non-goals

- Do not force users into the embedded terminal.
- Do not remove the native session/chat path.
- Do not require users to change how they use TUIs in order to benefit from Loopflow tracking.
- Do not make the branch depend on the full command-mode design landing now.

## Current problems to correct

### 1. Terminal takeover is too aggressive

`ContentView` currently routes the repo detail surface to `TerminalWorkspaceView()` whenever a terminal session is selected. That makes the embedded terminal feel like a replacement for the main wave UI.

### 2. The current primary selected-wave view is misframed

`WaveDetailPanel` mixes configuration/detail behavior with active work. The selected wave should open a workspace for doing the work.

### 3. Attention decoding is out of sync

Rust emits attention kinds such as:

- `design_review`
- `code_review`
- `calibration`
- `queue_failure`
- `step_failure`

Swift currently expects:

- `interactive_step`
- `algedonic`

That mismatch will keep the attention queue and related workspace surfaces from loading real data correctly.

## Build target

Land a refactor that produces this user experience:

### Selected active wave

The human sees a **wave workspace**.

Default emphasis:
- native session/chat if the wave is interactive
- roadmap and monitoring beside or around it
- optional embedded terminal available without replacing chat

### Selected non-interactive wave

The human sees roadmap + run monitoring + recent activity.

### No wave selected

The human sees the repo-wide attention queue.

## Recommended implementation shape

### Step 1: introduce a wave workspace root

Create a new primary selected-wave container, for example:

- `WaveWorkspaceView`

Responsibilities:
- choose the right primary content for a selected wave
- compose chat/session, roadmap, monitoring, and optional terminal access
- stay execution-first rather than settings-first

### Step 2: restore default routing in `ContentView`

Update `ContentView` so the selected wave routes to the workspace root instead of letting terminal-session selection hijack the whole detail surface.

Desired shape:
- analytics -> analytics view
- selected wave -> `WaveWorkspaceView`
- no selected wave -> `AttentionQueueView`

Do not route the repo detail pane directly to `TerminalWorkspaceView` as the default selected-wave experience.

### Step 3: keep embedded terminal additive

Do not throw away `TerminalWorkspaceView`. Reuse it as a building block.

For this branch, embedded terminal can surface as one of:

- a `CLI` tab inside the workspace
- a collapsible secondary pane
- an experimental toggle shown only when a terminal session exists

Selection and tracking for terminal sessions should stay intact.

### Step 4: keep native session/chat first-class

If a wave is in an interactive step, the native session/chat UI remains the default-safe surface.

Embedded terminal is optional. The human can still use the regular TUI-driven flow and have Loopflow keep track of it.

### Step 5: fix attention kind mapping in Swift

Update Swift models/parsing/UI to understand the backend attention kinds rather than the old placeholder taxonomy.

Minimum requirement:
- real backend attention items decode
- queue/workspace counts are correct
- filtering and labels remain readable

A direct 1:1 mapping is better than semantic collapsing right now.

## Suggested file-level plan

### macOS app routing and composition

- `swift/Concerto/Platform/macOS/Views/ContentView.swift`
  - remove the top-level terminal takeover path
  - route selected waves to a new workspace root
- `swift/Concerto/Platform/macOS/Views/WaveDetailPanel.swift`
  - split execution-workspace concerns from settings/detail concerns
  - keep or extract reusable sections
- `swift/Concerto/Platform/macOS/Views/TerminalWorkspaceView.swift`
  - reuse as a workspace child, not the primary repo detail target
- `swift/Concerto/Platform/macOS/Views/InteractiveSessionView.swift`
  - preserve as first-class execution surface

### shared state and model sync

- `swift/LoopflowCore/Models/AttentionItem.swift`
- `swift/LoopflowCore/Services/LocalWaveService.swift`
- `swift/LoopflowCore/Services/LocalEventService.swift`
- `swift/LoopflowCore/State/RepoState.swift`
- `swift/LoopflowCore/State/AttentionStore.swift`

Goal:
- align attention kinds with Rust
- keep terminal session tracking intact
- keep selected-wave state separate from selected terminal-session state

### backend

No major backend redesign is required for this refactor. Keep:
- `terminal_sessions` routes/store/events
- terminal watcher/resume behavior
- telemetry additions

Backend changes should be limited to small fixes only if the refactor exposes one.

## Decision rules during implementation

- Prefer composition over renaming everything.
- Keep the new terminal infrastructure unless it directly blocks the simpler workspace routing.
- Bias toward the smallest refactor that restores the right product shape.
- If a rename is needed, rename toward `workspace` vs `settings`, not `detail` vs `terminal`.

## Validation

### Manual

Run:

```bash
uv run python scripts/concerto-dev.py run-debug
```

Verify:

1. Selecting a wave opens a work surface, not a terminal-only takeover.
2. Interactive waves still show the native session/chat path by default.
3. If a terminal session exists, the embedded terminal is available as an additive surface.
4. No-wave-selected still lands on the repo-wide queue.
5. Real attention items render correctly from backend data.

### Automated

Run at minimum the Swift package tests that cover touched files. Add/update tests for:

- attention parsing
- workspace routing/state behavior where practical
- terminal session selection/state if the refactor changes it

## Follow-up after this build

Once this lands, the next milestone can target a real command mode:

- all active interactive sessions visible at once
- grouped by wave
- keyboard-first navigation
- zoom in / zoom out between repo command mode and single-wave workspace

That follow-up should build on the same principle:

**one tracking backend, multiple execution surfaces.**
