# 02: Terminal Embedding

## Problem

Concerto now has the right home screen: the attention queue. What happens after the human picks a wave is still wrong.

Today we have two incompatible interaction models:
- `SessionState` / `WaveSessionView` streams parsed session events into a chat-like transcript.
- `InteractiveSession` / `GhosttyTerminalView` can launch one local shell command, but it bypasses `lfd`, hardcodes `lf <step> && lf ops commit --push`, and cannot manage multiple wave-bound sessions.

That leaves the core agent-embedding promise unfulfilled. The conductor can see what needs attention, but the actual coding work still falls back to a chat view or an ad hoc shell. The people who benefit are the conductor running several waves at once and the builder who wants to stay inside one native workspace from queue triage through agent execution.

Why now: wave 01 established `AttentionItem` as the durable human-attention contract. The next architectural chunk is to make the coding surface equally durable: a real terminal, bound to a wave, with context beside it instead of hidden in a separate detail screen. This advances the wave goals “Coding sessions happen in embedded Ghostty terminals” and “Primary Concerto screen is an attention queue, not a chat view” without drifting into the wider compositor work reserved for item 06.

## Approach

Replace both current interactive-session paths with one wave-bound terminal system:

1. **`lfd` owns terminal session intent.**
   - Add a first-class `TerminalSession` domain object in `lfd`, separate from chat-style `Session`.
   - A terminal session records: `id`, `wave_id`, optional `wave_run_id`, `step`, `agent`, `cwd`, `argv`, sanitized env overrides, status (`pending`, `attached`, `running`, `succeeded`, `failed`, `canceled`), and timestamps.
   - For flow steps marked `interactive: true`, the wave executor creates a pending terminal session instead of a structured `SessionManager` session. The wave moves to `waiting` with `terminal_session_id` on the event payload.
   - `lfd` stays the source of truth for launch preparation, exit handling, auto-commit, and run resumption.

2. **Concerto launches the actual agent CLI inside Ghostty.**
   - Add `POST /v0/terminal-sessions` and `POST /v0/terminal-sessions/:id/{attach,start,complete,cancel}` endpoints plus websocket events.
   - `attach` returns a launch spec prepared by `lfd` from the same prompt-building path used by engine runs: `cwd`, `argv`, `env`, wave metadata, and a short-lived completion token.
   - Concerto passes that spec to Ghostty and starts the real harness process (`claude`, `codex`, `opencode`) in interactive mode inside the embedded terminal. No parsed transcript view sits between the agent and the human.
   - When the terminal exits, Concerto reports exit status back to `lfd`; `lfd` then auto-commits dirty worktree state, advances the run on success, or fails the run on non-zero exit.

3. **Ship a repo-scoped terminal workspace, not a single active terminal.**
   - Add `TerminalWorkspaceStore` in shared state for session descriptors, selected tab, and persisted ordering.
   - Keep Ghostty-specific runtime in `Concerto/Platform/macOS/Services/Ghostty/`: one `GhosttyRuntime` per app process, many surfaces keyed by terminal-session id.
   - Replace the singleton `activeSurface` / `activeSessionId` model with per-session surface bookkeeping so multiple sessions can exist concurrently.
   - UI ships as **tabs first**: one tab per active terminal session in the repo window, with stable pane/session ids so item 06 can later promote the same sessions into split layouts without another model rewrite.

4. **Put wave context beside the terminal, not inside it.**
   - The terminal workspace view is a two-column layout: terminal on the left, wave context sidebar on the right.
   - Sidebar sections come from existing stores, not a new dashboard model:
     - wave identity: name, status, branch, worktree, flow step, agent
     - current work item: parsed wave content / roadmap focus for the selected wave
     - queue pressure: unresolved `AttentionItem`s for the same wave plus repo-level counts
     - recent history: latest PR state, commits, and recent runs
     - controls: stop session, mark complete when process is done, jump back to queue, open worktree in IDE/Finder
   - The sidebar updates from existing wave/attention/run events so “I see a problem” to “I’m acting on it” stays at two clicks or less.

5. **Persist and reconnect by terminal-session id.**
   - If Concerto restarts while a terminal session is still pending or running, it reloads `TerminalSession`s from `lfd`, restores tabs, and shows whether the local Ghostty surface must be reattached or the session already ended.
   - This gives us durable session identity now and sets up item 06’s workspace persistence later.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `WaveSessionView` as the primary interactive surface and improve the transcript cards | Reuses existing `SessionState` and SSE plumbing | Fails the wave vision. Parsed transcript UI is exactly the chat-client posture we are trying to leave behind. |
