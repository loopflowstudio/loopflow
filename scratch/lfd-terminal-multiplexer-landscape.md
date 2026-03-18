# Terminal Multiplexer Landscape: What tmux Alternatives Got Right and Wrong

Research for the lfd daemon design. Each tool is evaluated through the lens of hosting coding agent terminal sessions, not generic terminal multiplexing.

---

## Zellij

### Architecture

Zellij uses a server/client split similar to tmux but implemented in Rust with a thread-per-subsystem server design. Six dedicated threads communicate via typed message enums through a central bus:

- **Route** thread dispatches client actions to subsystems
- **Screen** thread manages UI state (tabs, panes, rendering)
- **PTY** thread spawns processes and manages lifecycle
- **Plugin** thread runs WASM plugins via wasmi
- **PTY Writer** thread handles async PTY writes
- **Background Jobs** thread for async operations

Clients are stateless and ephemeral -- they capture input and render output but hold no session state. IPC uses Unix domain sockets at `/tmp/zellij-{uid}/{session-name}` with protobuf serialization and length-prefixed framing.

### Session Identity

Sessions are named and identified by their socket path. Clients can create, attach, resurrect, or watch (read-only) sessions. Multiple clients can attach simultaneously with independent terminal sizes.

### Session Resurrection

Zellij serializes session layout (panes, tabs, running commands) once per second to the user's cache folder as human-readable layout files. On resurrection, command panes start suspended with a prompt asking the user to re-run. Optionally serializes pane viewport and scrollback as ANSI files alongside the layout.

The layout-as-serialization-format is clever: sessions become portable, shareable, and human-editable. But it's restoration, not persistence -- the processes die and get re-spawned.

### Plugin System

WASM plugins (wasmi runtime, wasm32-wasip1 target) run sandboxed with explicit permission boundaries. Communication across the WASM boundary uses protobuf. Plugins receive typed events and communicate back via pipe messages. 13 built-in plugins handle UI chrome (tab bar, status bar, file picker). Each plugin instance is pinned to a thread via `plugin_id % num_threads`.

### Web Client

Single web server per machine serves multiple sessions to multiple clients. Translates between browser websockets and Zellij's IPC channels. Reuses the Zellij client code per connection.

### What Zellij Got Right

- **Thread-per-subsystem with typed messages.** Compile-time routing safety. The PTY thread doesn't accidentally receive screen instructions. This matters for a daemon hosting agent sessions -- you want the PTY lifecycle completely isolated from rendering concerns.
- **Stateless clients.** The client is a thin translation layer. This is the right model for agent sessions where the "client" might be a web UI, a mobile app, or another agent. The server holds all state.
- **Session resurrection via serializable layouts.** The layout-as-data model means sessions are inspectable and portable. For agent sessions, serializing the workspace layout (which agents are running, their configurations) as structured data is more useful than preserving raw terminal scrollback.
- **WASM plugin sandbox.** Demonstrates that terminal-adjacent extensibility can be sandboxed effectively. Relevant if lfd ever needs to run user-provided session hooks.

### What Zellij Got Wrong (for agent hosting)

- **Resurrection is re-creation, not true persistence.** Processes die. For agent sessions, you need the process to survive, not just the layout. A coding agent mid-operation can't be "re-spawned from a layout."
- **Thread-per-subsystem doesn't scale to many sessions.** Six threads for the whole server is fine for one user's terminal sessions. A daemon hosting dozens of agent sessions needs per-session or per-agent isolation, not a shared screen thread.
- **Client size mismatch handling is complex.** Multiple clients with different terminal sizes is a real problem for tmux and Zellij. For agent sessions, the agent doesn't care about terminal width -- only the observer does. This simplifies the architecture significantly.

---

## WezTerm Multiplexer

### Architecture

WezTerm's Mux is a global singleton (`Mux::get()`) managing windows, tabs, panes, and domains. The key abstraction is the **Domain** -- a connection context that can be local, SSH, Unix socket, or TLS.

The hierarchy: Windows contain Tabs, Tabs use binary trees of split Panes, Panes implement a trait that abstracts over local processes and remote proxies.

