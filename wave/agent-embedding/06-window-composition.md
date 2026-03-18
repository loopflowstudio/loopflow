---
asana_id: '1213718096104955'
linear_id: c41ad0f6-255a-42b6-8aae-43a49ce99263
---
# 06: Window Composition

**Finish line:** Concerto is a native Swift window compositor for development work. Terminals, diff viewers, file editors, wave configs, and queue/portfolio surfaces compose into layouts the way tmux composes terminal panes, but with native UI where it matters.

## Context

The terminal-embedding milestone already shipped the first concrete workspace: repo-scoped Ghostty tabs backed by stable `TerminalSession` ids, with wave context living beside the terminal instead of inside a chat transcript. Window composition should grow from that seam, not restart the workspace model from scratch.

Tmux solves a real problem: manage multiple panes of work in one screen, persist layouts across sessions, switch contexts fast. Developers live in it because nothing else gives that level of control over window composition.

But tmux is limited to terminals. Everything is text. Diffs are text. Config editing is text. A native compositor can do what tmux does and more — native diff views with syntax highlighting, visual wave config editors, file trees with semantic awareness, and queue/calibration surfaces that stay first-class instead of becoming terminal output.

The question isn't “can we replace tmux” — it's “what would tmux be if it had access to native UI and understood the development workflow?”

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
   - Each wave has a workspace — its layout, its terminal sessions, its open files
   - Switch wave = switch everything
   - Multiple waves visible simultaneously in split layouts (the conductor view)

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

## Open questions

- How should saved layouts interact with the current `TerminalWorkspaceStore` ordering and selection model?
- What is the right restore story for running Ghostty surfaces that currently only survive inside one app process?
- Which panes must work for remote repos before remote PTY transport exists, and which stay local-only?
- How much tmux-style keyboard vocabulary helps vs confuses in a native macOS app?

## Done when

- At least 3 pane types working (terminal, diff viewer, attention queue)
- Layouts are saveable and switchable
- Wave context switching updates all panes
- Keyboard navigation works for all pane operations
- A tmux user can do their core workflow without reaching for tmux
