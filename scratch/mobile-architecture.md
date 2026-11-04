> Exploratory notes retained for context. The current synthesis is [mobile-system.md](mobile-system.md), presented in [the interactive architecture map](mobile-architecture.html). Jack Heart subsequently selected a terminal as the MVP core; the current prototypes are [Sessions, Tasks and Focus](mobile-prototypes.html). Recommendations below that differ from the synthesis are earlier hypotheses.

# Loopflow mobile architecture

2026-11-03 · Research · No Task selected · Checkout `2bc29a72e0`

Jack Heart requested a native Swift mobile experience, borrowing heavily from Rex, with Tailscale as the only remote connection path and no custom session relay. Jack selected host-installed Tailscale for the first version and requested automatic detection and setup once signed in. The proposed boundary is **one private Machine service, available independently of Desktop, plus an optional public webhook receiver**. These are different responsibilities, not two editions of the execution server.

## What exists today

There is no general network-facing Loopflow server in this checkout. There are several useful pieces to assemble:

| Existing piece | What it does | Mobile implication |
| --- | --- | --- |
| Rust library and CLI | Read and mutate Machine-owned Work | Reuse these operations behind the service |
| Desktop library adapter | Runs reads, writes and watches in the app; other operations use `lf` | Desktop does not presently connect to a standalone backend |
| Desktop Unix socket | Inspects and controls the running GUI | Lives with Desktop; unsuitable as the mobile backend |
| Detached Session owners | Hold processes, PTYs and terminal state across viewer disconnection | Preserve this lifetime boundary when adding a network service |
| Machine SSH transport | Runs the destination's `lf`, independently of Desktop | Headless remote execution already exists |
| Shared Swift target | Models, queries and reusable views; declares iOS support | No iOS application target or mobile terminal integration yet |

Evidence: [local query adapter](../swift/LoopflowMac/Services/RegistryQueryLocal.swift), [Rust library adapter](../swift/LoopflowMac/Services/LoopflowLibrary.swift), [Desktop socket](../swift/LoopflowMac/DesktopServer.swift), [Machine routing](../docs/architecture/machines.md), [Swift manifest](../swift/Package.swift).

The [viewer transport](../rust/loopflow/src/transport/README.md) already specifies terminal snapshots, ordered output, input control, independent size/color ownership, and reconnection from the last received byte. Its current resume window is 60 seconds with a 1 MiB spool per viewer; beyond that it needs a fresh attachment. Images are not repainted. Network reconnection must preserve those distinctions rather than imply unlimited replay.

The [planning contract](../docs/architecture/planning.md) explicitly says the former daemon webhook ingress was removed. [Old daemon environment settings](../docs/architecture/environment.md) are inert. Current launchd scheduling code is not a general Machine daemon.

Follow-up [historical research](lfd-history.md) found two lfd generations: a broad execution/mobile API deleted in July, then a webhook-only daemon that grew into a supervisor and was deleted in September. Standalone launchd hosting, Tailscale mobile pairing and a thin command adapter were all attempted. A new Machine API would restore part of lfd's responsibility. SSH over Tailscale is an existing alternative to evaluate before selecting a persistent network API; the service below remains a proposal, not a requirement imposed by mobile or Tailscale.

Jack clarified that repeated attempts express both durable product intent and real implementation difficulty. Their removal does not revoke the desired experience or establish a preference against a server. Jack also requested that the future hub participate in the overall systems design now, while its features stay outside this implementation.

## Proposed service boundary

```text
iPhone / iPad                       Desktop / CLI
      │ Tailscale + HTTPS/WSS             │ local API
      └──────────────────┬───────────────┘
                         ▼
                Private Machine service
                 on a Mac or Linux host
                    │             │
           Work reads/writes   Session attachment
                    │             │
          Machine-owned state  Detached owners → agents / PTYs

GitHub / Linear → public webhook receiver (optional)
                         ▲
             Machine pulls queued events
```

The service runs under the OS account that owns the checkout, credentials and Machine store. Desktop can install and manage it; quitting Desktop must leave it available. A remote Linux installation needs no GUI. Use one implementation and protocol in both deployments, with a user LaunchAgent on macOS and a corresponding user service on Linux. Login, logout, credential access and machine sleep need explicit lifecycle behavior; a background process cannot make a sleeping laptop available.

