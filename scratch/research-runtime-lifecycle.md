# Runtime lifetime and installation for mobile

2026-11-03 · Research for Jack Heart · Proposed architecture, grounded in current source

**Recommendation: install a private Machine service independently of Desktop, while retaining execution in independently owned Session processes.** Service uptime, live execution and durable conversation history are three different promises. Host Tailscale supplies reachability; it supplies none of those promises. The optional public hub should eventually deliver events to this Machine boundary, without becoming its execution supervisor.

## What exists, and what would change

Current Desktop is already a client of shared Rust reads, writes and subscriptions through [RegistryQueryLocal.swift](../swift/LoopflowMac/Services/RegistryQueryLocal.swift). Its [DesktopServer.swift](../swift/LoopflowMac/DesktopServer.swift) is an application-lifetime, same-user Unix-socket automation API. It is not the proposed remote server. Running work belongs to detached owners in [transport.rs](../rust/loopflow/src/transport.rs), which retain PTYs, provider processes and capture independently of terminal viewers.

Those owners call `setsid()`, redirect logs and expose local sockets. Closing a viewer does not close execution. However, the spawning process remains their parent until it exits: a waiting thread reaps the child. Therefore neither “detached” nor “separate process” proves survival under a supervisor that kills descendants. This is the first boundary to test before building mobile UI.

The [lfd history](lfd-history.md) records both a general execution server and a later webhook ingress daemon. Each accumulated responsibilities before deletion. Earlier native launchd deployment already addressed Docker environment friction, and a thin argv adapter already existed. The new distinction must be observable: restarting the network service leaves execution running, and neither its memory nor a hub owns the authoritative Work model. Repeated attempts establish sustained product demand as well as difficult operating boundaries.

## Two architectures worth comparing

**One persistent server owns every provider and PTY.** Installation and health checks are simple; termination has one clear scope. But a server crash, restart or update interrupts every live turn unless additional runtime machinery is introduced. Persisting chat history makes restart recoverable, not uninterrupted. This is reasonable for a disposable compute worker; it is a poor default for a phone briefly attaching to existing work.

**Persistent network adapter plus detached execution owners.** The service authenticates connections and calls existing operations; owners retain terminal state and execution. Clients and adapter can reconnect independently. Costs are explicit owner discovery, compatibility, bounded retained output and cleanup. Much of this already exists locally. Prefer this architecture, proving the isolation instead of adding another parallel Session manager.

