---
linear_id: ff6c534b-b5ff-4bf4-a2c4-07ff4239ee0e
---
# 02: Daemon-Hosted Shells

**Finish line:** `lfd` is a network-facing server in front of tmux. Clients (Concerto, SSH-style, mobile) attach to tmux sessions through `lfd`. tmux owns PTYs and local session state. `lfd` owns auth, transport, multi-client negotiation, and structured observation.

## Context

The one-session-per-run model ships first: `lfd` creates one tmux session per wave run, launches `lf <flow>` inside it, and monitors the session. The CLI handles all flow sequencing in-process. Interactive and headless steps both run behind the same tmux session.

This item extends that to a full daemon-hosted shell experience. `lfd` sits in front of tmux as the server that remote and local clients connect through. tmux continues doing what it's good at — PTY ownership, session persistence, scrollback. `lfd` adds what tmux can't — network access, auth, multi-device negotiation, structured event routing.

## What to build

1. **`lfd` as tmux frontend.** `lfd` talks to tmux via control mode (`tmux -C`) or command interface. Clients never touch tmux directly. `lfd` translates between its attach protocol and tmux operations (`send-keys`, `capture-pane`, `resize-window`).

2. **Attach protocol over WebSocket.** Extend the existing WebSocket event stream to carry session I/O:
   - **Attach** returns session metadata (ID, wave, run, size, status) plus a streaming channel
   - **Output** is raw terminal bytes from tmux (Ghostty renders client-side)
   - **Input** is keystrokes routed through `lfd` to `tmux send-keys`
   - **Resize** triggers `tmux resize-window` on the target session
   - **Detach** closes the channel; tmux session continues

3. **Worktree selection.** Support both:
   - fresh shell in a new or prepared worktree
   - attach to an existing shell tied to a live run

4. **Multi-client size negotiation.** `lfd` mediates window size across attached clients:
   - Default to `latest` — most recently attached client controls
   - Read-only attach for observers (mobile watching a desktop session)
   - `lfd` calls `tmux resize-window` when the controlling client changes

5. **Auth.** tmux uses filesystem permissions only. `lfd` adds:
   - Local: existing `lfd` token auth (tmux socket stays filesystem-protected)
   - Remote: TLS + token. WezTerm's self-signed CA approach for machine-to-machine.
   - Auth-bearing from the start, even if local auth is permissive

6. **SSH-style product path.** "Attach to a shell in the right worktree" should feel like SSH in product terms. `lfd` is the sshd, tmux sessions are the shells.

## Why tmux stays

tmux is battle-tested infrastructure for exactly this job — PTY allocation, session persistence, scrollback management, process supervision. Replacing it with direct `openpty()` calls means reimplementing all of that. The things tmux can't do (network transport, auth, multi-device, structured events) are exactly what `lfd` adds.

The boundary is clean:
- **tmux owns**: PTY master fds, child process lifecycle, scrollback buffers, local session state
- **`lfd` owns**: network transport, auth, client management, size negotiation, structured event routing, run/wave/session correlation

## Scrollback

Agent output can be enormous. tmux handles scrollback natively, but `lfd` should configure it conservatively:

- Set tmux `history-limit` low per session (e.g. 2000 lines)
- Structured history in the runtime store (step events, outcomes) is the durable record
- `tmux capture-pane` for on-demand snapshots
- Late-joining clients get current screen state (Mosh's insight), not full replay

## Open questions

- What tmux interface works best for `lfd`? Control mode (`-C`) gives structured notifications but adds complexity. Direct commands (`tmux send-keys`, `tmux capture-pane`) are simpler but poll-based. Start with direct commands, move to control mode if latency matters.
- When multiple runs exist for a wave, what should the default attach target be? Most recent running session — foreground whatever most recently needed attention.

## Done when

- Concerto can attach to a run's tmux session through `lfd` without touching tmux directly
- Remote clients can attach over WebSocket + TLS
- Detach/reattach works across client disconnects
- Multi-client size negotiation works (desktop + mobile)
- The run/worktree/session relationship stays intact across attach cycles