| Keep the current Swift-side `InteractiveSession` command path and make Ghostty nicer | Fastest path to a visible terminal | Hardcodes launch logic in Swift, bypasses `lfd` run state, duplicates prompt assembly, and cannot safely support multiple sessions or run resumption. |
| Build remote PTY streaming in `lfd` before shipping any embedded terminal UI | Would eventually cover local + remote with one transport | Too large for this item. It blocks the local macOS win on a full terminal transport protocol and pushes terminal embedding behind infrastructure work. |

## Key decisions

- **Create a new `TerminalSession` type instead of stretching `Session`.** `Session` is a structured conversation/event model; terminal embedding needs launch specs, attach/completion lifecycle, and real stdio ownership.
- **Use `lfd` to prepare commands, not Swift.** Prompt composition, agent selection, cwd resolution, and resume semantics stay in one place.
- **Treat Ghostty as a process-wide runtime with many surfaces.** Ghostty’s app model is one runtime with one surface per terminal view; our current “single active surface” MVP is the wrong shape for multi-session work.
- **Tabs are the shipping UX; pane identity is the architectural seam.** Tabs satisfy item 02’s “tiled or tabbed” requirement now while leaving room for item 06 to compose the same sessions into native split layouts.
- **Local macOS first.** Embedded terminal sessions require a local process host. Remote repos stay out of scope for this item and must show a clear “terminal embedding unavailable for remote targets” state instead of silently dropping back to chat.
- **Design for wild success:** the conductor can keep three wave-bound terminals open, switch between them instantly, and always see queue pressure and recent PR state without losing terminal fidelity.
- **Design against wild failure:** if we let Swift and `lfd` each invent their own launch/resume rules, the feature will rot into two incompatible session systems. This design prevents that by making `lfd` authoritative.
- **New risk introduced:** local-first terminal ownership creates a temporary product split between local and remote repos. We should name that in the rollout and only close it with a later PTY transport project, not with a hidden fallback.

## Scope

- In scope:
  - `lfd` `TerminalSession` model, storage, HTTP routes, and websocket events
  - executor changes so `interactive: true` wave steps create terminal sessions and resume from terminal completion
  - macOS Ghostty runtime refactor from one active surface to many session-bound surfaces
  - repo-window terminal workspace with tabbed multi-session management
  - wave context sidebar driven by `RepoState`, `AttentionStore`, and `RunStore`
  - persistence/reconnect by terminal-session id for local repo windows
  - tests proving terminal-session lifecycle and multi-session surface ownership
- Out of scope:
  - remote PTY mirroring / terminal streaming over HTTP
  - iOS terminal embedding
  - arbitrary freeform shell tabs unrelated to a wave
  - full split-pane compositor, saved layouts, or cross-pane drag/drop (item 06)
  - replacing chat/session UI for non-coding interactions like calibration notes

## Done when

Run `uv run python scripts/concerto-dev.py run-debug`, start two local waves that pause on interactive steps, and verify all of the following in one repo window:
- each waiting wave opens a separate Ghostty-backed terminal tab bound to that wave
- the selected tab shows wave context, active attention, recent PR/commit history, and quick actions in a sidebar
- switching tabs does not destroy the other terminal surface
- exiting one terminal with status 0 resumes that wave in `lfd`; exiting non-zero marks the run failed
- no coding-session view in the repo window reformats agent output into chat bubbles while the terminal session is active

## Measure (if applicable)

Add `terminal_sessions` telemetry in `lfd` so we can measure adoption and friction.

- Baseline before this work: **0%** of interactive coding sessions complete inside Concerto; all require chat UI or an external terminal.
- Capture after launch:
  - `in_app_rate = completed_terminal_sessions_started_from_concerto / all_interactive_wave_steps`
  - `resume_latency = terminal_session_completed_at -> wave_resumed_at`
- Better looks like:
  - `in_app_rate > 70%` for local interactive wave steps
  - p95 `resume_latency < 2s`
- Verification query once implemented:

```bash
sqlite3 ~/.lf/lfd.db '
  select source, status, count(*)
  from terminal_sessions
  group by source, status
';
```
