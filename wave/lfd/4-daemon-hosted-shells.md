---
linear_id: 86838e84-a2d7-41d2-8684-46bc52dd9f04
---
# 03: Daemon-Hosted Shells

**Finish line:** `lfd` owns attachable shells / PTYs in fresh or existing worktrees, and Concerto or SSH-style clients attach to those sessions to run normal `lf` commands by hand.

## Context

Interactive execution should not be a launch-spec shim wrapped around a raw agent command. The real interactive model is a daemon-hosted shell: `lfd` owns the session, clients attach to it, and the terminal shows the same `lf design` or `lf build` command a human would type anywhere else.

This needs to work locally first, but it should be shaped like the long-term remote model. "SSH-like" is the product goal: attach to a real shell in the right worktree, detach, reattach, and keep the run/session relationship intact.

## What to build

1. **PTY/session manager.** Create, attach, read, write, resize, detach, reattach, and close daemon-owned terminal sessions.

2. **Worktree selection.** Support both:
   - fresh shell in a new or prepared worktree
   - attach to an existing shell tied to a live run or workspace

3. **Session identity.** Make terminal sessions first-class records with stable IDs, run/worktree association, lifecycle status, and reconnect-friendly metadata.

4. **Client transport.** Replace launch specs with an attach protocol. HTTP plus websocket streaming is fine if it keeps `lfd` as the process owner.

5. **SSH-style path.** Shape the interface so a future remote client can "ssh into lfd" in product terms even if the first implementation is not literal OpenSSH.

6. **Exit-driven reconciliation.** Session exit should resume, fail, or otherwise advance the relevant run through daemon-owned process observation rather than a shell callback.

## Design guidance from tmux study

This is where the tmux research pays off most directly. Daemon-hosted shells are tmux's core product surface.

### PTY ownership model

tmux allocates PTY pairs with `openpty()`, forks the child process, and keeps the master fd in the server. All I/O routes through the server: read from master → update grid → send to clients. Write from client → send to server → write to master.

`lfd` should follow the same ownership:
- `lfd` owns the PTY master fd
- `lfd` forks `lf <flow-or-step>` as the child process
- Client connections (Concerto, future SSH) read/write through `lfd`, never directly to the PTY
- This is what makes detach/reattach work — the PTY stays open in the daemon regardless of client state

### Attach protocol: structured, not terminal

tmux control mode proved that a rich client doesn't need raw terminal bytes. iTerm2 communicates entirely through commands (`send-keys`, `split-window`, `capture-pane`) and receives structured notifications (`%output`, `%layout-change`). The real terminal rendering happens client-side.

For Concerto + `lfd`:
- **Attach** returns session metadata (ID, wave, run, step, size, status) plus a streaming channel for output
- **Output** can be raw bytes (for embedded Ghostty rendering) or structured events (for non-terminal UI). Start with raw bytes — Ghostty already knows how to render them
- **Input** is keystrokes sent to `lfd`, which writes them to the PTY master
- **Resize** is an explicit message (like tmux's `MSG_RESIZE`), `lfd` calls `ioctl(TIOCSWINSZ)` on the master
- **Detach** closes the streaming channel; session continues running

### Multi-client: size negotiation

tmux's `window-size` option (smallest/largest/latest/manual) is the right abstraction. For `lfd`:
- Default to `latest` — the most recently attached client controls the size
- Mobile + desktop case: desktop attaches at 120x40, mobile at 40x20. Latest-wins means the active device controls. The other sees a viewport.
- Read-only attach (tmux's `-r` flag) should exist from the start — a mobile observer shouldn't accidentally send input

### Scrollback: structured history first

tmux stores full terminal scrollback in server memory (grid arrays). For agent sessions, this is the wrong default — agent output can be enormous and most of it is not useful as raw terminal text.

`lfd` should:
- Keep a bounded scrollback buffer per session (configurable, default modest) for terminal rendering continuity on reattach
- Store structured history in the runtime store (step events, wait points, outcomes) as the durable record
- `capture-pane` equivalent for on-demand snapshots, not continuous full-fidelity persistence
- Mosh's insight applies here: for observers attaching late, sync current state rather than replaying all output

### Ghostty's libghostty-vt

Ghostty's terminal state machine (libghostty-vt) is zero-dependency and runs on WebAssembly with SIMD-optimized parsing. If `lfd` needs to maintain a server-side screen model (for `capture-pane` equivalents, for mobile clients that can't run Ghostty, or for structured extraction), this library is the right foundation rather than building a VT parser from scratch.

### Auth for remote

tmux uses filesystem permissions only. For `lfd` when remote arrives:
- Local: Unix socket permissions (tmux pattern), plus the existing `lfd` token auth
- Remote: TLS + token auth at minimum. WezTerm's TLS domain is a reference — it generates a self-signed CA and client certs per connection.
- The attach protocol should be auth-bearing from the start even if local auth is permissive — retrofitting auth is harder than starting with it

## Open questions

- How much scrollback should survive detach and reconnect during one daemon lifetime? (Guidance: bounded buffer, not unlimited. tmux defaults to 2000 lines per pane. Agent sessions should default lower and rely on structured history for durable context.)
- What auth model should remote or cross-machine attach use? (Guidance: TLS + token. WezTerm's self-signed CA approach for machine-to-machine. The existing `lfd` token infrastructure can extend.)
- When multiple runs exist for a wave, what should the default attach target be for product clients like Concerto? (Guidance: most recent waiting or running session. tmux's `latest` concept — foreground the thing that most recently needed attention.)

## Done when

- `lfd` owns the PTY lifecycle for interactive sessions
- Concerto can attach to a daemon-owned shell without launching a local wrapper command
- Interactive terminals show normal `lf <flow-or-step>` commands
- Detach/reattach works without losing run/worktree/session association
- The same primitive can later back SSH-style access without inventing a second interactive model
