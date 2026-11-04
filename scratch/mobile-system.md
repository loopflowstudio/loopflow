# A terminal in your pocket, execution on your Machine

Architecture research for Jack Heart · 2026-11-03 · LOO-718 · Checkout `2bc29a72e0`

**Recommended architecture:** a native Swift terminal connects over host-installed Tailscale to a private Loopflow Machine service. The service runs independently of Desktop and exposes existing Rust operations and Session attachments. Execution remains in separate owners. A future public hub receives webhooks and can become an explicitly authorized client of that same Machine boundary.

Jack selected native Swift, a terminal as the core experience, host Tailscale and no custom relay. Jack requested automatic setup and architectural room for an optional hub, without building hub features now. HTTPS/WSS, the pairing protocol, service packaging and the precise screen design below remain recommendations to validate.

## Accepted minimum experience

Jack Heart selected existing open Tasks as the app’s entry point, like Desktop’s working-task sidebar. Tap a Task and go directly to its existing Session terminal. The terminal supports typing, editing input, submitting and responding to prompts. Wave navigation, shells, other pane types, changes and check views remain outside the selected surface. Jack Heart subsequently selected new-Task flow 1: a compact describe-first sheet with a visible editable repository/Machine destination. [Selected flow](mobile-new-task-flows.md); [current backend draft](mobile-backend.md). See the [updated Task prototype](prototype-tasks.html). Input ownership and geometry still need a protocol decision; the first screen no longer does.

The Machine service delegates commands to `lf`; sharing native in-process subscriptions with `lf watch` remains a design question. Earlier references below to calling existing operations describe shared authority, not an accepted requirement to execute commands inside the service.

## The server question, answered

The proposed server belongs to **a Machine: one OS account and Loopflow data directory**. It can run on the Mac used for Desktop or on a remote host without Desktop. Two accounts on one computer can be two Machines. A network address is a route to that authority, not its identity. [Current Machine contract](../docs/architecture/machines.md)

Today, Desktop calls Rust through a local library and launches CLI operations. Its own Unix socket automates the GUI and dies with the app. Detached Session owners already retain execution; remote CLI commands already run through SSH. There is no general network Machine API in this checkout. [Local query adapter](../swift/LoopflowMac/Services/RegistryQueryLocal.swift), [Desktop socket](../swift/LoopflowMac/DesktopServer.swift), [Session transport](../rust/loopflow/src/transport/README.md)

This restores a responsibility of **lfd**. Standalone hosting, launchd, mobile pairing and a thin command adapter have all existed before. The earlier deletions show maintenance and ownership difficulties alongside sustained product intent. The difference to prove this time is that the network frontend can disappear without changing the identity or lifetime of running work. Naming it “thin” proves nothing. [Historical evidence](lfd-history.md)

## 1. Give each component a clear owner

| Component | Owns | Boundary to preserve |
|---|---|---|
| iPhone / iPad | Terminal presentation, navigation, local selection and scroll position, paired credential | An attachment does not create a new provider conversation. |
| Desktop / CLI | Local interaction and device-specific layout | Desktop need not run for phone access; local clients can keep current Rust paths initially. |
| Machine service — proposed | Authentication, protocol connections and command admission | Calls existing operations; no parallel Task or Graph engine. Durable receipts belong in Machine-owned storage. |
| Existing Rust operations and store | Local records and execution facts; provider synchronization | Planning intent keeps its configured provider/journal authority. |
| Session owners and Graph runners | Their actual execution, provider identity, PTYs and retained output | Separate supervision from the network frontend must be proved. |
| Optional hub — future | Verified event inbox, trigger configuration, dispatch receipts | May request authorized work; does not become the Machine database or terminal relay. |

The hub can begin as an inbox, then gain an explicit execution grant. It dispatches an existing Loopflow operation to **one named Machine**; that Machine's Graph runner chooses the steps. Event refresh may fan out. Execution must not. Current Task claims deliberately tolerate disagreement and preserve local work; they are not a distributed lock. Offline status does not authorize another Machine to take over. [Authority research](research-authority-hub.md), [ownership implementation](../rust/loopflow/src/store/sqlite/task_ownership.rs)

