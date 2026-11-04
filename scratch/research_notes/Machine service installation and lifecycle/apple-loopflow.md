# Apple service installation and Loopflow's local backend

Research checked 2026-11-03 for LOO-718. These are recommendations and evidence, not an accepted implementation decision. Jack Heart is considering a socket-based Desktop backend as well as phone access.

## Does Desktop already install the Machine server?

### Takeaway

No. The proposed Machine service does not exist. Desktop starts an app-owned UI automation socket. Existing launchd installation code schedules installation refreshes, not a server.

### Cited Findings

- [DesktopServer.swift](../../../swift/LoopflowMac/DesktopServer.swift) explicitly says its socket lives and dies with the app. It serves inspection, UI actions, terminal text and change events; Sessions run in detached lf transports. It checks same-user peer credentials and sets private filesystem permissions.
- [LoopflowApp.swift](../../../swift/LoopflowMac/LoopflowApp.swift) calls `DesktopServer.start()` during application initialization. The inspected Swift source has no SMAppService registration.
- [published.rs](../../../rust/loopflow/src/lf/commands/install/published.rs) writes `com.loopflow.refresh.plist` to schedule refreshes. This is unrelated to retaining an interactive backend.

### Inferences

Keep the existing app automation endpoint with the app: its operations require windows and main-actor state. A new Machine service should provide Work and Session operations independently of those windows. Similar socket framing does not make these the same service.

### Gaps

No installed Loopflow Machine job exists to validate yet.

## How automatic can installation be?

### Takeaway

If Desktop becomes a socket client, register/bootstrap its local backend automatically when Desktop needs it. Enable network listening separately when phone access is enabled. Use a per-user LaunchAgent for the personal Mac default.

### Cited Findings

