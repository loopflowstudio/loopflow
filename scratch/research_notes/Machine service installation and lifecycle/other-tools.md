# Paseo, Happy, and WezTerm: Machine service installation and lifecycle

Research checked 2026-11-03. Scope: official documentation and current public source; no services installed or started. Findings describe current source, which may be ahead of packaged releases. Jack requested learning from comparable tools while designing Loopflow's Desktop/mobile Machine service.

## Does Desktop automatically install a persistent service?

### Takeaway

Paseo proves automatic Desktop bootstrap, but the examined evidence does **not** establish automatic launchd registration. Treat bundling, process startup, survival after app quit, and startup after login/reboot as four separate decisions.

### Cited findings

Paseo's recommended Desktop path bundles its own daemon and starts it automatically. Standalone installation instead uses the npm CLI; its first launch offers relay pairing, with direct TCP/Tailscale/VPN as an alternative. Its state/config root is `PASEO_HOME`, normally `~/.paseo`. Provider CLIs still need installation and authentication. [Getting started](https://paseo.sh/docs)

Desktop's default settings enable built-in daemon management and set `keepRunningAfterQuit: false`. The quit lifecycle consults this setting: enabling it skips the stop. This is a concrete background-survival option, distinct from merely closing a window. [Default settings](https://github.com/getpaseo/paseo/blob/main/packages/desktop/src/settings/desktop-settings.ts), [quit lifecycle](https://github.com/getpaseo/paseo/blob/main/packages/desktop/src/daemon/quit-lifecycle.ts)