Keep the service as a frontend to existing Rust operations and detached Session owners. Restarting that frontend should leave agent execution running. Do not move provider ownership into it merely because the phone needs a listener. Desktop can adopt the local service progressively; the mobile prototype does not require immediately replacing every local FFI call.

The optional hub accepts authenticated provider webhooks, retains delivery IDs and queued events, and lets Machines catch up. Events invalidate or refresh provider observations through the existing writers. The hub does not need source checkouts, provider credentials for agent execution, terminal streams, or a second copy of authoritative Task state. Without it, foreground refresh and scheduled polling still work. Specify retention and catch-up separately from provider retry guarantees.

GitHub and Linear need a publicly reachable webhook endpoint; the private mobile endpoint remains on the tailnet. [Tailscale Funnel](https://tailscale.com/docs/features/tailscale-funnel) can expose a separate webhook-only service as an alternative deployment, but it makes that endpoint public and leaves its availability tied to the host.

## What to borrow from Rex

Rex documents standalone personal, shared-account and login servers. The login server routes authenticated people to separate OS accounts; it is not a public webhook hub. Start Loopflow with the personal-account model. [Server modes](https://www.superlogical.com/rex/docs/linux/server-modes)

Its published contracts give us concrete patterns: a bootstrap snapshot followed by sequenced events; explicit attachment and ownership; version reporting; and short-lived, one-use tickets for terminal data WebSockets. The API documents `/api/data` and a terminal subprotocol. It does not establish the complete terminal wire format in the pages inspected. Copy the separation and reconnection behavior, while retaining Loopflow's Work and Session identities. [API methods](https://www.superlogical.com/rex/docs/reference/api)

Rex also exposes method descriptions and JSON schemas from the running server. This is a useful model for keeping Swift bindings and server capabilities aligned. [API discovery](https://www.superlogical.com/rex/docs/automate/server-api)

For Loopflow, propose two logical channels: typed Work operations/subscriptions, and Session terminal streams bridged to the existing owners. Negotiate API compatibility before attaching. Reconnect output from a cursor or a fresh snapshot; do not automatically replay uncertain keystrokes or launch requests. Give retryable mutations durable request identities where duplicate execution matters.

Rex's Tailscale choices include an embedded node, Tailscale Serve and direct tailnet listeners, with caller identity and access policy. We can follow its Serve approach without adopting its embedded networking immediately. [Remote access](https://www.superlogical.com/rex/docs/linux/remote-access)

Host-installed Tailscale is selected. If the service API is selected over SSH, use HTTPS/WSS through Serve, and a backend reachable only through the trusted local proxy path. Serve provides identity headers; they must not be trusted on a separately exposed listener. [Serve](https://tailscale.com/docs/features/tailscale-serve)

Embedding Tailscale is a different packaging decision: its supported `tsnet` integration is Go, so Rust/Swift needs an embedding bridge or helper. [tsnet](https://tailscale.com/docs/features/tsnet) Tailscale may use encrypted DERP relays when a direct path fails; that still satisfies “no custom Loopflow relay.” [Connection types](https://tailscale.com/docs/reference/connection-types)

### Automatic setup with host Tailscale

Proposed experience: **Enable phone access → scan a QR code → return to the same Machine automatically.** Tailscale sign-in and any required tailnet administration remain with Tailscale; Loopflow handles the application setup around them.

On the host, discover the installed client, including macOS app variants whose CLI is not on PATH. Read structured status for backend state, host addresses and DNS identity. Distinguish an installed client from a running, signed-in node. Use the client's machine-readable interfaces, accounting for their versioned behavior rather than parsing display text. [CLI status](https://tailscale.com/docs/reference/tailscale-cli?tab=macos)

For the proposed API path, inspect existing Serve configuration, allocate a nonconflicting private route, start the Loopflow endpoint and verify its identity/version. Configure only Loopflow's route and remove only that route when disabled. Serve supports port forwarding on the macOS app variants, machine-readable configuration inspection and background persistence; HTTPS certificates are provisioned automatically. [Serve commands](https://tailscale.com/docs/reference/tailscale-cli/serve)

If HTTPS prerequisites are missing, follow the consent URL supplied by Tailscale and resume setup afterward. A restricted account may need its tailnet administrator. Do not turn a missing permission into an endless retry or silently change unrelated tailnet access rules. [Serve prerequisites](https://tailscale.com/docs/features/tailscale-serve)

The QR/link supplies the endpoint and expected Machine identity. The phone saves it and tests the actual authenticated Loopflow connection, then reconnects when foregrounded or after a network change. Host status alone cannot prove phone-to-host reachability. Diagnose observed failures; a timeout alone cannot distinguish a sleeping host, a disconnected VPN or a blocked network policy.

Do not promise automatic enumeration of every Loopflow host from the iOS Tailscale app: an accessible peer-discovery integration has not been established. One initial QR scan avoids needing that integration or a Tailscale administration API key on the phone. Tailscale's iOS installation still requires its own VPN consent and sign-in. [iOS setup](https://tailscale.com/docs/install/ios)

The SSH alternative should offer the same endpoint handoff and readiness experience, but needs its own account/authentication setup; Tailscale sign-in does not by itself configure ordinary SSH. HTTPS/Serve setup applies only if the API transport is chosen.

## What to borrow from Paseo

Paseo separates a daemon, protocol schemas, client SDK and app. It supports direct connections as well as an optional relay, and its terminal worker survives daemon restarts. Its mobile client restores cached state, then reconciles with server data. These are useful references for reconnect and app lifecycle; adopting its Expo UI, agent runtime or relay would duplicate choices Loopflow already owns. [Architecture](https://github.com/getpaseo/paseo/blob/main/docs/architecture.md)

## Native terminal shortlist

“libghostty” names more than one integration surface. Loopflow already uses libghostty-vt for retained terminal state and a patched GhosttyKit for macOS rendering. Its [current artifact](../swift/GhosttyKitPatches/README.md) is not an iOS UIKit integration. Upstream now builds libghostty-vt for iOS, but its API remains in development. [Ghostty 1.4 notes](https://ghostty.org/docs/install/release-notes/1-4-0)

| Candidate | Best use for this project |
| --- | --- |
| [Lakr233/libghostty-spm](https://github.com/Lakr233/libghostty-spm) | First integration candidate: Apple-platform XCFramework, native wrappers, Metal rendering and host-managed I/O. Feed the Loopflow stream without requiring SSH. |
| [Termini](https://github.com/hudsonkit/Termini) | Alternative SwiftUI library with a controller for custom byte transports. Evaluate its view/input API alongside the first candidate. |
| [Rootshell](https://github.com/kitknox/rootshell) | Broad app reference: open-source iOS/iPadOS Ghostty terminal, keyboard, session and mobile networking experience. Study selected components rather than importing the whole app. |
| [gterm](https://github.com/madeye/gterm) | Smaller reference: passthrough Ghostty I/O, UIKit surface, terminal key bar and SSH integration. |
| [SwiftTerm](https://github.com/migueldeicaza/SwiftTerm) | Native Swift/UIKit fallback and input/accessibility comparison. Its terminal engine is separate from Ghostty. |

This is a fit-based shortlist, not a measured performance ranking. Before selecting a dependency, prove simulator/device builds, IME and dictation, selection, hardware keys, scrollback, suspend/resume and power behavior. Check whether Loopflow's existing Ghostty patches are needed on iOS and whether two differently patched libraries would enter the same target.

## Proposed phone experience

Rex's iOS references expose host/session switching, tabs, panes, keyboard controls and a configurable accessory key bar. Its installation page still limits public testing to Mac and server builds, so these are documented interactions, not a hands-on iOS assessment. [Actions](https://www.superlogical.com/rex/docs/reference/ios-actions), [settings](https://www.superlogical.com/rex/docs/reference/ios-settings), [availability](https://www.superlogical.com/rex/docs/installation)

Use native SwiftUI navigation around a UIKit/Metal terminal. Keep Loopflow's Wave → Task navigation, with direct access to recent Sessions and a visible Machine selector. On iPhone, open one Session at a time; Task details and changes are adjacent native views. On iPad, add a sidebar and optional split views.

Opening a Session initially observes it. An explicit “Use here” action acquires input and, when intended, terminal size ownership. A phone glance should not reflow the desktop's running TUI. Keep a last-seen screen through reconnect, visibly disconnected, and disable input until attachment is acknowledged. Preserve drafts locally; never submit them merely because the connection returned. Use native text composition for dictation/IME alongside direct terminal keys.

Foreground control needs no public hub. Background completion alerts are a separate decision: iOS normally suspends apps, so an open WebSocket cannot be the alert delivery mechanism. APNs requires a provider path; a future notification service could share deployment with the optional hub without carrying terminal traffic. [Apple background lifecycle](https://developer.apple.com/documentation/uikit/preparing-your-ui-to-run-in-the-background), [push delivery](https://developer.apple.com/documentation/usernotifications/pushing-background-updates-to-your-app)

## Architecture decisions and comparative study

The choices to establish early are authority, lifetime, session behavior and delivery semantics. A daemon's name and the specific wire transport follow those choices. The following are research questions and proposed experiments, not accepted architecture decisions.

### Authority across Machines and the future hub

Distinguish a Machine's stable identity from its current address, an OS account from a Tailscale principal, and a Task's identity from the Machine executing its work. Keep the current planning provider's ownership intact. Decide which facts mobile caches and which commands must reach a specific Machine. A unified list of Sessions should not silently imply that any Machine may execute the same Task.

For the hub, compare an event inbox with an authorized automation client. Does it only deliver observations, or may it request execution and record workflow progress? How does it select a Machine? What happens to pending work when ownership changes or two Machines receive an event? Specify these boundaries and disconnection behavior now; defer the actual webhook integrations and automation UI.

[Paseo Hub](https://paseo.sh/hub) is a direct comparison: an optional service above existing daemons using the same RPCs as its clients. Its [security contract](https://paseo.sh/docs/hub/security) separates connecting a daemon from authorizing execution. Our hub need not adopt its workflow ownership or broad capabilities. [GitHub delivery guidance](https://docs.github.com/en/webhooks/using-webhooks/best-practices-for-using-webhooks) and [Linear webhooks](https://linear.app/developers/webhooks) supply the provider-specific evidence for acknowledgements, redelivery and duplicate handling.

### Runtime lifetime and installation

Write down which processes survive closing a window, quitting Desktop, disconnecting the network, restarting the API, replacing its executable and rebooting the host. A persistent conversation record does not mean an in-flight process survives reboot. Explicitly place the owner of PTYs, provider connections, history and restoration. Include tools supplied by a client: browser and desktop capabilities may disappear even when the agent keeps running.

Study [Paseo's detached terminal worker](https://github.com/getpaseo/paseo/blob/main/docs/terminal-worker.md), including why ordinary detached children can still die during process-tree cleanup. [VS Code's Agent Host](https://code.visualstudio.com/docs/agents/concepts/agent-host) separates session execution from clients and explicitly identifies client-contributed tools that require an attached client. These are comparison models, not reasons to replace Loopflow's owners.

For macOS, study [SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice) before choosing bundle layout and helper installation. Validate a real service's credential access, working environment, update and uninstall behavior; a foreground terminal launch proves little about those. Establish Linux service behavior separately.

### What is shared between clients

A Session, its input control, terminal dimensions, scroll position and pane layout are separate concepts. Decide which are shared and which belong to a particular device. The phone may observe a desktop-sized terminal, take over its geometry, or show a structured conversation without changing the PTY. None of these should create a second provider conversation accidentally.

Study [iTerm2's tmux integration](https://iterm2.com/documentation-tmux-integration.html) for native UI over persistent remote state. [tmux control mode](https://github.com/tmux/tmux/wiki/Control-Mode) supplies commands, output notifications and flow control over a channel usable through SSH. [Paseo's terminal performance contract](https://github.com/getpaseo/paseo/blob/main/docs/terminal-performance.md) makes size ownership explicit and explains how batching can harm keystroke responsiveness.

Prototype two clients attached at once, one with the software keyboard opening and closing. Include takeover, observation, terminal response handling, paste and independent history inspection. Decide whether the product needs simultaneous typing or deliberate handoff.

### Reconnection and uncertain delivery

There are three different recovery problems: current Work state, terminal output/history, and commands sent just before the network disappeared. Give each a contract. Specify cursors, snapshot fallback and retention bounds for reads; represent a submitted command as accepted, completed or outcome-unknown until reconciled. Do not equate socket disconnection with a failed action.

[Mosh](https://mosh.org/#techinfo) synchronizes screen state and can skip intermediate display updates; it does not preserve complete terminal scrollback by itself. [Eternal Terminal](https://eternalterminal.dev/) emphasizes reconnecting a persistent stream and supporting native scrolling/tmux control mode. Compare those tradeoffs with Loopflow's current snapshot-plus-byte-resume design. [Paseo timeline sync](https://github.com/getpaseo/paseo/blob/main/docs/timeline-sync.md) distinguishes live updates from authoritative history. [Stripe's idempotency contract](https://docs.stripe.com/api/idempotent_requests) is a useful model for retryable semantic actions, not a guarantee we can automatically apply to raw keystrokes or arbitrary external side effects.

### Application protocol and transport

Define the client operations and their semantics before deciding SSH versus HTTPS/WebSocket. A persistent application server may be reached through SSH: [WezTerm multiplexing](https://wezterm.org/multiplexing.html) already uses this combination. Avoid making transport choice secretly choose a second Work model.

The phone, Desktop, host service and retained Session owners will not necessarily update together. Establish a version/capability handshake, a bounded compatibility policy and clear unsupported-operation behavior before the first distributed release. Keeping an old Session alive while upgrading the network service is a concrete compatibility case. [VS Code's Agent Host protocol model](https://code.visualstudio.com/docs/agents/concepts/agent-host#agent-host-protocol) is worth examining for resource subscriptions, initial snapshots and reconnection.

### Trust and automatic setup

Host Tailscale and one-time QR pairing are selected. Decide what the QR establishes, how the server maps a connected principal to the Machine's OS account, and how phone access is revoked. Tailnet reachability alone should not implicitly grant every member control. A future hub connection needs its own explicit authority; receiving signed provider events and authorizing execution are different steps.

Evaluate the full setup on a clean Machine and a headless host: sign-in, service discovery, endpoint creation, phone validation and repair after a host rename. The target is the accepted automatic flow, including honest diagnostics when the host cannot establish why the phone is unreachable. [Tailscale Serve](https://tailscale.com/docs/features/tailscale-serve) is the integration reference if the HTTP API is selected.

### Native mobile experience

Decide how far native Task/change/conversation views go alongside the terminal. Use structured Loopflow/provider data where available; rendering terminal text as a chat transcript would lose its semantics. Typed drafts, IME/dictation, hardware keys, selection, links, accessibility, background suspension and low idle power belong in the first terminal experiment.

[Blink](https://docs.blink.sh/) is a useful product reference for touch selection, gestures and terminal keys; [Rootshell](https://github.com/kitknox/rootshell) and the library shortlist above supply Ghostty implementation references. [Apple's background lifecycle](https://developer.apple.com/documentation/uikit/preparing-your-ui-to-run-in-the-background) determines how reconnection and future notifications must behave. Public docs establish mechanisms; interaction quality still needs device use.

### Evidence to gather before committing to the design

| Exercise | Decision it should resolve |
| --- | --- |
| Two clients, one Session; rotate the phone, open its keyboard, observe and take control | Shared terminal geometry, input ownership and device-local presentation |
| Drop connectivity immediately after submit, then reconnect after the output retention window | Command receipts, uncertain outcomes and snapshot recovery |
| Update/restart the network frontend during an agent turn | Process ownership and the necessary compatibility boundary |
| Set up from a clean Mac and Linux account with host Tailscale | Real installation, authentication and credential requirements |
| Compare the same phone operations over SSH and an API transport | Setup friction, control responsiveness and implementation cost |
| Draw an event arriving twice while its owning Machine is offline, then ownership changes | Future hub routing, deduplication and retained authority without implementing the hub |

Study order: Paseo's terminal worker, timeline and Hub boundaries; tmux/iTerm2 and WezTerm; Mosh/ET; then the relevant Apple and Tailscale integration contracts. Return to the lfd history for specific failure modes and unrealized product goals rather than treating prior deletions as a veto.

## Smallest useful proof

Build only after choosing the first interaction and comparing SSH transport with the proposed service: connect a Swift iPhone client to a headless Machine through Tailscale, list Sessions and attach a Ghostty surface. Keep one live agent running while closing Desktop, backgrounding the phone, changing networks, restarting any transport service and reconnecting beyond the current grace period. Verify input is not duplicated and observation does not steal desktop size. Repeat on a Linux host. This tests the architecture before adding webhook delivery or broad mobile planning UI.

## Decisions to make

1. Transport over host Tailscale: **a.** existing SSH commands and attachment; **b.** private API service. Host-installed Tailscale and automatic setup are selected; this transport choice remains open.
2. First phone surface: **a.** recent Sessions with native Task context; **b.** Wave → Task navigation first, then its Session. Both retain the same backend and terminal.
3. Notifications: **a.** foreground interaction first; **b.** background completion alerts in the first release, with an explicit APNs provider design.