- macOS 13+ `SMAppService.agent(plistName:)` registers bundled LaunchAgents. Registration bootstraps the agent immediately and on subsequent logins; registration is per user. Elevated LaunchDaemons require administrator approval. [Apple register documentation](https://developer.apple.com/documentation/servicemanagement/smappservice/register())
- Apple describes default permission plus notification for ordinary background/login behavior, with control in System Settings. Therefore “automatic” need not mean a second Loopflow confirmation, but cannot mean overriding a disabled item. [WWDC22](https://developer.apple.com/videos/play/wwdc2022/10096/)
- SMAppService exposes authorization/registration status and a way to open Login Items settings. This is separate from application-level readiness. [SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice)

### Inferences

Recommended startup: resolve the selected installation and home → register if needed → connect to the private socket → verify protocol/service readiness. Display an actionable Background Items state if macOS has disabled it. Do not repeatedly re-register to defeat that choice. Tailscale detection and remote readiness are separate status checks; normal local Desktop use should not require Tailscale.

A permanent listener or validated launchd socket activation is necessary for remote-only access. A CLI's lazy start alone cannot accept a phone connection when no local command runs. Prefer the simplest supervised foreground service first; no self-daemonizing or double-forking listener.

### Gaps

Validate signed distribution and development builds, consent-disabled recovery and simultaneous app/CLI startup on supported macOS releases.

## What owns packaging, updates and standalone installation?

### Takeaway

Use one installation authority and service identity. Bundle-relative SMAppService registration fits the promoted app's helper; the public `lf` entry remains the control path, not a second independently installed daemon.

### Cited Findings

- The installed Apple SDK's [SMAppService.h](/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks/ServiceManagement.framework/Headers/SMAppService.h) requires the agent plist in the calling app's `Contents/Library/LaunchAgents`, supports `BundleProgram` with a bundle-relative executable path, and requires code signing. Bundle-relative paths accommodate moving the app.
- That header requires re-registration when the plist or executable changes and recommends unregistering first for executable changes. Unregister kills the running helper; the asynchronous completion indicates when it is safe to re-register. Do not confuse updating installation files with a nondisruptive service update.
- [installation.rs](../../../rust/loopflow/src/installation.rs) already resolves the authoritative installed app from the public `~/.local/bin/lf`, which promotion links to `Contents/MacOS/lf`. Development apps deliberately pair with their own lf.
- Apple's sample supports a signed GUI-less app bundle containing a LaunchAgent and a registration executable. It demonstrates standalone installation without running a graphical application. [Apple package sample](https://developer.apple.com/documentation/servicemanagement/updating-your-app-package-installer-to-use-the-new-service-management-api)

### Inferences

Have existing installation promotion reconcile the service, including unregister/re-register and reconnection. Desktop and CLI invoke that same operation. A registration helper inside the selected signed bundle is more natural than expecting an arbitrary external Rust CLI to act as that bundle. Standalone distribution can use a small signed container with the same server/protocol; it need not ship the Desktop UI. Do not create a second store or semantic server implementation.

Keep development service identities/homes separate from the promoted installation, using existing resolution. Installation migration and schema ownership must be deliberate: today's FFI only opens the store prepared by lf and never migrates it. Deleting Desktop's bundle also deletes any server executable packaged inside it; runtime independence does not imply uninstall independence. Uninstall should unregister only Loopflow's item and remove its owned artifacts, not reset all background-item registrations.

### Gaps

Select app-owned bundle versus separately installed GUI-less container after testing upgrade/uninstall requirements. A per-user helper does not establish unattended before-login support.

## What lifetime does launchd provide?

### Takeaway

The backend can survive Desktop quitting. A personal LaunchAgent runs while its user is logged in; it does not promise survival across logout, reboot or machine sleep.

### Cited Findings

- launchd creates per-user agents at login and sends SIGTERM at logout; it supports demand/socket activation and continuous services. [Apple launchd guide](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)
- [transport.rs](../../../rust/loopflow/src/transport.rs) already detaches Session owners using `setsid`. That is source evidence of process separation, not an installed-job proof across service unregister, logout or system restart.

### Inferences

Keep Session owners independent of the listener and prove that independence under launchd's real job cleanup. Service update can reconnect clients without terminating active agents only if this separation works. An asleep host is unavailable; a reboot loses live processes even if durable Sessions can later resume. Before-login service operation is a different daemon deployment with administrator approval, user identity, credential and tool-path questions.

Acceptance should cover app quit, backend crash/restart, active Session during upgrade/unregister, login/logout, reboot, sleep/wake, missing Tailscale, disabled Background Items, moved/deleted app and developer/release coexistence. Registration, authorization, running, locally ready and remotely reachable must remain distinguishable.

### Gaps

No claim that detached owners survive job unregister is justified until tested. Do not promise process persistence merely from durable Session identity.

## Can Desktop and mobile share a socket backend without duplicate domain logic?

### Takeaway

Yes. The existing C boundary already carries JSON typed reads, writes and watch frames. Replace its transport adapter with a service client and reuse the Rust implementation. The existing DesktopServer is not this boundary.

### Cited Findings

- [RegistryQueryLocal.swift](../../../swift/LoopflowMac/Services/RegistryQueryLocal.swift) wires `RegistryQuery` to FFI reads/writes and Work watches, while other commands and remotely routed writes use bundled lf subprocesses. Remote Task file watches use lf over SSH.
- [LoopflowLibrary.swift](../../../swift/LoopflowMac/Services/LoopflowLibrary.swift) encodes `ServedRead`/`ServedWrite` as JSON for `lf_read`, `lf_write`, and `lf_subscribe`; `ServedWatch` translates wakeup/pull/request/cancel into async frames.
- [RegistryQuery.swift](../../../swift/Loopflow/Services/RegistryQuery.swift) provides the typed query facade and JSON shapes already consumed by views.
- [FFI implementation](../../../rust/loopflow-ffi/src/lib.rs) calls `Read::read`, `Read::subscribe` and `Write::write`, defined in [read.rs](../../../rust/loopflow/src/lf/commands/read.rs), [watch.rs](../../../rust/loopflow/src/lf/commands/watch.rs) and [write.rs](../../../rust/loopflow/src/lf/commands/write.rs). Watches already coalesce changes, pin read snapshots and accept argument-update/confirmation requests.

### Inferences

The service should call the same native `Read` and `Subscription` APIs. Typed mutations may delegate to lf as Jack requested; reuse command mapping and receipts rather than duplicate mutation logic. Local Unix and authenticated Tailscale transports can carry one application protocol and DTO set, with transport-specific peer authentication. A remote client cannot inherit Unix same-user trust.

Migration targets: replace `RegistryQueryLocal`'s FFI closures and `LoopflowLibrary`/`ServedWatch` glue with socket implementations; preserve the higher-level `RegistryQuery` and Work watch consumers where their semantics fit. Add connection/subscription IDs, explicit cancellation, version negotiation, bounded buffering, reconnect snapshots and command receipts. Inventory the remaining direct subprocess operations so routing ownership is explicit.

If this cutover is selected, remove the migrated FFI route and its build/test wiring; do not retain a permanent local-direct fallback that can quietly diverge from mobile. Preserve unrelated FFI uses only if separately justified. Keep app-window automation local to Desktop. This moves ownership while retaining one implementation of Work.

### Gaps

Existing watch replies are snapshots and request acknowledgements, not a general event-replay or offline mutation protocol. Network reconnect/retry semantics still require a design. This research made no source or installation changes.