### Domain Model

Four domain types:

- **LocalDomain**: spawns native processes via portable-pty. Includes WSL, ExecDomain (Lua-transformed commands), and Serial variants.
- **RemoteSshDomain**: establishes SSH connections, spawns remote processes, wraps I/O in `WrappedSshPty` for status overlays. Detachable -- when the last pane closes, the domain detaches rather than destroying the session.
- **ClientDomain**: proxies operations to a `wezterm-mux-server` via RPC. Maintains bidirectional ID mappings between remote and local identifiers. Lazy synchronization via `resync()`.
- **TmuxDomain**: created dynamically when tmux sends control mode activation via DCS escape codes. WezTerm can multiplex tmux sessions through its own pane system.

### Identity Model

Atomic counters allocate unique IDs: WindowId, TabId, PaneId, DomainId. In client-server mode, ClientDomain maintains HashMap translations between server-assigned and local IDs, preserving identity stability across reconnections.

### Client-Server Protocol

PDU encoding uses LEB128 variable-length integers for length/serial/discriminant, with bincode-serialized payloads optionally compressed via zstd. Server-side `SessionHandler` tracks per-pane state and pushes dirty line ranges as unilateral PDUs. Client-side uses promise-based RPC with background thread processing. Unilateral messages (serial=0) update local mux state to mirror the server.

LocalPane holds a Terminal for VT parsing and a MasterPty for process I/O with two background threads. ClientPane caches screen state, sends input through `WriteToPane` PDUs, and fetches missing lines on-demand via `GetLines` RPC.

### What WezTerm Got Right

- **The Domain abstraction.** Uniform spawning interface across local, SSH, Unix socket, and TLS connections. The same Pane trait works for local processes and remote proxies. For lfd, the equivalent is: an agent session looks the same whether it's local, in a container, or on a remote host. The domain is the connection context, the pane is the terminal view.
- **Bidirectional ID mapping in ClientDomain.** Local IDs and remote IDs are separate namespaces with explicit translation. This is essential for a daemon where the server assigns canonical IDs but clients need stable local references.
- **Detachable SSH domains.** When the last pane closes, the domain detaches rather than destroying. This is the right lifecycle for agent sessions -- the session outlives any particular viewer.
- **Incremental state sync.** Server tracks per-pane dirty state and pushes only changed line ranges. For agent sessions where most output is irrelevant to the observer, this is more efficient than replaying everything.
- **TmuxDomain via control mode.** Demonstrates that you can wrap an existing multiplexer's sessions through escape code protocol integration, not just replacement.

### What WezTerm Got Wrong (for agent hosting)

- **GUI-coupled architecture.** The Mux singleton sits between terminal emulation and GUI rendering. For a headless agent daemon, you don't want a GUI layer in the dependency chain at all.
- **Everything is a Pane.** The visual metaphor dominates the identity model. An agent session isn't a "pane" -- it's a running process with structured lifecycle metadata. WezTerm's model forces everything into the window/tab/pane hierarchy.
- **No structured output.** The protocol syncs terminal screen state (dirty lines, cursor position). For agent sessions, you want structured events (tool calls, file edits, status changes) alongside or instead of raw terminal output.

---

## VS Code Remote / Cursor

### Architecture

VS Code Remote installs a `vscode-server` process on the remote host. The local VS Code client connects to it and all terminal, file system, and extension operations run server-side. Transport depends on the connection type:

- **SSH**: authenticated SSH tunnel with SOCKS proxy forwarding (localPort -> socksPort -> remotePort)
- **Tunnels**: Microsoft's relay service for connections without SSH
- **Containers**: Docker's exec channel
- **WSL**: random local port

The server provides full IntelliSense, debugging, and terminal hosting. Extensions run on the server, not the client.

### Terminal Sessions

Terminals opened in VS Code run on the remote host via the server. The server allocates PTYs and manages the terminal processes. Communication uses an IPC socket identified by `VSCODE_IPC_HOOK_CLI`.

### Reconnection

