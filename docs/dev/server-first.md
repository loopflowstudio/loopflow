---
layout: default
title: Server-first Sessions
---

# Server-first Sessions

Planned in LOO-819. None of this is built yet; [Client API server](server.md)
and the [transport README](../../rust/loopflow/src/transport/README.md)
describe what runs today. Each piece below lands as its own Task and updates
those documents when it does. Delete this page when the last piece lands.

How Paseo, T3 Code, Paradev and Rex handle the same questions, and the traps
they hit, is in [Server protocols elsewhere](research/server-protocols.md).

## Why

Every conversation has its own detached owner process holding the agent on a
private PTY, and most have one or two more beside it: a status pipe, a Codex
observer, a Task-update watcher. One Machine measured on November 10, 2026 had
281 `lf` processes using 11.6 GB, 256 of them per-conversation residents, and
73 of its 95 owners were more than a day old. Nothing visible holds those
owners, so nothing shows how many conversations are alive.

## Two kinds of Session

```text
bare                                  managed
lf implement                          lf resume --task LOO-1, Desktop
    |                                     |
    v                                     v
agent, a child in your terminal       view --+
                                      view --+--> Machine server --> holder --> agent on a PTY
                                      view --+        |
                                                      +-- store, hooks, Task updates, steers
```

| | Bare | Managed |
| --- | --- | --- |
| Started by | `lf <skill>`, `lf graph …`, `lf -i` in a terminal | Task and Wave commands, Desktop |
| Holds the process | Your terminal | A holder the Machine server controls |
| Resident `lf` processes | None beyond the launching parent | The server and its holders, shared by every Session |
| Hooks | None | Steers and Task updates in; a wake-up to the server out |
| Views | The terminal it runs in | Any number, in Desktop or any terminal, all live |
| Outlives its terminal | No | Yes |

A Session is the same record in both. What differs is who holds its current
process.

### Bare

`lf` compiles the prompt and context, starts the agent as a foreground child
of the calling terminal, waits, and reads the provider's transcript when the
child exits. It installs no hooks and needs no server. A host that shows the
foreground program, such as Rex, shows the agent.

Persistence, reattachment and status display belong to the terminal. Opening a
bare Session somewhere else starts a new process in the same Session from the
provider's history, exactly as for a conversation started by plain `claude` or
`codex` ([Bring in one you started elsewhere](../sessions.md)). A process
still open elsewhere is not one `lf` stops.

### Managed

The Machine server starts the agent on a PTY kept by a holder. Every place the
Session is opened is a view of that one process: typing in any view shows in
all of them. In a terminal the view is a small `lf` relay, so a host such as
Rex shows `lf` there, not the agent.

- **Size.** The most recently opened view owns the terminal size; other views
  render that grid. When the owner closes, ownership falls back to the next
  most recently opened view. A reconnect is not an open, and a Desktop pane
  counts as open only while it is shown.
- **Input.** Every view can type. There is no controlling view, takeover
  token or Move here.
- **Joining late.** The server keeps the terminal emulator and paints a
  joining view from it. The holder keeps only recent raw bytes. After a server
  restart the emulator is rebuilt from those bytes, or the agent is asked to
  repaint by a size nudge; scrollback from before the restart may be absent.
- **Attach never creates.** A view of a Session whose process is gone says so
  and offers resume.

## Processes

| Process | Restarts | Holds |
| --- | --- | --- |
| Machine server | On every install | The store connection, the work model, hook and client connections, terminal emulators, observation |
| Holder | Never for an upgrade | PTYs, their child agents, a ring of recent output per PTY |
| Graph runner | With its Graph | One running Graph and its headless step children |

An install never ends a conversation. A holder keeps the build it started with
until its Sessions end; new Sessions go to a holder on the current build, so
several holders may be alive at once. The server speaks to older holders, which
keeps the holder's protocol small: start, write, resize, read from a byte
position, report exit.

A holder is started so that stopping or replacing the server's service does not
stop it, and it claims its socket with a lock the kernel releases on death. It
never unlinks a socket path before binding. A dropped connection to a holder
says nothing about its agents.

## Telemetry

The provider's transcript is the record of turns, tools and usage for every
conversation: managed, bare and raw. Two signals sit beside it for managed
Sessions:

- The agent's own status reports in its output (working, blocked on a
  permission or a question) give "waiting on you". The server's emulator reads
  them; no per-pane status process is needed.
- A hook at a turn boundary tells the server to read the transcript now.

Hooks reach the server at its fixed socket path with a short deadline. A hook
that cannot reach it logs a gap and lets the turn continue.

## Pieces

Each is one Task and one PR. The first five do not depend on each other.

| Piece | Removes | Needs |
| --- | --- | --- |
| Bare launches go direct: no owner, agent in the terminal's foreground | Owners and relays for bare runs | |
| Hooks call the server, not the store | Store opens inside turn boundaries | |
| Desktop sends pane status on its server connection | One `session observe-status` per pane | |
| Transcript telemetry, on demand and watched by the server | The telemetry gap bare launches open | |
| Retire leaked processes and find the tests that leak them | Stale owners, test and demo leftovers | |
| Holder: PTYs move from per-Session owners to a holder beside the server | Per-Session owners | The managed rule below |
| Every view types; size follows the newest view | Takeover tokens, Move here | Holder |
| Observers become server registrations | `__codex-observe`, Task-update watchers | Hooks call the server |
| Bare launches drop their hooks | Hook installs for bare runs | Transcript telemetry |
| CLI cleanup: `session serve`, `session attach`, the detach key, `session.end_detached_after` | The owner's surface | Bare direct, holder |

Server operations for these pieces are added under one protocol revision
bump; pieces that add operations take the next revision together rather than
one each.

## The managed rule

A launch is managed when it comes through a Task or Wave command or from
Desktop. A bare `lf` typed inside a Task's checkout stays bare and records the
Task as its ancestry. One function answers this for every launch path.

## Open

- Whether Codex history can be read from its rollout files or needs a short
  app-server call at turn end. If neither works, Codex keeps a recorder.
- Whether the server becomes the only process that opens the store. Hooks and
  views already stop opening it under this plan; Graph runners and one-shot
  commands still do.
- Whether Claude or Codex re-send terminal modes (bracketed paste, focus,
  Kitty keyboard) on resize, which decides what a crashed server's rebuilt
  emulator gets wrong.

## Answered

- **Size nudge.** Measured November 10, 2026 (Claude Code 2.1.322,
  codex-cli 0.167.0, idle and after a 30-line answer): resizing one column
  or row and back repainted every non-blank row of the visible screen and the
  cursor from about 1.5–4 KB of output. Scrollback above the screen is not
  repainted.
- **Holder start.** A `setsid` child of a launchd agent survived
  `launchctl bootout` with and without `AbandonProcessGroup`; the server's
  plist sets it and its systemd unit uses `KillMode=process`. The server
  spawns the holder with `setsid`; no separate service.
