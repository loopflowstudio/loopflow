---
linear_id: 1b95c418-7f43-4ecd-96a8-b042e086f858
---
# 01: tmux Architecture Study

**Status: complete.** Design guidance propagated into items 02, 03, 04, and `agent-embedding/06`.

**Finish line:** understand which parts of tmux's server/client architecture should directly inform loopflow's later daemon-hosted shell model, and which parts should stay tmux-specific now that local v0 is shared-store observation plus ordinary local terminals. Keep the long-term target in view: loopflow should become the best way to host SSH-style sessions into coding agents, not just a local terminal embedding trick.

## Verdict

tmux is the right reference for **session identity, detach/reattach, client/server split, and structured attachment protocol**. It is the wrong reference for **execution semantics**. Loopflow should not move flow execution into `lfd` just because tmux owns PTYs.

The clean loopflow shape is:

- `lf` is the execution language
- a shared runtime store is the durable truth
- `lfd` is a host around that truth: scheduler, supervisor, fanout, later shell host
- Concerto and terminals are clients of the same runtime model

The research target is still ambitious: loopflow should become the best way to host SSH-style sessions into your coding agents. Local Ghostty plus shared-store observation is the staircase, not the destination.

## tmux server architecture

### What the server owns

**Everything persistent.** Sessions (red-black tree), windows, panes, PTY file descriptors, child process PIDs, screen/grid buffers (visible + scrollback), paste buffers, global options, key bindings, environment. The client owns only the real terminal TTY, socket fd, and display rendering.

When the last client detaches, nothing happens to server state — the server continues running indefinitely. It exits only when no sessions remain (and `exit-empty` is on) or `exit-unattached` is on.

### Identity model

**IDs are monotonic, never reused, type-prefixed.** Sessions get `$N`, windows `@N`, panes `%N`. IDs survive detach/reattach. The pane ID is exported to child processes as `TMUX_PANE`. This eliminates aliasing bugs and makes the control protocol unambiguous.

Target resolution is hierarchical: `session:window.pane`. Each component resolves via ID, exact name, prefix match, or glob.

### Scrollback

Each pane has a `struct screen` containing a `struct grid` — a contiguous array of `grid_line` structs (not a ring buffer). History occupies indices `0..hsize-1`, visible lines `hsize..hsize+sy-1`. When `hsize` exceeds `history-limit` (default 2000), ~10% of oldest lines are evicted in batch (amortizing `memmove`). PTY kernel buffer is separate and small (4-64KB). When the server exits, all scrollback is lost unless explicitly captured via `capture-pane`.

Alternate screen (vi, less, etc.) does not update history. The base screen's history is preserved and restored when the program exits alternate screen.

### Multi-client size negotiation

The `window-size` option has four modes:
- `smallest` (historical default) — all clients see the smallest size, larger clients get dot fill
- `largest` — smaller clients get scrollable viewport that tracks cursor
- `latest` (current default) — matches most recently active client
- `manual` — fixed size set by `default-size`

Each client can also have an independent `active-pane` selection (`attach-session -f active-pane`) without affecting the window's canonical active pane. This affects cursor position and input routing per client.

Input routing: all non-read-only clients can type. Keystrokes go to the active pane of the current window. The `-r` flag on attach makes a client read-only.

### Socket protocol

Unix domain socket (`AF_UNIX`), stream type. Uses OpenBSD's `imsg` library for framing: length-prefixed messages with fd passing.

**Handshake:** client sends `MSG_IDENTIFY_*` burst (flags, terminal name, TTY name, CWD, features, terminfo, env vars, stdin/stdout fds, PID), ending with `MSG_IDENTIFY_DONE`. Server processes, applies ACL, creates client object, sends `MSG_READY`.

**File I/O tunneling:** `MSG_READ_*` / `MSG_WRITE_*` let the server read/write files through the client's filesystem access — the fd passing makes this possible.

**Authentication:** pure filesystem permissions. Socket directory (`tmux-$UID`) has owner-only access. `server-access` command manages a per-user ACL (`-a` grant, `-d` revoke, `-r`/`-w` read-only/read-write). No cryptographic auth, no tokens. If you can connect to the socket, you're in (subject to ACL).

**Socket recovery:** if the socket file is accidentally deleted, `SIGUSR1` to the server process recreates it.