Auto-reconnect attempts up to 8 times. After ~30 minutes of inaccessibility, gives up and requires window reload. **Critically: terminals are not persistent across reconnects.** When the connection drops, terminal processes die. The community workaround is to configure VS Code to auto-attach each terminal to an independent tmux session.

This is a known gap: GitHub issue #118031 requests "Keep remote server alive so remote SSH sessions can reconnect without losing work environment in terminals." It remains unresolved.

### What VS Code Remote Got Right

- **Server-side execution model.** Extensions, terminals, and file operations all run where the code is. The local client is a rendering surface. This is the right topology for agent hosting: the agent runs where the code is, observers connect from anywhere.
- **Transport abstraction.** SSH, tunnels, containers, WSL -- different transports, same experience. For lfd, the equivalent: local Unix socket, SSH tunnel, or relay service should all present the same session interface.
- **IPC socket for CLI integration.** `VSCODE_IPC_HOOK_CLI` lets processes inside the terminal communicate back to the host IDE. For agent sessions, a similar mechanism lets the agent's tools communicate with the lfd daemon for structured lifecycle events.

### What VS Code Remote Got Wrong (for agent hosting)

- **No terminal persistence.** This is the fatal flaw. Terminals die on disconnect. For a coding agent that might run for hours, losing the session because your laptop went to sleep is unacceptable. VS Code punts to tmux, which means they haven't solved the problem.
- **Reconnect is fragile.** 8 retries, then give up. Mosh solved this a decade ago with stateless roaming. VS Code's TCP-based reconnect is brittle across network changes.
- **Server lifecycle tied to client.** The vscode-server starts when you connect and (eventually) stops when you disconnect. A daemon hosting agent sessions needs the opposite lifecycle: always running, clients come and go.

---

## Mosh

### Architecture

Mosh replaces SSH's byte-stream relay with the State Synchronization Protocol (SSP). Instead of forwarding every byte from server to client, both endpoints maintain a snapshot of current terminal state, and the protocol synchronizes state, not bytes.

Two SSP instances run in each direction:
- Server -> Client: synchronizes the Screen object (terminal framebuffer)
- Client -> Server: synchronizes user keystrokes as a verbatim transcript

Transport is UDP with AES-128-OCB3 encryption. Authentication uses the initial SSH connection to exchange a session key, then switches to UDP for all subsequent communication.

### State Synchronization vs. Byte Relay

SSH forwards every byte, which means:
- If the network is slow, bytes queue up and Control-C can't get through
- If the network drops, the connection dies
- If the client roams to a new IP, the TCP connection breaks

Mosh's SSP works at the object layer:
- Server can skip intermediate frames, sending only the latest state
- Control-C always works because input is a separate channel
- Network drops don't kill the session because there's no connection to break
- IP changes are handled by the next authenticated packet from any address

### Roaming

Client sends datagrams with increasing sequence numbers plus heartbeats every 3 seconds. When the server receives an authentic packet with a higher sequence number from a new IP, it updates its target address. Single-packet roaming -- one successful packet from the new address is sufficient.

### Prediction Engine

Client predicts that keystrokes echo at the cursor. Predictions display only on high-delay connections. When the server confirms the prediction, it becomes real. When predictions are wrong (escape sequences, carriage returns), the engine resets. Four modes: always, never, adaptive, experimental.

### What Mosh Got Right

- **State sync over byte relay.** This is the single most important architectural insight for agent session hosting. An agent's terminal output is 99% noise (build logs, test output, verbose tool calls). Synchronizing "what's on screen now" is orders of magnitude more efficient than replaying every byte. For lfd, the lesson is: don't stream raw PTY output to observers. Maintain authoritative state and sync it.
- **UDP with stateless roaming.** Session identity is cryptographic (session key), not topological (TCP connection). The session survives network changes because identity doesn't depend on IP:port pairs. For mobile observers watching agent sessions, this is essential.
- **Separate input and output channels.** User input and terminal output are independent SSP streams. For agent sessions, this maps to: agent input (tool invocations, commands) and agent output (terminal state, structured events) are separate synchronization objects.
- **Frame rate control.** SSP can throttle synchronization to avoid filling network buffers. For agent sessions producing high-volume output (large builds, test suites), this prevents the observer's connection from becoming the bottleneck.

