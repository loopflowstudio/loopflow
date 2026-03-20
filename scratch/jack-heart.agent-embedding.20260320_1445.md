# Wave Workspace Redesign

**What to build:** Replace the scroll-view detail panel with a multiplexer-based wave workspace. Cmd+K palette becomes the primary navigation surface. New pane types surface wave content (roadmap, readme, runs, launcher).

## What changed from the beat synthesizer

The beat synthesizer concept dissolved during design. The sequencer grid was solving a problem that doesn't exist — the real primitives compose without it:

- Tempo → crons on waves (list of scheduled flows, deferred to chord-model wave)
- Priority → queue ordering in attention view
- Conducting → unblocking interactive flows
- VSM autonomy → cron + always-running workers, no beats

## The shift

Current: sidebar selects a wave → detail panel shows a vertical scroll of sections (step runner, goals, scratch doc, commits, diff, roadmap, ops actions).

New: sidebar selects a wave → multiplexer layout fills the detail area with panes. Each pane shows one concern. Keyboard-navigable (Cmd+K to open/switch, focus movement between panes). Layout persists per wave via MultiplexerStore.

## Scope (this commit)

### In

- **Kill `WaveDetailPanel`** — `ContentView` shows multiplexer for the selected wave instead of the scroll-view detail panel
- **New pane types** — `roadmap`, `readme`, `runs`, `launcher` added to `PaneType`
- **Pane views** — SwiftUI views for each new pane type, built from data that already exists:
  - `roadmap`: reads from `WaveContent` (existing markdown roadmap items)
  - `readme`: reads from `WaveContent.vision` / wave README
  - `runs`: reads from `RunStore` (active runs, worktrees, branches, PRs)
  - `launcher`: flow picker + run button (replaces `StepRunner` header widget)
- **Expand command palette** — add wave switching (global) and pane open/focus (scoped to selected wave) to `CommandPalette`
- **Default layout** — `MultiplexerStore` generates a default layout when a wave has none:
  ```
  ┌──────────────┬──────────────┐
  │   roadmap    │   terminal   │
  │              │              │
  ├──────────────┤              │
  │    runs      │              │
  └──────────────┴──────────────┘
  ```
- **Focus-not-create** — palette focuses existing pane of that type if one is open, creates by splitting if not

### Deferred

- `crons` pane and `workers` config — needs data model changes in Rust/Python (`lfd`), belongs in chord-model wave
- `roadmap` hitting PM API directly (Linear/Asana) — show existing `WaveContent` roadmap items for now
- Sidebar collapse/hide — keep sidebar as-is, revisit after living with the new layout
- `launcher` vs `launchpad` merge — keep both for now, `launchpad` is terminal-context, `launcher` is wave-context

## Data structures

```swift
// PaneType gains new cases
public enum PaneType: String, Codable, Sendable {
    case terminal
    case markdown
    case diff
    case launchpad
    case roadmap
    case readme
    case runs
    case launcher
}
```

No new models needed — all pane views read from existing stores (`WaveContent`, `RunStore`, `WaveViewModel`).

## Key functions

```swift
// MultiplexerStore — default layout for waves without one
func defaultLayout(for wave: WaveViewModel) -> LayoutNode

// MultiplexerStore — find pane by type in a wave's layout
func pane(ofType: PaneType, for waveId: String) -> PaneState?

// CommandPalette — build scoped action list
func paletteActions(waves: [WaveViewModel], selectedWave: WaveViewModel?, multiplexerStore: MultiplexerStore) -> [PaletteAction]
```

## Constraints

- Multiplexer currently requires a worktree path for terminal panes. Waves without worktrees need to handle this — non-terminal panes should work regardless, terminal panes show "no worktree" placeholder.
- `WaveContent` is loaded lazily on wave selection — pane views that depend on it need to handle the loading state.
- Interactive sessions (attention items that need unblocking) should still take over the view when active — this behavior from the current detail panel needs to be preserved.

## Done when

- Selecting a wave in the sidebar shows a multiplexer layout, not the scroll-view detail panel
- Cmd+K can switch waves and open/focus panes by name
- `roadmap`, `readme`, `runs`, `launcher` panes render with real data
- Keyboard navigation between panes works (already does via multiplexer)
- Existing terminal pane behavior is preserved
- Interactive sessions still surface when a wave is waiting for input
- `swift test --package-path swift` passes