[Paseo’s terminal worker](https://github.com/getpaseo/paseo/blob/main/docs/terminal-worker.md) separates PTYs from its daemon. A short-lived launcher reparents the worker because the supervisor kills by parent-PID tree. A lost controller leaves terminals running; reconnection replays retained screen state. Its compatibility policy adopts a previous-version worker for draining but can terminate substantially older workers. Useful lesson: survival has both an OS-process boundary and a protocol-version boundary. Its documented process-tree solution does not establish Linux cgroup isolation.

[WezTerm](https://wezterm.org/multiplexing.html) distinguishes ordinary local UI-owned terminals from Unix/SSH multiplexing domains backed by a separate server. Remote attachment can start the server and connect to its Unix socket; compatible client/server versions are required. Borrow the explicit distinction between connecting and creating execution, and the ability to recover a viewer without constructing a new session.

## The service-manager boundary is real

On Linux, systemd defaults to `KillMode=control-group`: stopping a unit kills remaining processes in that unit’s cgroup. `setsid()` and reparenting do not move a process into another cgroup. The current spawn path, placed inside a service unchanged, would therefore fail adapter-restart survival. Avoid solving this with `KillMode=process` or `none`, which systemd discourages because processes escape lifecycle/resource management. [systemd.kill](https://www.freedesktop.org/software/systemd/man/latest/systemd.kill.html)

**Proposed Linux implementation:** run the adapter as a user service and start execution owners in separate transient user units. `systemd-run` supports transient services, started by the service manager, and scopes that retain the caller’s environment; these have different startup and waiting semantics. Select the mechanism with a small prototype, preserving today’s ready acknowledgement, environment and Process identity. Never report launch success merely because the manager accepted a job. [systemd-run](https://www.freedesktop.org/software/systemd/man/latest/systemd-run.html)

For unattended Linux, enable user lingering as an explicit installation mode: the user manager then starts at boot and remains after logout. This does not resurrect a process across reboot or manufacture credentials. [loginctl](https://www.freedesktop.org/software/systemd/man/latest/loginctl.html)

On macOS, the installed `launchd.plist(5)` manual says ordinary job-exit cleanup targets the job’s process group; `AbandonProcessGroup` disables that behavior. Current `setsid()` separates that group, but this is not proof against logout, bootstrap-domain removal, updater cleanup or explicit recursive kill. Test the actual installed job. A per-user LaunchAgent is a login-session service; Apple documents termination at logout. Do not market it as boot-time unattended hosting. [Apple launchd guide](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)

## Installation should expose two operating modes

**Personal Mac:** Desktop offers “Allow connections while Loopflow is closed,” registering the service with `SMAppService`. LaunchAgent registration bootstraps it now and on subsequent logins; registration is per user and subject to approval. System Settings can disable it, so show actual registration/running state. A LaunchDaemon has an administrator-approval and boot-time lifecycle instead. [SMAppService.register](https://developer.apple.com/documentation/servicemanagement/smappservice/register())

**Standalone host:** install the same service without requiring the Desktop UI. Apple’s own sample uses a GUI-less signed application bundle and command-line registration, so `SMAppService` need not mean running Desktop. A conventional CLI-installed LaunchAgent is another packaging option, but should resolve to the same service identity and implementation. Choose one authoritative installation per Machine; app and CLI must not start rival listeners. [Apple package sample](https://developer.apple.com/documentation/ServiceManagement/updating-your-app-package-installer-to-use-the-new-service-management-api)

For a Mac that must work before login or after logout, a separately approved system-domain job running as the selected account is a different support tier. Defer that promise until credential behavior is demonstrated. Linux user service plus lingering is the simpler initial always-on host. Neither requires building the public hub.

## Survival contract

The following is the **proposed target**, not a claim that the network service exists. “Continue” assumes required provider services and tools remain available.

| Event | Adapter / phone | Live execution | Durable history |
|---|---|---|---|
| Close Desktop window | Unaffected | Continues | Retained |
| Quit Desktop application | Independent installed service remains | Continues; app-contributed tools may disappear | Retained |
| Phone backgrounds, disconnects or loses Tailscale | Connection ends; reconnect later | Continues; phone tools unavailable | Retained |
| Adapter crashes/restarts | Temporarily unreachable | Continues only after OS isolation is proved | Retained |
| Adapter updates | Reconnect and negotiate capabilities | Existing owners drain under compatible code | Retained subject to schema compatibility |
| Owner/provider crashes | Adapter reports interrupted state | That execution stops | Resume supported conversation; do not replay input automatically |
| User logs out | Mac LaunchAgent stops; Linux lingering service can remain | Mac unsupported across logout; Linux depends on separate units and credentials | Retained |
| Host sleeps | Unreachable until awake | Suspended; provider/network operations can time out | Retained |
| Host reboots | Starts at next supported boot/login point | Old processes are gone | Reconcile and resume eligible conversations |

Current [transport lifetime policy](../rust/loopflow/src/transport/lifetime.rs) ends unviewed Sessions after 24 hours by default, when waiting for a person. Replaced/superseded executables also retire idle owners; active turns settle first. Headless Runs are excluded. Thus “continues after disconnect” is bounded by deliberate policy, not indefinite immortality. Mobile must expose this behavior without treating retained history as a live process.

Current [recovery](../rust/loopflow/src/transport/README.md) uses `lf resume --all` for owners missing without a recorded exit; Desktop invokes it at launch. A standalone host needs an explicit equivalent recovery trigger. Preserve the rule that interrupted input is flagged, never replayed. Do not convert service startup into unbounded automatic replay of Tasks or Graphs.

## Credentials, tools and upgrades

A login environment is part of execution. Current [GUI environment enrichment](../swift/LoopflowMac/Services/ProcessEnvironment.swift) supplies common executable paths and removes inherited execution attribution. The [scheduled-host installer](../scripts/bootstrap-cron-host.sh) deliberately uses the Mac GUI domain for login-Keychain access. A new Rust service must share the effective host environment contract instead of assuming Swift’s enrichment runs. Resolve provider binaries, Git helpers and runtime managers under the selected account; report executable paths and authentication readiness without exposing secrets.

Apple warns that a daemon outside a login context cannot use the data-protection keychain, and SSH-launched tools can encounter locked credentials. A reachable listener therefore does not prove an agent can authenticate. Test cold login, locked screen, SSH-only access and provider credential renewal separately. [Apple keychain guidance](https://developer.apple.com/forums/thread/724013)

[VS Code Agent Host](https://code.visualstudio.com/docs/agents/concepts/agent-host) makes an important qualification: desktop-managed work can survive closing a project window while the application remains running. Client-contributed tools still require their client; a stored conversation does not establish continuing execution. Loopflow should advertise tool availability by location: host filesystem/shell, provider runtime, Desktop browser/computer tools, or phone tools. A surviving PTY cannot keep an absent client tool alive.

Use Loopflow’s existing stable entry and [installation adoption](../rust/loopflow/src/lf/commands/install/published.rs), not a worktree executable or temporary app extraction path. **Proposed upgrade contract:** update the adapter independently, retain old owner executables while busy, negotiate owner/client compatibility, and respect store reader floors before migration. Existing owners can lose store access after schema advancement; “wait until idle” alone cannot guarantee recovery. Establish that policy before auto-update.

## First experiment and boundary for the future hub

Build a disposable lifecycle harness before the full mobile client: start one real provider, a long-running shell and a Graph; disconnect every viewer; restart the installed adapter; update its executable; then reconnect and verify the same process identities, terminal state and single recorded action. Repeat under macOS launchd and Linux systemd, including lost acknowledgements. Separately exercise logout, sleep and reboot as recovery cases, not survival demonstrations.

Keep a future hub outside this lifecycle. It may durably accept signed webhook events and deliver them when a Machine reconnects. The Machine owns execution and acknowledgement; the hub neither wakes a sleeping laptop by implication nor keeps its processes alive. Reserve that delivery seam now. Build none of its features for the initial mobile service.