### What Mosh Got Wrong (for agent hosting)

- **No scrollback.** Mosh synchronizes only the visible terminal screen. For agent sessions, you need history -- the observer should be able to scroll back through what the agent did. Mosh explicitly punts this to tmux/screen.
- **Requires mosh-server on remote.** Custom binary on both ends. For lfd, this is actually fine (we control both ends), but it's a deployment friction that SSH avoids.
- **No multiplexing.** One mosh session = one terminal. No tabs, splits, or session grouping. For agent hosting, you need to group related terminals (agent + build + test) under a single session identity.
- **Terminal-width coupling.** Mosh synchronizes a fixed-size framebuffer. Resize requires re-sync. For agent sessions, the agent process doesn't care about terminal width, but Mosh's model assumes it does.

---

## Ghostty

### Architecture

Ghostty is built around **libghostty**, a C-ABI compatible library written in Zig that provides terminal emulation, font handling, and rendering. The macOS app (Swift/AppKit/SwiftUI) and Linux app (Zig/GTK4) are both thin consumers of this library.

The library is being decomposed into a family of smaller libraries:
- **libghostty-vt**: zero-dependency (not even libc) library for parsing terminal sequences and maintaining terminal state (cursor position, styles, text wrapping). Available now for Zig and C, compatible with macOS/Linux/Windows/WebAssembly.
- Future: input handling, GPU rendering, GTK widgets, Swift frameworks

libghostty-vt provides SIMD-optimized parsing, Unicode support, Kitty Graphics Protocol, and tmux control mode compatibility.

### Multiplexing and Session Persistence

Ghostty provides built-in tabs and splits but **no session persistence**. When windows close, processes die. The application can run headless (`--initial-window=false --quit-after-last-window-closed=false`), but there's no reattachment mechanism.

The community discussion identifies three distinct problems:
1. **Reattachment**: keeping processes alive when windows close
2. **Restoration**: persisting layouts and scrollback to disk
3. **Management UI**: fuzzy-finder for switching sessions

As of early 2026, the maintainer has acknowledged the request but committed to no roadmap. The community tool **zmx** uses ghostty-vt to hold terminal state and scrollback, sending a terminal snapshot to stdout on reattach.

### What Ghostty Got Right

- **libghostty-vt as an embeddable primitive.** A zero-dependency terminal state machine that works on WebAssembly. This is exactly what a headless agent daemon needs: accurate terminal emulation without any GUI or rendering dependencies. Parse the escape sequences, maintain the screen state, done.
- **Library-first architecture.** The terminal emulator is a library consumed by platform-specific apps. For lfd, this means: the PTY hosting and terminal state management should be a library, and the daemon, CLI, and Concerto should be consumers.
- **Separation of concerns.** Terminal emulation (libghostty-vt) is cleanly separated from input handling, rendering, and platform integration. Each layer can be used independently.
- **zmx as existence proof.** The community-built zmx tool demonstrates that ghostty-vt can hold terminal state for headless sessions, with snapshot-based reattachment. This is close to what lfd needs for agent session hosting.

### What Ghostty Got Wrong (for agent hosting)

- **No session persistence by design.** Ghostty's philosophy is "we're a terminal emulator, not a multiplexer." For agent hosting, you need both: the terminal emulation AND the session lifecycle management. Ghostty provides only the former.
- **No daemon mode.** Despite being able to run headless, there's no built-in mechanism for the headless process to accept new client connections. The "headless Ghostty" is a party trick, not an architecture.
- **GUI-first identity.** Sessions are windows. There's no concept of a session that exists independently of a window. For agent hosting, the session IS the agent process; windows are ephemeral observers.

---

## Synthesis: What Matters for Agent Session Hosting

### The Right Architecture Combines Ideas From Multiple Tools

