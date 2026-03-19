# Terminal Embedding: Production-Grade Local Workspace

## Problem

The branch ships the foundational terminal embedding: Ghostty renders inside Concerto, sessions are wave-bound and daemon-persisted, the context sidebar shows wave state, and tabs manage multiple sessions. A conductor can watch agents work.

But the conductor can't *live* in this workspace yet. Sessions depend on completion callbacks that can leave them stuck if the agent crashes. Session creation requires daemon automation — you can't just open a terminal and start working. Keyboard navigation between sessions doesn't exist. Resize handling is one-shot. The workspace is a promising demo, not a daily driver.

The finish line isn't "Ghostty renders" — it's "a conductor runs three agents from Concerto and never reaches for an external terminal." That means the workspace must be robust, fast, and keyboard-first.

## Approach

Harden the existing local-first terminal workspace into production quality across four areas: session lifecycle robustness, user-initiated sessions, keyboard-first navigation, and terminal rendering polish. Stay compatible with the daemon-hosted PTY future (lfd/04) by keeping `TerminalSession` as the stable identity and `lfd` as the source of truth.

### 1. Session lifecycle robustness

**Heartbeat and timeout.** The current completion model relies on a POST callback from a wrapper script. If the agent process crashes, the daemon never learns the session ended. Add a heartbeat:

- Concerto sends periodic heartbeat for each attached session (every 10s) via `POST /terminal-sessions/{id}/heartbeat`
- `lfd` tracks `last_heartbeat_at` on the session record
- A background reaper in `lfd` marks sessions as `Failed` if no heartbeat arrives for 60s and the session is in `Running` state
- When `lfd` later owns PTYs, the heartbeat becomes unnecessary — process exit is observed directly. Shape the heartbeat as a client-health signal, not a permanent architecture

**Graceful exit detection.** When Ghostty's child process exits, Concerto should immediately POST completion with the exit code rather than waiting for the wrapper script. The wrapper script callback becomes a fallback, not the primary path. This means `GhosttyTerminalView` needs an `onExit` callback that bubbles up through the view hierarchy to `RepoState.completeTerminalSession()`.

**Orphan recovery on launch.** When Concerto connects to `lfd`, scan for sessions in `Running` or `Attached` state that have no heartbeat in the last 60s. Offer to cancel or reattach them. This handles the case where Concerto crashed and restarted.

### 2. User-initiated sessions

Today, terminal sessions only appear when a wave run reaches an interactive step. A conductor should also be able to:

- **Start a coding session for a wave.** Select a wave, hit a key, get a terminal in that wave's worktree with the wave's environment. No waiting for automation. The daemon creates a session with source `"user_initiated"` instead of `"wave_step"`.

- **Start a freeform session for a repo.** Not every terminal needs a wave. Open a shell in the repo root for exploratory work. Session is repo-scoped, not wave-scoped.

Implementation:
- New `POST /terminal-sessions` variant that accepts `wave_id` (optional) and `cwd` (defaults to wave worktree or repo root). Daemon creates session, returns ID. No `wave_run_id` required.
- Concerto adds a "New Terminal" action in the wave workspace (keyboard shortcut: `Cmd+T`) and in the sidebar (for repo-scoped sessions).
- User-initiated sessions use the repo's configured agent as argv (e.g., `["claude"]`), or a plain shell if no agent is configured.

### 3. Keyboard-first navigation

The conductor pattern — three agents across three waves — demands fast switching without the mouse.

| Shortcut | Action |
|----------|--------|
| `Cmd+1..9` | Switch to terminal tab by position |
| `Cmd+]` / `Cmd+[` | Next / previous terminal tab |
| `Cmd+T` | New terminal session (for selected wave) |
| `Cmd+W` | Cancel and close current terminal session |
| `Cmd+Shift+]` / `[` | Next / previous wave |
| `Cmd+K` | Quick switcher (fuzzy find across sessions, waves, attention items) |

The quick switcher is the most important piece. It's a Spotlight-style overlay that searches across:
- Active terminal sessions (by wave name, step, agent)
- Waves (by name, status)
- Unresolved attention items (by kind, wave)

Selecting a result navigates to it — switches wave, selects terminal tab, or opens the attention item. One keystroke from "I wonder what's happening" to "I'm looking at it."

### 4. Terminal rendering polish