For a future deployment, the Machine initiates an outbound authenticated connection to the public hub. Webhook events and scoped requests travel back over that connection. Direct phone traffic still uses Tailscale. This avoids requiring the hub to join each tailnet. Paseo provides a useful precedent for separating enrollment from execution permission; Loopflow should retain its own Graph model. [Paseo security](https://paseo.sh/docs/hub/security)

## 2. Define survival before installation

| What happens | Proposed behavior | Qualification |
|---|---|---|
| Phone disconnects or backgrounds | Work continues; phone later reattaches | iOS background networking is not an always-on connection. |
| Desktop quits | Installed service and execution continue | Desktop-provided tools may become unavailable. |
| Machine service restarts | Connections drop; execution continues | Not yet proved under either installed supervisor. |
| Service or app updates | Negotiate compatible protocols; retain busy owners | Executables, schema reader floors and owner versions all matter. |
| Mac user logs out | Personal LaunchAgent stops | Unattended hosting is a separate install mode. |
| Host sleeps | Execution suspends; network requests can time out | Tailscale does not make a sleeping host available. |
| Host reboots | Old processes are gone; reconcile saved history | Resuming a conversation must not replay an interrupted submission. |

**The important discovery:** today's owners call `setsid()`, but that does not escape a systemd service's cgroup. A naïve Linux service restart would kill its supposedly detached children. Independent execution units are one candidate solution. macOS requires a separate launchd/logout/update experiment. [Lifecycle research](research-runtime-lifecycle.md), [systemd process termination](https://www.freedesktop.org/software/systemd/man/latest/systemd.kill.html)

Personal Mac hosting can use a user LaunchAgent managed through SMAppService. Standalone installation can use the same implementation without a Desktop UI. Linux user services with lingering are a practical always-on option. A Mac that must recover before login needs both a compatible Loopflow installation and an appropriate Tailscale variant; ordinary Mac Tailscale GUI variants do not run before login. [Apple registration](https://developer.apple.com/documentation/servicemanagement/smappservice/register()), [Tailscale variants](https://tailscale.com/docs/concepts/macos-variants)

Credential access, PATH and browser/computer tools must be tested under the installed service account. A healthy listener does not prove an agent can authenticate or use a tool supplied by a closed application. Existing detached-session expiry and owner rollover policies remain relevant; this design does not promise immortal processes.

## 3. Share the Session, keep presentation local

**Open to observe. Choose “Use here” to take control.** Opening a phone, rotating it or showing its keyboard must not resize Desktop's TUI. Loopflow's existing wire already distinguishes input control from geometry/color ownership and supports passive observation. One PTY still has one canonical geometry. A passive phone pans or scales that screen; taking geometry causes the program to reflow for everyone. [Shared-session research](research-shared-sessions.md)

Keep tabs, split ratios, font size, zoom, search and scroll position on each device. Reconnection must not steal control from a device that took over more recently. Input accepted before takeover may execute; takeover cannot retract bytes already delivered.

There must also be **one terminal-query responder**. Current code uses the headless owner when no input controller exists and the controlling renderer otherwise, even though geometry can belong elsewhere. Prefer a stable host responder if its emulation coverage is sufficient; otherwise make responder election explicit. Two renderers must never both answer a device query or replay old responses. [Output processing](../rust/loopflow/src/transport/output.rs)

Native Swift provides the open-Task list and navigation back from its Session terminal. The Session accepts editable terminal input. Native dictation composition is now under discussion, with editable text before explicit submission; its integration with live terminal input remains to prove. Direct terminal keys have program semantics; the HTML field only simulates them.

## 4. Three recovery contracts, not one

| Stream | Current foundation | Remote contract to establish |
|---|---|---|
| Work state | Watches emit complete changed readings | Fresh authoritative snapshot plus ordered updates; label cached data. Current store revisions are not an event replay log. |
| Terminal output | Per-viewer byte resume: 60 seconds, 1 MiB spool | Resume only within the same stream generation and bounds; otherwise install a fresh screen snapshot. |
| Semantic commands | Existing operation-specific behavior | Durable request identity, acceptance/result lookup and explicit outcome-unknown state. |

Terminal snapshots currently omit images and bound retained history. A fresh screen is not a complete activity transcript. An interrupted socket says nothing definitive about whether a command ran. [Transport contract](../rust/loopflow/src/transport/README.md)

For semantic actions such as starting work, persist request identity and reconcile it to the accepted Process/Session. Reusing a request ID with different content must fail. Acceptance deduplication still does not guarantee exactly-once arbitrary external side effects; crashes before or after dispatch need recoverable admission and outcome reconciliation.

**Never automatically resend raw keys, paste or Enter after uncertainty.** Terminal input has no general operation result to query. Preserve the last screen visibly as stale, reacquire attachment and control, then let the person decide what to type. Backpressure must disconnect a slow viewer without blocking execution or Desktop. [Recovery research](research-recovery.md)

## 5. Choose the application contract before the carrier

Use two logical surfaces: typed Work operations/subscriptions and Session attachment/control/output. Reuse existing Rust operations; do not publish arbitrary CLI argv as the permanent application contract. The typed write surface still needs an inventory of missing mobile operations. A generic argv adapter already existed in lfd's history.

**Recommended first transport experiment: HTTPS/WSS through host Tailscale Serve.** It fits native app networking and ordinary Mac Tailscale installations. Compare the same operations over SSH before making the transport commitment. SSH can carry an application protocol and connect to a persistent server; it does not imply a serverless design. [WezTerm multiplexing](https://wezterm.org/multiplexing.html)

Handshake on Machine identity, API major/minor or equivalent compatibility range, and capabilities before attachment. Negotiate supported operations rather than relying on matching app build numbers. The phone, gateway and still-running owners can be three versions. Define that policy before the first independent mobile release. Exact compatibility duration remains open. [Protocol research](research-protocol.md)

Rex's control/data separation, bootstrap and sequenced events, explicit ownership and scoped terminal connection tickets are the closest API reference. Borrow the contract patterns; retain Loopflow identities and behavior. Public documentation does not prove a complete reusable terminal wire implementation or hands-on iOS experience. [Rex API](https://www.superlogical.com/rex/docs/reference/api)

## 6. Automatic setup with explicit trust

**Enable phone access → scan once → reconnect automatically.** The host detects its installed Tailscale variant, signed-in state, available private Serve endpoint and running Machine. The phone validates the actual connection; host readiness alone cannot prove phone reachability.

Proposed QR: endpoint, expected Machine ID, protocol version and a short-lived single-use pairing ticket. Exchange it for a revocable per-phone credential stored in Keychain. No tailnet admin key, provider credential or permanent bearer credential in the QR. Revoking access closes active streams as well as rejecting future requests.

Tailscale controls network reachability; application pairing controls access to a particular Machine. Serve's identity headers are useful context but do not isolate a loopback backend from other local OS users. Require application authentication there too. Prefer a dedicated private Serve port, preserve unrelated routes, and never enable Funnel for phone access. Serve/Funnel exposure applies to a whole port. [Trust/setup research](research-trust-setup.md), [Serve](https://tailscale.com/docs/features/tailscale-serve), [Funnel](https://tailscale.com/docs/features/tailscale-funnel)

Automatic setup cannot silently complete SSO, grant VPN consent, change organizational policy or wake a host. Tailscale offers iOS Shortcuts device discovery, but that does not establish a direct in-process enumeration API for Loopflow. QR handoff keeps setup independent of that question. Ordinary SSH still needs server/key setup; Tailscale SSH server support excludes the usual Mac GUI variants. [Shortcuts](https://tailscale.com/docs/features/mac-ios-shortcuts), [Tailscale SSH](https://tailscale.com/docs/features/tailscale-ssh)

## 7. Make the terminal excellent on an actual phone

Keep terminal rendering independent of transport. The host owns the PTY; the Swift app receives bytes and sends input. Existing Loopflow GhosttyKit is macOS-only. Upstream libghostty-vt building for iOS does not itself supply UIKit input and Metal rendering, and its API remains in development. [Swift manifest](../swift/Package.swift), [Ghostty library status](https://ghostty.org/docs/install/release-notes/1-4-0#libghostty)

Start a device spike with **Lakr233/libghostty-spm**: it offers host-managed I/O, native wrappers and UIKit input. Compare Termini's smaller custom-transport controller; study Rootshell and gterm for complete mobile interaction. Use SwiftTerm as an accessibility/input benchmark. These are candidates, not a measured ranking. [Native-mobile research](research-native-mobile.md)

Prove IME marked text, dictation, multiline paste, hardware keys, selection, VoiceOver, keyboard resizing and hidden-surface power behavior. Rendering a demo is insufficient. A phone file path is not a remote file attachment; defer transfer or implement it explicitly. Preserve renderer reply suppression during replay.

The MVP remains useful entirely in the foreground. Background completion alerts require a separate APNs delivery design, possibly deployed beside the future hub. No background socket promise is implied. The three HTML prototypes explore terminal-first entry and navigation; they simulate behavior and do not establish native-device quality. [Prototype comparison](mobile-prototypes.html)

## What to study, and what each reference resolves

| Reference | Study for | Do not inherit automatically |
|---|---|---|
| Rex | Control/data API, ownership, bootstrap, server modes | Its full window/block model or embedded Tailscale. |
| Paseo | Detached workers, replay, geometry claims, hub permission boundary | Relay, provider runtime or separate workflow engine. |
| tmux / iTerm2 | Native presentation over persistent terminal state | A second session hierarchy under Loopflow. |
| WezTerm | SSH to a persistent mux, domains and version compatibility | Requirement to use SSH or its compatibility policy. |
| Mosh / Eternal Terminal | Screen convergence versus retained stream recovery | New UDP protocol or predictive echo in the MVP. |
| Apple / Tailscale / systemd | Real lifecycle, identity and installation constraints | “Background” as a guarantee of continuous availability. |
| Ghostty wrappers / SwiftTerm | Input, rendering, accessibility and packaging | Claims of device quality without measurement. |

Detailed primary-source comparisons and proof limits live in the seven research notes below. None of this research ran a native build, configured a service, or demonstrated end-to-end mobile connectivity.

## Prove these boundaries first

1. **Two clients, one Session.** Phone observes, takes control, opens its keyboard and returns control. One provider, one geometry, one query responder.
2. **Kill the frontend during work.** Same provider and Graph continue under real launchd and systemd. Reconnect to the same identities.
3. **Lose the reply after submission.** Semantic command lookup finds its original acceptance. Raw input is never silently replayed. Reconnect beyond terminal retention gets a fresh screen.
4. **Clean-host pairing.** Ordinary Mac Tailscale variants and Linux; existing Serve routes remain intact. Test cellular access, logout, certificate renewal and revocation.
5. **Update with a busy owner.** Mobile/gateway/owner version skew and schema floors remain explicit. No forced duplicate conversation.
6. **Paper-test the future hub.** Duplicate event, offline Machine, changed owner and lost receipt. Fixed execution recipient and ordinary transfer preserve authority. Build no hub feature to run this exercise.

## Research record

- [Authority and future hub](research-authority-hub.md)
- [Runtime lifecycle and installation](research-runtime-lifecycle.md)
- [Shared Sessions and terminal ownership](research-shared-sessions.md)
- [Reconnection and uncertain delivery](research-recovery.md)
- [Application protocol and transport](research-protocol.md)
- [Trust, pairing and automatic setup](research-trust-setup.md)
- [Native mobile and terminal libraries](research-native-mobile.md)
- [Prior lfd attempts](lfd-history.md)

Selected direction records Jack Heart's statements. Recommendations and experiments remain unapproved architecture choices. Research is source review, not evidence that the proposed system is implemented.

## Page verification

Chrome/Playwright checks covered all three architecture views, six scenario selections, component inspection, and narrow dark-mode layout. The three prototypes passed their simulated control, input, paste, disconnect/reconnect and pairing flows. All five pages have valid local file links and no external runtime dependencies. Screenshots were visually inspected. These checks validate the HTML studies, not native terminal behavior or network/process survival.

## Decisions still open

1. First carrier to prototype: **a. HTTPS/WSS + Serve** (recommended); **b. SSH carrying the same application operations**. Host Tailscale is already selected.
2. Initial hosting promise: **a. personal Mac while logged in plus always-on Linux**; **b. also unattended Mac before login**, requiring separate packaging and credential proof.
3. Session input handoff and terminal geometry when the phone opens a Task already viewed on Desktop. Existing open Tasks → editable Session is selected.
4. First-release alerts: **a. foreground only**; **b. background push**, with a separately scoped provider service. The future webhook hub remains optional either way.
