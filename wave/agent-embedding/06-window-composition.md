# 06: Window Composition

**Finish line:** Concerto is a native Swift window compositor for development work. Terminals, chat views, diff viewers, file editors, wave configs — composed into layouts the way tmux composes terminal panes, but with native UI where it matters.

## Context

tmux solves a real problem: manage multiple panes of work in one screen, persist layouts across sessions, switch contexts fast. Developers live in it because nothing else gives that level of control over window composition.

But tmux is limited to terminals. Everything is text. Diffs are text. Chat is text. Config editing is text. A native compositor can do what tmux does and more — native diff views with syntax highlighting, rich chat with inline code blocks, visual wave config editors, file trees with semantic awareness.

The question isn't "can we replace tmux" — it's "what would tmux be if it had access to native UI and understood the development workflow?"

## Feature comparison: tmux vs Concerto compositor

| Capability | tmux | Concerto target |
|-----------|------|-----------------|
| Split panes | Terminal only | Terminal + native views |
| Pane types | All terminals | Terminal, chat, diff, file editor, wave config, block queue |
| Layouts | Manual splits, saved configs | Named layouts per workflow (build, review, tend, debug) |
| Session persistence | Across disconnects | Across app launches, bound to wave state |
| Context switching | `prefix + window number` | Wave-aware — switch wave, layout follows |
| Pane communication | Pipes, copy-paste | Semantic — select text in diff, opens in editor at that line |
| Search | Terminal scrollback | Cross-pane — search finds results in chat, terminal, and files |
| Status bar | Customizable text | Wave status, block count, active flow step |

## What to build

1. **Pane types.** Each pane is a native SwiftUI view:
   - **Terminal** — Ghostty embedded, full terminal emulation
   - **Chat** — Agent conversation, native rendering (not terminal)
   - **Diff viewer** — Side-by-side or unified, syntax highlighted, reviewable (approve/comment inline)
   - **File editor** — Native text editor with syntax highlighting (or embedded editor component)
   - **Wave config** — Visual editor for wave YAML (direction picker, area file browser, flow step list)
   - **Block queue** — The tend flow's human interface (from agent-embedding/01)
   - **Portfolio** — Multi-wave overview (from agent-embedding/03)

2. **Layout system.** Panes compose into layouts:
   - Horizontal/vertical splits, resizable, nestable (like tmux)
   - Named layouts saved per wave or per workflow
   - Default layouts: `build` (terminal + chat + file tree), `review` (diff + terminal + chat), `tend` (portfolio + block queue + calibration), `debug` (terminal + terminal + log viewer)
   - Layout follows wave — switch wave context, panes update to show that wave's state

3. **Wave-aware context switching.** The tmux session concept, but bound to waves:
   - Each wave has a "workspace" — its layout, its terminal sessions, its open files
   - Switch wave = switch everything (like tmux `select-window`, but richer)
   - Multiple waves visible simultaneously in split layouts (the conductor view)

4. **Cross-pane interaction.** Panes aren't isolated — they understand each other:
   - Click a file path in terminal → opens in file editor pane
   - Click a PR reference in chat → opens in diff viewer pane
   - Select a block in queue → terminal pane shows that wave's session
   - Drag a file from file tree → opens where you drop it

5. **Keyboard-driven.** tmux users live on the keyboard. Concerto should be equally fast:
   - Leader key (like tmux prefix) for pane management
   - Pane navigation (hjkl or arrow keys)
   - Quick switcher (fuzzy find waves, files, blocks)
   - All pane operations available without mouse

6. **Session persistence.** Survive app restart:
   - Layout state persisted
   - Terminal sessions reconnect (or show where they left off)
   - Open files and scroll positions restored
   - Wave context maintained

## Research questions

- What terminal embedding options exist for Swift? Ghostty, SwiftTerm, others? Performance and compatibility tradeoffs.
- How do existing native dev tools handle pane composition? (Xcode, Nova, Zed) What works, what's frustrating?
- tmux power users: what do they do that a native app would struggle to replicate? (Remote sessions, scripting, plugin ecosystem)
- How does this interact with existing tmux usage? Can Concerto embed inside tmux, or does it replace it?

## Done when

- At least 3 pane types working (terminal, diff viewer, block queue)
- Layouts are saveable and switchable
- Wave context switching updates all panes
- Keyboard navigation works for all pane operations
- A tmux user can do their core workflow without reaching for tmux