## Control mode protocol

tmux control mode (`-CC`) is the structured alternative to terminal rendering. A rich GUI client communicates entirely through line-oriented text over stdin/stdout.

### Command-response framing

```
%begin <unix_timestamp> <command_number> <flags>
...output lines...
%end <unix_timestamp> <command_number> <flags>
```

On error, `%end` becomes `%error`. Timestamp, command number, and flags are consistent between opening and closing markers. Command numbers are unique per command, enabling async callback routing. Async notifications never appear inside an output block.

### Async notifications

`%`-prefixed lines that arrive between command response blocks. The full set:

| Notification | Trigger |
|---|---|
| `%output %PANE DATA` | Pane produced output (octal-encoded) |
| `%extended-output %PANE AGE_MS : DATA` | Pane output with flow-control age |
| `%window-add @WIN` | Window linked to session |
| `%window-close @WIN` | Window closed |
| `%window-renamed @WIN NAME` | Window renamed |
| `%window-pane-changed @WIN %PANE` | Active pane changed |
| `%session-changed $SESS NAME` | Attached session changed |
| `%session-renamed NAME` | Session renamed |
| `%sessions-changed` | Session created or destroyed |
| `%layout-change @WIN LAYOUT FLAGS` | Layout modified |
| `%pane-mode-changed %PANE` | Pane entered/exited mode |
| `%client-detached CLIENT` | Client detached |
| `%client-session-changed CLIENT $SESS NAME` | Client switched session |
| `%pause %PANE` | Flow control pause |
| `%continue %PANE` | Flow control resume |
| `%subscription-changed NAME ...` | Format subscription value changed |
| `%paste-buffer-changed NAME` | Paste buffer modified |
| `%message MSG` | `display-message` output |
| `%exit [REASON]` | Client exiting |

### Flow control

`refresh-client -f pause-after=N` replaces `%output` with `%extended-output %PANE AGE_MS : DATA`. If a client falls behind by >N seconds, tmux sends `%pause %PANE` and stops output for that pane. Client resumes with `refresh-client -A '%PANE:continue'`. If a pane falls >300 seconds behind (`CONTROL_MAXIMUM_AGE`), the client is killed.

Without flow control, `%output` is push-based and immediate for all panes in the session. `refresh-client -f no-output` suppresses all output; client uses `capture-pane` on demand instead.

### Format subscriptions

`refresh-client -B name:scope:format` registers a subscription. Server checks at 1-second intervals, sends `%subscription-changed` when values change. Scopes: empty (session), `%N` (specific pane), `%*` (all panes), `@N` (specific window), `@*` (all windows).

### No formal spec