| Concern | Best Model | Source |
|---|---|---|
| Server/client split | Stateless clients, stateful server | Zellij |
| Connection abstraction | Domain model (local/SSH/socket/TLS) | WezTerm |
| State synchronization | Object-level sync, not byte relay | Mosh |
| Session identity | Cryptographic, not topological | Mosh |
| Terminal emulation | Embeddable library, no GUI deps | Ghostty (libghostty-vt) |
| Session lifecycle | Detachable domains that outlive clients | WezTerm |
| Workspace serialization | Layout-as-data for inspection/portability | Zellij |
| Server-side execution | Everything runs where the code is | VS Code Remote |

### What None of Them Solved

1. **Structured output alongside terminal output.** Every tool treats the terminal as an opaque byte stream or framebuffer. Agent sessions produce structured events (tool calls, file edits, test results) that should be first-class, not scraped from terminal text.

2. **Process-centric identity.** Every tool's identity model is spatial (window/tab/pane) or connection-oriented (session/domain). An agent session's identity should be the running process and its purpose, not its position in a layout.

3. **Selective observation.** All tools sync the full terminal state. An agent session observer might want only structured events, or only the last N lines, or only "interesting" output. The sync protocol should support filtering.

4. **Multi-modal clients.** tmux/Zellij assume terminal clients. WezTerm assumes a GUI client. VS Code assumes VS Code. A daemon hosting agent sessions needs to serve terminal clients, web UIs, mobile apps, and other agents through the same session interface.

### Key Design Principles for lfd

1. **Library-first terminal emulation.** Use or build something like libghostty-vt for PTY hosting and state management. Don't couple to any GUI or rendering layer.

2. **State sync, not byte relay.** Borrow Mosh's insight: synchronize terminal state objects, not byte streams. Observers get the current state efficiently without replaying history.

3. **Structured events as first-class.** Terminal output is one channel. Structured lifecycle events (agent started tool X, edited file Y, test Z passed) are a parallel channel. The daemon produces both.

4. **Session identity = process identity.** A session is an agent run, identified by run ID, not by window position or socket path. The session exists because the process exists. Clients attach and detach; the session doesn't notice.

5. **Domain-style connection abstraction.** Local Unix socket for development. SSH tunnel for remote hosts. Relay service for mobile. Same session interface regardless of transport.

6. **Stateless clients, stateful daemon.** The daemon owns all session state. Clients are rendering surfaces that can be replaced, duplicated, or destroyed without affecting the session.

---

Sources:
- [Zellij Client-Server Model (DeepWiki)](https://deepwiki.com/zellij-org/zellij/2.1-client-server-model)
- [Zellij Session Resurrection](https://zellij.dev/documentation/session-resurrection.html)
- [Zellij Session Resurrection Announcement](https://zellij.dev/news/session-resurrection-ui-components/)
- [Zellij Web Client Blog](https://poor.dev/blog/building-zellij-web-terminal/)
- [WezTerm Multiplexer Architecture (DeepWiki)](https://deepwiki.com/wezterm/wezterm/2.2-multiplexer-architecture)
- [WezTerm Domains and Panes (DeepWiki)](https://deepwiki.com/wezterm/wezterm/2.2.1-domains-and-panes)
- [WezTerm Multiplexing Docs](https://wezterm.org/multiplexing.html)
- [VS Code Remote SSH](https://code.visualstudio.com/docs/remote/ssh)
- [VS Code Server](https://code.visualstudio.com/docs/remote/vscode-server)
- [VS Code Terminal Persistence Issue #118031](https://github.com/microsoft/vscode/issues/118031)
- [Mosh Homepage](https://mosh.org/)
- [Mosh Architecture (DeepWiki)](https://deepwiki.com/mobile-shell/mosh)
- [Ghostty About](https://ghostty.org/docs/about)
- [Libghostty Is Coming (Mitchell Hashimoto)](https://mitchellh.com/writing/libghostty-is-coming)
- [ghostty-vt PR #8840](https://github.com/ghostty-org/ghostty/pull/8840)
- [Ghostty Session Manager Discussion #3358](https://github.com/ghostty-org/ghostty/discussions/3358)
