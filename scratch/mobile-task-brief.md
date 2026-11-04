# Use a native iOS terminal with Loopflow over host Tailscale

Jack Heart requested this Task on November 3, 2026 and explicitly selected the existing `loopflow.mobile` worktree and this ongoing conversation as its home. Preserve branch `jack-heart/mobile`, the conversation, research and HTML prototypes. Do not create another checkout or primary conversation. Jack subsequently invoked launch-plan: implement the native iOS lane from mobile-ios.md here while LOO-593 implements the local backend. Preserve this conversation and worktree; Auto-Ship is not approved.

## Accepted MVP interaction

Jack Heart confirmed: open the app to existing open Tasks, like Desktop’s working-task sidebar; tap a Task to go directly to its existing Session terminal; type, edit input, submit and respond to prompts there. No Wave navigation, shell panes or other window types are needed initially. No intermediate Task details, changes or checks screen is required. Jack subsequently selected the compact describe-first new-Task sheet in `scratch/explore.html`: an editable typed/dictated request and visible changeable repository/Machine destination, followed by Create & start. Backend design now covers that path alongside existing-Session attachment; detailed launch semantics remain proposed. This replaces the earlier choice among Sessions/Tasks/Focus: use the simplified Tasks prototype at `scratch/prototype-tasks.html`.

Jack also requested an input-focused prototype pass informed by Rex, Paseo, Claude, Blink and Happy, using Desktop’s styling: bundled typography, cream/burgundy light and slate dark colors, compact Task metadata and status markers. Demonstrate keyboard-open geometry, arrows/modifiers, multiline paste and a terminal approval. This refines the same two screens; native input and network behavior still require device proof.

Jack then requested fewer keys because the MVP targets agent Sessions rather than a generic shell. The revised study exposes Paste, New line and keyboard visibility by default; permission questions get contextual choice/Cancel/Confirm controls and working agents get Stop. Touch cursor editing uses native selection. Trustworthy provider state for conditional controls remains an implementation question, not a capability proven by the prototype.

## Outcome and selected direction

Build a native Swift iOS experience centered on a terminal connected to existing Loopflow work on a local or remote Machine over host-installed Tailscale. No embedded Tailscale node, custom Loopflow relay, or required public hub. Detect signed-in host Tailscale and automate endpoint discovery/setup and reconnection as far as supported APIs allow. Be explicit about setup that requires Tailscale sign-in, VPN consent or administrator action.

The Machine side should farm commands out to `lf`, retaining one implementation of work operations. Use native in-process subscriptions sharing the existing `lf watch` machinery in the proposed service. The phone is a client; an on-phone `lf` installation is not the plan. The proposed Machine endpoint runs independently of Desktop; define its standalone/headless installation and preserve the distinction from Desktop’s app-owned automation socket. `lfd` is a historical name, not a selected new name.

Study Rex heavily through its supported public APIs and behavior, with Paseo and the other references already researched. Earlier lfd attempts demonstrate both sustained product intent and real ownership/maintenance difficulties. Neither a blanket veto nor proof that a thin service will be easy follows from their deletion.

Plan the architecture for a future optional public hub receiving GitHub/Linear webhooks, including authority, authentication and delivery boundaries. Do not build webhook integrations, a hub, custom relay or broad automation platform in this Task.

## Backend design

Jack Heart requested concrete backend design after selecting new-Task flow 1, then proposed moving Desktop Work operations from the C binding into a socket protocol and studying Rex heavily. Jack accepted this general service/socket direction, then asked to test it against lfd history and the cost of multiple code paths. The current [backend draft](mobile-backend.md) specifies one Work contract over local Unix sockets and remote host Tailscale: native shared Rust reads/subscriptions, typed CLI command delegation and an independent terminal-owner bridge. The proposed cutover removes migrated Work FFI wiring rather than retaining a permanent parallel path. Ghostty rendering bindings and Desktop-only UI automation remain local. Proposed automatic local LaunchAgent setup is separate from enabling remote access; packaging and lifecycle guarantees remain to prove. See [installation and lifecycle research](<reports/Machine service installation and lifecycle.md>). The launch-plan handoff authorizes the local backend lane in LOO-593 and native frontend lane here; remote transport and other open product choices remain unsettled. [Historical findings](<reports/Lessons from earlier lfd attempts.md>) support one complete operation owning resolution, defaults, state writes, attribution and recovery; shared low-level functions alone did not prevent drift. CLI and socket adapters are allowed, CLI need not require the service, and no service-specific Task orchestration or business receipts should emerge. Current FFI already shares the engine. Prior deletion was a scope decision, not proof that a completed thin service failed.

