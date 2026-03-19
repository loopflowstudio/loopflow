---
asana_id: '1213718096104955'
linear_id: c41ad0f6-255a-42b6-8aae-43a49ce99263
---
# 03: Window Composition

**Finish line:** Concerto is a native Swift window compositor for development work. Terminals, diff viewers, file editors, wave configs, and queue/portfolio surfaces compose into layouts the way tmux composes terminal panes, but with native UI where it matters.

## Context

The terminal-embedding milestone shipped the first workspace: `WaveWorkspaceView` routes to native work view by default, with an additive terminal tab backed by stable `TerminalSession` ids and `TerminalWorkspaceStore`. Wave context lives beside the terminal instead of inside a chat transcript. `TerminalWorkspaceStore` persists tab ordering and selection per repo, and `lfd` exposes attach/start/cancel endpoints plus durable `terminal_sessions` rows. Window composition should grow from that seam — promoting the existing tabbed model into split layouts — not restart the workspace model from scratch. It also needs to stay compatible with the transport upgrade tracked in `wave/lfd/`: grow around session identity and workspace state, not around today's local launch-spec shim.

Tmux solves a real problem: manage multiple panes of work in one screen, persist layouts across sessions, switch contexts fast. Developers live in it because nothing else gives that level of control over window composition.

But tmux is limited to terminals. Everything is text. Diffs are text. Config editing is text. A native compositor can do what tmux does and more — native diff views with syntax highlighting, visual wave config editors, file trees with semantic awareness, and queue/calibration surfaces that stay first-class instead of becoming terminal output.

The question isn't "can we replace tmux" — it's "what would tmux be if it had access to native UI and understood the development workflow?"

## Feature comparison: tmux vs Concerto compositor

| Capability | tmux | Concerto target |
|-----------|------|-----------------|
| Split panes | Terminal only | Terminal + native views |
| Pane types | All terminals | Terminal, diff, file editor, wave config, attention queue, portfolio, calibration |
| Layouts | Manual splits, saved configs | Named layouts per workflow (build, review, tend, debug) |
| Session persistence | Across disconnects | Across app launches, bound to wave + terminal-session state |
| Context switching | `prefix + window number` | Wave-aware — switch wave, layout follows |
| Pane communication | Pipes, copy-paste | Semantic — select text in diff, opens in editor at that line |
| Search | Terminal scrollback | Cross-pane — search finds results in terminal, queue, and files |
| Status bar | Customizable text | Wave status, attention pressure, active flow step |

## What to build

1. **Pane types.** Each pane is a native SwiftUI view:
   - **Terminal** — promote the existing Ghostty / `TerminalSession` integration from tabs to panes; no freeform shell model separate from `lfd`
   - **Diff viewer** — side-by-side or unified, syntax highlighted, reviewable
   - **File editor** — native text editor with syntax highlighting (or embedded editor component)
   - **Wave config** — visual editor for wave YAML (direction picker, area file browser, flow step list)
   - **Attention queue** — the conductor's human checkpoint surface
   - **Portfolio** — multi-wave overview
   - **Calibration** — structured chord review view

2. **Layout system.** Panes compose into layouts:
   - Horizontal/vertical splits, resizable, nestable (like tmux)
   - Named layouts saved per wave or per workflow
   - Default layouts: `build` (terminal + files + queue context), `review` (diff + terminal), `tend` (portfolio + attention queue + calibration), `debug` (terminal + terminal + log viewer)
   - Promote the existing tabbed terminal workspace into split layouts instead of replacing it with a second workspace stack

3. **Wave-aware context switching.** The tmux session concept, but bound to waves:
   - Each wave has a foreground workspace — the selected run, its layout, its terminal session, its open files
   - Switch wave = switch the foreground run context for that wave
   - Multiple waves visible simultaneously in split layouts (the conductor view)
   - Background runs for the same wave remain real; the compositor is choosing what to foreground, not asserting exclusivity in the daemon

4. **Cross-pane interaction.** Panes aren't isolated — they understand each other:
   - Click a file path in terminal → opens in file editor pane
   - Click a PR reference in queue/review UI → opens in diff viewer pane
   - Select a block in queue → terminal pane shows that wave's session
   - Drag a file from file tree → opens where you drop it