Confirmed by tmux maintainer (GitHub issue #763). The protocol is defined by the source code and iTerm2's implementation.

## How iTerm2 uses control mode

iTerm2 is the canonical (and original) consumer. George Nachman designed control mode for this purpose.

### Architecture

Three core components:
- **TmuxGateway** (`TmuxGateway.m`): line parser via `executeToken()`, command queue with unique IDs, async callback routing
- **TmuxController** (`TmuxController.m`): session state manager. `windowPanes_` maps tmux pane IDs → `PTYSession`. `_windowStates` tracks window state objects. `affinities_` groups window IDs that should share one native window.
- **TmuxWindowOpener** (`TmuxWindowOpener.m`): orchestrates async layout parsing and session creation

### Mapping

- tmux window → iTerm2 tab
- tmux pane → iTerm2 split pane (recursive `NSSplitView`)
- tmux session → gateway session (`TMUX_GATEWAY` mode) + multiple client sessions (`TMUX_CLIENT` mode)

### Attach flow

1. iTerm2 runs `tmux -CC attach -t <session>`
2. tmux sends initial `%begin`/`%end` block, then `%window-add` and `%sessions-changed` notifications
3. For each window, `TmuxWindowOpener` issues coordinated async commands:
   - `capture-pane -peqJ` (history)
   - `capture-pane -peqJ -a` (alternate screen history)
   - `list-panes -t "%wp" -F "..."` (cursor position, mode info)
   - `capture-pane -p -P -C` (pending output)
4. `pendingRequests_` counter tracks outstanding commands. When zero, `requestDidComplete()` calls `loadTmuxLayout()`

### Layout encoding

tmux encodes split hierarchies as compact strings: `1234,120x40,0,0{60x40,0,0,5,59x40,61,0,6}` where `{...}` denotes vertical splits, `[...]` horizontal. `TmuxLayoutParser` recursively produces a dictionary tree for `NSSplitView` construction.

### Resize coordination

`numOutstandingWindowResizes_` prevents feedback loops: incremented before sending `refresh-client -C WxH`, decremented on `%layout-change`. Layout fitting is skipped while resizes are outstanding.

### Tab affinity persistence

Which tmux windows should be tabs in the same iTerm2 window is stored as tmux session variables (`@affinities`) encoded in graphviz DOT format. Survives reconnect.

### Control mode limitations

- No terminal rendering — client gets raw bytes via `%output`, not rendered screen. Use `capture-pane` for current state.
- No interactive UI — copy mode, choose mode, menus not sent to control clients
- No key input routing — stdin is for tmux commands. Use `send-keys` for pane input.
- Size management is manual — client sets size with `refresh-client -C WxH`
- Pane output is all-or-nothing — no per-pane filter (use `no-output` + `capture-pane` on demand)

## Landscape comparison

### WezTerm

The **Domain abstraction** is the standout: local, SSH, Unix socket, and TLS domains all implement the same spawn/pane interface with bidirectional ID mapping across the network boundary. Detachable SSH domains (session outlives the last viewer) is the right lifecycle model for agent sessions. But it's GUI-coupled and forces everything into window/tab/pane spatial hierarchy.

### Mosh

Deepest architectural insight: **state synchronization over byte relay**. Both endpoints hold a screen snapshot; the protocol syncs state, not bytes. UDP with cryptographic session identity enables stateless roaming (single packet from new IP is enough). For agent sessions producing massive output, observers get current state without replaying everything. Limitation: no scrollback, no multiplexing.

### VS Code Remote

Proves server-side execution + client rendering topology. But terminals die on disconnect — they punt to tmux for persistence. Reconnect is brittle (8 retries then give up). Server lifecycle is backwards — starts on connect instead of running independently.

### Zellij

Thread-per-subsystem server, WASM plugin sandbox, serializable session layouts. Client holds no state. But resurrection re-spawns processes (no true persistence) and shared-thread-pool doesn't scale to many independent agent sessions.

### Ghostty

**libghostty-vt** is a zero-dependency terminal state machine that works on WebAssembly with SIMD-optimized parsing. Exactly what a headless daemon needs for terminal emulation if `lfd` needs a server-side screen model. No session persistence or daemon mode by design.

### What none solved

Structured output alongside terminal output. Process-centric identity (not spatial). Selective observation filtering. Multi-modal clients through one session interface. These are `lfd`'s differentiators.

## Mapping tmux concepts onto loopflow

| tmux concept | What it means in tmux | Loopflow equivalent | Copy / adapt / avoid |
|---|---|---|---|
| Server | Long-lived daemon that owns PTYs and session state | Shared runtime store first; `lfd` as supervisor/fanout host; later PTY host | **Adapt** |
| Client | Attached viewer/controller process | Concerto, terminal UI, mobile app, CLI helper | **Copy** |
| Session | Persistent identity that survives detach | `TerminalSession` once PTYs exist; before that, the observed run/session record in the store | **Copy** |
| Window | Top-level screen inside a session | No direct runtime equivalent; compositor concern | **Avoid for now** |
| Pane | Rectangular sub-PTY inside a window | No direct runtime equivalent; compositor policy | **Avoid for now** |
| Socket / control connection | Structured client/server transport | Shared-store writes first; later attach protocol for daemon PTYs | **Adapt** |
| Detach / reattach | Client comes and goes; session survives | Required later for daemon PTYs; not required for local Ghostty v0 | **Copy later** |

## Copy / adapt / avoid

### Copy

**1. Stable identity outlives attachment.** A client is disposable; the session is not. Attachments are ephemeral. Session/run identity is durable. Mobile and desktop can both refer to the same live thing.

**2. Multiple clients are part of the model.** One live session, zero or more attachments, possibly one active input owner at a time. The mobile + desktop case makes this real, not hypothetical.

**3. Structured control beats terminal scraping.** The runtime contract should be structured and eventful — command responses separated from async notifications — not inferred from terminal text.

### Adapt

**4. The "server" is partly a store before it is a PTY host.** The first durable center is the shared runtime store. `lfd` sits around it and adds supervision, launches, fanout, later PTY hosting.

**5. Session is narrower than tmux session.** `WaveRun` = execution unit. `TerminalSession` = live interactive terminal identity. Workspace layout = client/compositor concern. Don't overload one object.

**6. History is structured first, scrollback second.** Durable history is command start/stop, resolved flow/step, wait points, failures. That comes from `lf` writing into the store, not from terminal scrollback.

### Avoid

**7. Do not import windows/panes into the runtime contract.** If Concerto grows split views, that stays a client composition model.

**8. Do not make `lfd` the source of flow semantics.** Running `lf build` directly, from Concerto, or from automation should all mean the same thing.

**9. Do not require daemon PTYs before local usefulness.** Local Ghostty plus shared runtime state first. PTY hosting earns its complexity by solving reconnect, remote access, and multi-client live attachment.

## Recommended staircase

### V0 — shared-store observation

`lf` discovers the shared runtime store and writes structured lifecycle events if available. Manual CLI runs become observable. No daemon execution engine required. "Bring your favorite TUI" becomes true.

### V1 — local Ghostty embedding

Concerto uses the shared store to know what is active and opens ordinary local Ghostty sessions. Embedded local work without fake transport. `TerminalSession` and workspace UI harden around durable identity.

### V2 — automated runs via real `lf`

`lfd` launches normal `lf <flow-or-step>` commands and supervises them against the same store. Manual and automated runs converge. Daemon executor logic shrinks.

### V3 — daemon-owned PTYs

tmux's server/client lessons matter directly: live PTY ownership, attach/read/write/resize, reattach after disconnect, multiple clients per session.

### V4 — remote access

Decide whether remote starts as SSH into host/container or a custom daemon PTY transport. The shared store and CLI contract stay unchanged either way.

The product goal sharpens here: the remote model needs to feel better than "raw SSH plus vibes." Differentiators are durable agent/run identity, structured history, queue/calibration context, and clean reattachment across clients.

## Requirements for the SSH-agent-host future

### Required

**1. Durable identity above the shell.** A human should be able to answer: which wave/run is this? Which agent/session? What was it doing? What happened before I attached? Shell identity must sit under run/session identity.

**2. Reattach across clients.** Desktop and mobile find the same live session. The controlling client may change; session identity does not.

**3. Structured history around the shell.** Start/stop, resolved flow/step, waits/failures, human checkpoints, queue/calibration state. The layer raw SSH does not provide.

**4. Clean "bring your favorite TUI" compatibility.** Running `lf` inside an SSH session should participate in the same runtime model as app-launched or daemon-launched work.

**5. Input ownership without identity confusion.** Multi-attach distinguishes: who is attached, who is allowed to type, what session they are looking at. Can start simple.

### Nice later, not first

- rich shared cursor/presence UX
- durable full-fidelity terminal scrollback
- pane/window topology as part of the runtime contract
- collaborative editing semantics beyond turn-taking or takeover

### Anti-goal

Do not build a generic SSH terminal manager that happens to run coding agents. The value is that loopflow makes agent work legible, attributable, and steerable.

## Implications for agent embedding now

1. **Keep `TerminalSession` as the durable handle.** Queue actions, workspace routing, and foregrounding target session IDs.
2. **Stop deepening the launch-spec shim.** It is a bridge, not the contract.
3. **Keep local terminal embedding local.** Ordinary Ghostty sessions are good enough for the first product win.
4. **Build around observed state, not around who launched the command.** CLI-started, app-started, and later daemon-started work should look like one runtime.
5. **Keep compositor work out of the runtime model.** Tabs, splits, and window composition compose over session identity, not redefine it.

## Open design questions for 02

These belong in the daemon-aware CLI contract next:

1. How does `lf` discover the shared runtime store locally?
2. What is the smallest event schema that can correlate manual CLI runs, app-launched runs, and daemon-launched runs?
3. Which existing `terminal_sessions` fields survive unchanged once the runtime is shared-store-first?
4. Does the first contract need explicit attachment records, or are run/session IDs enough until daemon PTYs exist?
5. What is the minimum auth/safety boundary for local store writes?