## Design still to settle

- Exact Desktop Work socket cutover/removal inventory, automatic local service registration and one Desktop/CLI installation authority. Exact signed-bundle/standalone packaging remains open.
- Machine API versus SSH carrying the application protocol; HTTPS/WSS through Tailscale Serve is a research recommendation, not accepted transport.
- Process/PTY ownership and survival across Desktop exit, service restart/update, disconnect, sleep, logout and reboot; avoid duplicate retention authorities.
- Shared Session identity, observer/control handoff, canonical terminal geometry, terminal query replies, bounded output recovery and uncertain input delivery. Never silently resend raw keys or paste after reconnect.
- Typed client operations, subscriptions and version/capability negotiation, while command execution delegates to the CLI.
- Pairing, per-device revocation, account authority and safe automatic host setup. QR tickets and particular authentication schemes are proposals to validate.
- Input ownership and viewport behavior when opening the selected Task’s Session on a phone while Desktop is attached. The Task-list entry point and editable terminal are selected; the protocol must support that interaction.
- Native terminal library: evaluate libghostty wrappers on actual iOS, including IME, hardware keyboard, paste, selection, accessibility, keyboard resizing and power use. Current macOS Ghostty integration is not an iOS renderer.
- Initial hosting promise and foreground MVP versus separately scoped push notifications.

## Existing research and evidence

Read [the task plan](mobile-task-plan.md) for delivery outcomes and parallel backend/frontend work. [The backend design](mobile-backend.md) and [interactive review](mobile-backend-review.html) hold current architectural direction. `scratch/mobile-system.md`, `scratch/mobile-architecture.html` and `scratch/mobile-architecture.md` retain earlier synthesis and exploration. Detailed evidence: `scratch/lfd-history.md` and `scratch/research-{authority-hub,runtime-lifecycle,shared-sessions,recovery,protocol,trust-setup,native-mobile}.md`.

`scratch/mobile-prototypes.html` presents the selected Task-to-Session prototype, with earlier alternatives retained as references; `scratch/mobile-prototypes.md` records their scope and verification. HTML behavior is simulated, not evidence of a native build, network connectivity or process survival. Preserve the distinction between Jack's accepted direction and research recommendations. The later command-delegation and in-process-subscription discussion above supersedes any earlier ambiguous recommendation to execute commands in the service.

## Coordination

LOO-717 owns local/remote Desktop shell lifecycle and cleanup. LOO-498 owns Rex workspace opening/restoration. Both are already started; share their contracts rather than implement competing policies. LOO-593 owns Desktop server roles; coordinate proposed changes. LOO-709 owns agent idle lifetime and preserved interactive state; arbitrary shell state has different recovery needs. Inspect LOO-710 terminal snapshot fidelity and LOO-712 Rex-shaped Session transport before changing their surfaces. Preserve their active work and unresolved review choices.

## First implementation proof after design

Develop the local backend proof and native iOS client in parallel, sharing application semantics and executable fixtures. The backend proves representative Desktop reads/subscriptions, commands and Session attachment, including state/attribution parity, cancellation and installed-service restart. The frontend proves the selected Task UI, renderer and native input without requiring completed host setup. Live Tailscale integration then joins them: existing open Tasks → existing editable Session → reconnect to the same provider. Complete the remaining Desktop Work cutover alongside that integration and remove replaced FFI glue before supported release. The [task plan](mobile-task-plan.md) separates these outcomes and recoverable new-Task creation.

Define input/geometry handoff with Desktop, and reconnect without replay or duplicate providers. Prove behavior under phone backgrounding/network loss, Desktop exit and transport-service restart, with real Mac and Linux hosting evidence for whichever hosting promises are selected. Test uncertain command acceptance and reconnect beyond output retention. Record unsupported lifecycle boundaries honestly. Paper-test future hub duplicate delivery/offline/ownership cases without building it.

Use the existing research to reach reviewable architecture and the smallest working vertical slice; do not restart broad research or silently convert open choices into requirements.