5. **Keyboard-driven.** tmux users live on the keyboard. Concerto should be equally fast:
   - Leader key (like tmux prefix) for pane management
   - Pane navigation (hjkl or arrow keys)
   - Quick switcher (fuzzy find waves, files, attention items)
   - All pane operations available without mouse

6. **Session persistence.** Survive app restart:
   - Layout state persisted
   - Terminal panes restore around existing terminal-session ids; do not invent pane-local terminal identity
   - Open files and scroll positions restored
   - Wave context maintained
   - Foreground-run selection is restored as product state, without collapsing multiple runs into one daemon concept

## Design guidance from tmux study

### What to borrow from tmux

**Layout as data, not just view state.** tmux encodes split hierarchies as compact strings (`120x40,0,0{60x40,0,0,5,59x40,61,0,6}`) that serialize the full tree. iTerm2 parses these to build its `NSSplitView` hierarchy on attach. Concerto should similarly serialize layout trees — not just tab order, but split directions, sizes, and pane types — so layout persistence and wave-context switching are data operations.

**Session identity above compositor identity.** tmux sessions have stable IDs; windows and panes are subordinate. Concerto's composition should work the same way: `TerminalSession` IDs and wave/run IDs are the stable anchors. Pane positions and split ratios are compositor state that can change without affecting session identity.

**Independent active-pane per client.** tmux lets each attached client independently select which pane is active. In a Concerto split layout, this means focus/selection is a per-view-instance concern, not a session-level concern. Two split terminal panes showing different sessions can each have independent focus.

### What to avoid from tmux

**Pane as runtime identity.** tmux pane IDs are runtime primitives — they gate input routing, size negotiation, and process lifecycle. Concerto panes should be pure compositor constructs. The runtime primitives are `TerminalSession` and `WaveRun`. If Concerto creates a split view showing two terminals, that's two pane views wrapping two session references, not two new runtime identities.

**Universal terminal assumption.** tmux's every pane is a terminal. Concerto's value is that panes can be native views (diff, queue, calibration) that understand their content semantically. Don't force native panes through a terminal abstraction.

### iTerm2 as reference client

iTerm2's tmux integration is the most mature example of a rich native GUI consuming tmux sessions. Key patterns:
- **Tab affinity** — which tmux windows group into one native window is persisted as tmux session variables and restored on reconnect. Concerto needs equivalent persistence for which sessions/views group into a layout.
- **Outstanding-resize counter** — prevents feedback loops during resize. Concerto will face the same problem when split panes resize affects terminal sessions.
- **Pending-request watermark** — iTerm2 tracks outstanding async commands and delays UI construction until all respond. Concerto should similarly batch initial state loading before rendering a layout.

### Mosh insight for composition

Mosh's state-sync model (sync screen snapshot, not replay bytes) is relevant for composition. When switching wave context, Concerto doesn't need to replay terminal history — it needs the current screen state. If `lfd` later exposes a `capture-pane` equivalent (current terminal grid as text), layout switching can show instant content without streaming replay.

## Open questions

- How should saved layouts interact with the current `TerminalWorkspaceStore` ordering and selection model? (Guidance: layouts should reference sessions by ID, and `TerminalWorkspaceStore` remains the authority for per-repo session ordering. Layouts compose over that — they describe spatial arrangement, not session lifecycle.)
- What is the right restore story for running Ghostty surfaces that currently only survive inside one app process? (Guidance: once `lfd` owns PTYs (`wave/lfd/` item 04), Ghostty views just reattach to daemon sessions on restore. Before that, local Ghostty processes die with the app — persist the layout tree, recreate shells on restart.)
- Which panes must work for remote repos before remote PTY transport exists, and which stay local-only? (Guidance: queue, portfolio, calibration, and diff panes work for any repo with an `lfd` HTTP connection. Terminal panes need either local Ghostty or daemon PTY transport. Keep the pane type roster split along that line.)
- How much tmux-style keyboard vocabulary helps vs confuses in a native macOS app? (Guidance: leader-key + hjkl for pane navigation is natural for tmux/vim users. But Concerto is a macOS app — Cmd-based shortcuts should also work. Offer both, don't force one.)

## Done when

- At least 3 pane types working (terminal, diff viewer, attention queue)
- Layouts are saveable and switchable
- Wave context switching updates all panes
- Keyboard navigation works for all pane operations
- A tmux user can do their core workflow without reaching for tmux