The standalone CLI distinguishes background `daemon start` from foreground `daemon run`, returning `already_running` when appropriate. `startDaemonInstance` spawns detached unless foreground is requested. This is process detachment; the inspected implementation does not register an OS service. [CLI start/run](https://github.com/getpaseo/paseo/blob/main/packages/cli/src/commands/daemon/start.ts), [daemon instance implementation](https://github.com/getpaseo/paseo/blob/main/packages/server/src/server/daemon-instance.ts)

WezTerm offers a similarly useful separation: configuring a Unix mux domain and default GUI startup to connect will spawn its server if needed. Its SSH connection can bootstrap a compatible remote mux daemon. Neither fact, by itself, promises installation at login. [WezTerm multiplexing](https://wezterm.org/multiplexing.html)

### Inferences for Loopflow

Provide one Machine executable usable by Desktop and headless installations. Desktop can discover/start it automatically while the persistent-install action separately establishes an OS-managed owner. Avoid inferring launchd installation from another product's word “daemon.” A host can be reachable after Desktop quits yet still disappear after reboot unless login/startup has been configured.

### Gaps

No verified Paseo launchd plist, login registration implementation, or persistent-service uninstall command was found in the inspected paths. Do not claim Paseo has none globally. macOS service installation details should come from the Apple/Rex research.

## What survives quitting and upgrading?

### Takeaway

Paseo separates the supervisor/daemon lifetime from terminal ownership, but explicit shutdown policy still decides whether terminals survive. Detached processes alone do not define the product contract.

### Cited findings

When Desktop stops its owned daemon, Paseo preserves terminals only for app-update or version-mismatch replacement, and never on Windows. Quit, settings changes, host removal and manual stops close terminals. This applies when a stop occurs; `keepRunningAfterQuit` can avoid that stop. Older daemons without preservation support fall back to closing terminals. Desktop captures the instance it launched and does not indiscriminately stop another daemon sharing the home; explicit manual stop checks PID plus start time. [Daemon manager](https://github.com/getpaseo/paseo/blob/main/packages/desktop/src/daemon/daemon-manager.ts)

Paseo's terminal-worker design uses a short-lived launcher so the worker is reparented away from the supervisor tree. A daemon can disappear while PTYs and headless terminal state remain. Reconnection adopts a worker rather than spawning duplicate sessions. Workers exit when idle with no controller, on shutdown/signals, or when their record disappears. Identity includes birth time, marker, protocol version and a private secret. Upgrade policy distinguishes current workers, previous-version draining, older retirement, and newer incompatible workers. [Terminal-worker design](https://github.com/getpaseo/paseo/blob/main/docs/terminal-worker.md)

Happy's new Agent takes a different approach: Desktop downloads a service release, asks the runtime to drain, stops it, and starts the replacement while the window reconnects. Turns resume from SQLite checkpoints after the active tool/inference reaches a saved boundary. “Restart now” interrupts unfinished commands/file edits and records interruption rather than rerunning them. This is durable execution recovery, not transparent PTY survival. [Happy update design](https://happy.engineering/updates/)

### Inferences for Loopflow

Specify independently what happens on window close, Desktop quit, Machine restart, explicit session termination, logout, reboot, upgrade, and uninstall. Preserve existing detached Session owners during a Machine transport restart. Reconnection must not imply rerunning an `lf` command. Happy's checkpoint-and-drain model is useful for native work subscriptions, but copying its harness ownership would contradict Loopflow's CLI/Session-owner boundary.

### Gaps

Paseo's terminal guarantees do not establish provider-agent survival for every stop path. Happy's published update demo is evidence for its own runtime, not a vendor CLI terminal. Neither survives powered-off hardware while continuing computation.

## Can Desktop and mobile share one server contract?

### Takeaway

Paseo is a close reference for moving Desktop onto the same Machine API while retaining a small native platform bridge.

### Cited findings

Paseo Desktop's local transport opens WebSockets through Unix sockets or Windows pipes, and also supports SSH transport. Electron forwards transport events to the renderer. Desktop's bootstrap separately registers windows, dialogs, notifications, external URL opening, editor targets, and browser automation IPC. These are visible platform responsibilities outside the local daemon transport adapter. [Local transport](https://github.com/getpaseo/paseo/blob/main/packages/desktop/src/daemon/local-transport.ts), [Desktop bootstrap](https://github.com/getpaseo/paseo/blob/main/packages/desktop/src/main.ts)

Happy explicitly describes its runtime as the service and the window as its client. Remote installations have their own projects, agents and credentials. Its newer Agent is its own harness; the original CLI's forwarding model is a different product pathway. [Happy updates](https://happy.engineering/updates/), [remote server guide](https://happy.engineering/guides/remote-server/)

### Inferences for Loopflow

A Desktop C-binding-to-socket cutover can establish one Tasks/Sessions/subscriptions contract for Desktop and iOS. Use local IPC for the Desktop connection and the host's Tailscale interface for phone access. Keep opening local windows/editors, native notifications and other UI actions in clients; a headless Machine must remain usable without a logged-in GUI. Device-specific view state should not become Machine domain state. The future webhook hub should consume explicit Machine capabilities, without becoming the default owner of local session processes.

## What makes unattended startup reliable?

### Takeaway

The important engineering is ownership, readiness, environment and version boundaries; making a process start is the small part.

### Cited findings

Paseo reconstructs the login-shell environment for Finder/Dock launches, borrowing the technique from VS Code, so the daemon and children can locate terminal-installed tools. On failure it keeps the inherited environment. [Shell environment implementation](https://github.com/getpaseo/paseo/blob/main/packages/desktop/src/login-shell-env.ts)

Paseo's startup discovers the existing per-home daemon before spawning; waiting for readiness detects an instance replacement. Stop operations check ownership before signaling and distinguish graceful shutdown from forced tree cleanup. This prevents a stale controller from following or killing an unrelated replacement. [Daemon instance implementation](https://github.com/getpaseo/paseo/blob/main/packages/server/src/server/daemon-instance.ts)

Happy's standalone deployment recipe installs a foreground service under systemd with a consistent unprivileged account, explicit HOME/working directory/environment, restart policy and private credentials. It warns not to launch a second daemon alongside the service manager. Readiness requires authenticated health, not just a PID. Maintenance drains admitted work before stopping, but does not promise to finish arbitrary terminal/background jobs. It also distinguishes copied expiring provider credentials from durable unattended authentication. The recipe targets Linux and tells macOS deployments to adapt their service manager. [Standalone deployment](https://happy.engineering/recepies/remote-agent/)

### Inferences for Loopflow

One installation identity and one OS service owner should cover Desktop and CLI starts. Treat running, authenticated, ready, provider-ready and Tailscale-reachable as distinct states visible in diagnostics. Versioned requests and a controlled upgrade boundary prevent Desktop replacement from killing a live execution owner. Preserve user data and credentials when removing service registration; separately define whether uninstall stops active sessions. Test launchd-like minimal environments and provider authentication without assuming interactive shell startup.

### Gaps

No verified uninstall behavior was established for the compared products. These notes propose an explicit Loopflow uninstall contract; they do not attribute it to Paseo or Happy. Published documentation cannot establish every failure mode of Keychain access under macOS background service contexts.