**Resize coordination.** Current resize handling sets the terminal size once at creation. When the Concerto window resizes or the sidebar collapses, the Ghostty surface must resize and the session's PTY must get a `SIGWINCH`. For the local shim, this means:
- `GhosttyTerminalView` observes `GeometryReader` changes
- On significant resize (>2 char delta), send `POST /terminal-sessions/{id}/resize` with new cols/rows
- `lfd` stores the size (useful for future multi-client negotiation per tmux study)
- Local Ghostty surface handles the actual pty resize via libghostty

**Focus management.** When switching tabs, the newly selected terminal must capture keyboard focus immediately. Current implementation may leave focus in the sidebar or tab bar. `GhosttyMetalView.becomeFirstResponder()` must fire on tab selection.

**Session status indicators.** Tab labels should show:
- Running: wave name + step name + elapsed time
- Succeeded: wave name + checkmark + duration
- Failed: wave name + X + exit code
- Pending: wave name + "waiting..."

Color coding from the design system: `statusSuccess` for succeeded, `statusError` for failed, `statusWarning` for pending, `burgundy` accent for the selected tab.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Wait for daemon-hosted PTYs (lfd/04) before hardening | Clean architecture from day one | Blocks all terminal UX work for months. Local-first embedding is useful now and the session model carries forward. |
| Build split panes now instead of hardening tabs | More spatial flexibility | Premature — composition is item 06 and depends on at least 3 pane types. Tabs are the right unit for the conductor pattern today. Splits promote from tabs later. |
| Replace completion callback with daemon-side process monitoring | More robust lifecycle | Requires lfd to own the process, which is lfd/03-04. Heartbeat + client-side exit detection bridges the gap without deepening the shim. |
| Add tmux-style leader key instead of Cmd shortcuts | Familiar for terminal users | Concerto is a macOS app. Cmd shortcuts are native and discoverable. Leader key belongs in the composition layer (item 06). |

## Key decisions

**Heartbeat is a client signal, not a permanent architecture.** When lfd owns PTYs, it observes process exit directly. The heartbeat exists to bridge the local-shim era. It's shaped as "client reports health" not "client reports process status" — that distinction keeps it compatible with the future model where lfd might want to know which clients are still attached even after it owns the PTY.

**User-initiated sessions are wave-optional.** The wave-binding model is right for automated flows, but a conductor also needs exploratory terminals. Making `wave_id` optional on session creation covers both without a separate "freeform terminal" concept.

**Quick switcher over command palette.** A command palette (VS Code style) is for invoking actions. The conductor needs to navigate to state — "show me wave X's terminal." A fuzzy finder over sessions/waves/attention is the right primitive. Actions come later if needed.

**No layout persistence yet.** Tab order persistence already exists in `TerminalWorkspaceStore`. Split layouts belong to item 06. This design doesn't try to bridge the gap — it makes tabs excellent, and composition promotes from there.

## Scope

In scope:
- Session heartbeat + reaper in lfd
- Client-side exit detection in Ghostty view
- Orphan recovery on Concerto launch
- User-initiated session creation (wave-scoped and repo-scoped)
- Keyboard shortcuts for tab switching + quick switcher
- Terminal resize coordination
- Focus management on tab switch
- Session status indicators in tabs

Out of scope:
- Split pane layouts (item 06)
- Daemon-hosted PTYs / daemon-side process observation (lfd/04)
- Remote terminal transport
- Portfolio view integration (item 03)
- Terminal scrollback persistence beyond what Ghostty manages locally

## Done when

```bash
# Automated
cargo test --all
swift test --package-path swift
uv run pytest python/tests/

# Observable
# 1. Start Concerto, connect to lfd with a configured wave
# 2. Cmd+T opens a terminal in that wave's worktree — no waiting for automation
# 3. Kill the terminal process (kill -9) — session transitions to Failed within 60s
# 4. Restart Concerto — orphaned sessions are detected and cancellable
# 5. Three waves running, Cmd+K → type wave name → enter → looking at that session
# 6. Resize the window — terminal content reflows correctly
```

Advancing wave goals:
- "Coding sessions happen in embedded Ghostty terminals" — from demo to daily driver
- "Percentage of coding sessions that happen inside Concerto vs external terminal (target: >70%)" — keyboard shortcuts and user-initiated sessions remove the reasons to leave
- "Clicks from 'I see a problem' to 'I'm acting on it' (target: <=2)" — quick switcher makes it 1 keystroke
