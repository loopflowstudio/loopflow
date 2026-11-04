# One Session, several devices

Research for Jack Heart · 2026-11-03 · recommendation, not an implemented mobile contract.

The useful invariant is **one running conversation, several independent views**. Keep execution, provider identity, terminal state and authority on its host. Desktop, iPhone and iPad choose their own presentation. Opening the phone should observe the existing Session without resizing its terminal, taking input, starting a provider, or rearranging Desktop. Host Tailscale carries the connection; these semantics should survive either SSH carriage or an application connection.

## What already exists

Loopflow already has detached execution owners and a framed Unix-socket viewer protocol. Input control and terminal appearance ownership are separate. `Attach` accepts `control:false` and `own:null`; that is a genuine passive observer. `own:request` is different: it queues for future size/color ownership, so it is inappropriate for passive phone viewing. `own:take` replaces the geometry owner. Controlling attachment issues a fresh token and revokes the former controller. The current CLI viewer generally leaves after losing control, although the server permits a viewer to remain. [Wire types](../rust/loopflow/src/transport/wire.rs), [claim/input handling](../rust/loopflow/src/transport.rs).

The owner retains a Ghostty terminal, bounded history, parser state and output. Fresh attachment receives a repaint; short reconnects can resume a bounded byte spool. Ended owners save a Ghostty-revision-specific snapshot. These are different things: live replay is a display bootstrap, retained bytes repair a short gap, and an ended snapshot is a historical preview. None proves that execution is still alive. Current repaint omits images and truncates history; snapshots are not portable across Ghostty revisions. [Transport contract](../rust/loopflow/src/transport/README.md), [repaint](../rust/loopflow/src/transport/terminal.rs), [snapshot format](../rust/loopflow/src/transport/snapshot.rs).

A subtle existing contract matters: the headless owner answers terminal queries **only when there is no input controller**. Otherwise the controlling viewer is expected to answer. Geometry may belong to a different viewer. Thus a phone taking input can also change which emulator supplies device and mode responses, even without taking geometry. This is source evidence, not a demonstrated mobile failure. [Output processing](../rust/loopflow/src/transport/output.rs), [controller detection](../rust/loopflow/src/transport.rs).

## State belongs in three places

| State | Authority | Recommended behavior |
|---|---|---|
| Loopflow Session, selected provider conversation, process identity | Host | Attach references these identities; reconnect creates none. |
| Transcript, tool lifecycle, program status, pending approval | Host/provider projection | Native views receive structured state with revisions and stable identities. |
| PTY size, screen, modes, program palette, retained scrollback | Execution owner | One canonical terminal; snapshots identify geometry and stream position. |
| Input controller and geometry owner | Host, per attachment | Separate revocable claims, visibly named by device. |
| Split ratios, selected tabs, zoom, font, search, scroll position | Device | Desktop arrangements never become the phone's required layout. |
| Draft text and selection | Device initially | No automatic cross-device draft overwrite or submission. |
| Readable cached screen/transcript | Device replica | Show freshness; cached content does not imply current control. |

Current Swift `MultiplexerStore` serializes focused, zoomed, hidden panes, split trees and drafts into `WorkPaneLayout`. That supports restoration, but this read does not establish a device-scoped persistence key. Audit storage before reusing it remotely. The portable reference should be “this pane shows Session S,” with each device retaining its own arrangement. [Layout model](../swift/Loopflow/Models/MultiplexerLayout.swift), [serialization/restoration](../swift/Loopflow/Models/MultiplexerStore.swift).

## An explicit attachment lifecycle

1. **Discover → observe.** Resolve the existing execution owner. Subscribe, install a snapshot at its recorded geometry, then apply later output. Keep input disabled and make no ownership request. Scrolling, selecting text, rotating or opening the keyboard cannot mutate the PTY.
2. **Observe → Use here.** An explicit action acquires input, receives a fresh fencing token and optionally takes geometry. Default to both for comfortable phone interaction, while offering “keep Desktop size” for occasional input. Show the resulting controller on both devices. Desktop remains an observer.
3. **Control → observe.** Explicit release or takeover removes input authority without closing the view. Geometry release has an independently documented successor policy. Passive observers must never become owners merely because somebody disconnected.
4. **Disconnect → recover.** Stop admitting input under that connection's token. Preserve bounded replay state, then resume or install a fresh snapshot. Reconnection restores observation first; reacquire control only if the server confirms that no later takeover superseded it.
5. **Execution ended.** Keep the last view readable. Continuing the provider conversation is a separate explicit operation; failed attachment, timeout or stale endpoint must never be interpreted as permission to start another provider.

The host must linearize claim changes and accepted input. Old-token input received after a takeover is rejected. Already accepted input may still execute; takeover cannot retract bytes delivered to the PTY. Expose that boundary rather than promising cancellation. The current wire associates the token with a connection and silently drops passive/stale input; a network API should provide an explicit lost-control result for semantic submissions.

## Geometry and terminal replies

A PTY has one effective rows/columns pair. Two independent full-screen TUI layouts cannot both describe that same process. A passive phone can pan or scale the canonical screen; it can render a separate native transcript at phone width. It should not reinterpret the terminal bytes at a different width and claim an equivalent screen. When the phone takes geometry, the running program receives a resize and Desktop observes that changed screen. Freeze the previous geometry when nobody owns it; coalesce keyboard/layout changes from the current owner.

Recommend making the host emulator the sole responder for supported cursor, mode and device queries in every attachment state. Client renderers suppress automatic replies during replay and live display. Explicitly negotiated client-dependent effects—clipboard access, for example—need a designated client and policy, not several raw emulator responses. This is an architectural preference requiring a compatibility spike: current Desktop obtains some behavior from its full Ghostty surface, and the headless VT library may not answer every extension it advertises. An alternative is one designated live responder plus headless fallback, with responder generation, capabilities and query handoff explicitly separate from input control.

## What the comparison teaches

**Rex:** ownership arbitrates disagreements such as terminal size and grants no other rights. Attach and ownership are distinct; bootstrap combines initial state and subscriptions. Its API describes snapshot/event sequence reconciliation and bounded recovery. Borrow those boundaries, not its entire block/window model: Loopflow's Session is a conversation, so equating it with a complete terminal workspace would distort existing concepts. [Rex API](https://www.superlogical.com/rex/docs/reference/api).

**tmux and iTerm2:** control mode separates command results, asynchronous pane output and native presentation. A control client does not affect size until it sends its size; flow control can pause a lagging pane and recover through capture. iTerm2 demonstrates persistent remote execution presented as native windows with ordinary local scrollback/search interactions. This makes tmux control mode a serious benchmark for attachment semantics and a possible implementation alternative, though adding tmux underneath existing owners would introduce another terminal/session hierarchy. [tmux control mode](https://github.com/tmux/tmux/wiki/Control-Mode), [iTerm2 integration](https://iterm2.com/documentation-tmux-integration.html).

**WezTerm:** a mux domain exposes remote windows/tabs to native UI, and SSH can start/connect its remote daemon through a Unix socket. Its compatible-version requirement is a useful warning: transport convenience does not eliminate client/server evolution. Borrow explicit host domains and bootstrap; retain Loopflow's identities. Predictive local echo is optional optimization, not a substitute for authoritative state. [WezTerm multiplexing](https://wezterm.org/multiplexing.html).

**Paseo:** one daemon size claimant distinguishes claim from ordinary geometry updates; idle clients cannot steal size. Replay uses captured geometry and suppresses generated replies. Viewer delivery is isolated, and output coalescing flushes the first chunk immediately, batching sustained bursts. Those are concrete acceptance criteria. Its separate structured timeline combines live updates with authoritative projected history; viewport focus is not a delivery correctness gate. Borrow these semantics without borrowing its relay. [Terminal performance](https://github.com/getpaseo/paseo/blob/main/docs/terminal-performance.md), [timeline synchronization](https://github.com/getpaseo/paseo/blob/main/docs/timeline-sync.md).

## Alternatives and unresolved assumptions

**Extend existing owners** is the strongest starting point: preserve Session/provider identity, expose observation and claims, reuse retained terminal state. **tmux control mode** buys mature terminal persistence but requires integrating two ownership models or replacing the existing one. **Structured-native-only** makes phone reading excellent but cannot represent every provider TUI, shell or arbitrary program. Prefer structured status/history alongside a faithful terminal, with capability-based availability; never scrape screen text into authoritative tool approvals.

Before selecting a wire protocol, prove snapshot/live ordering under resizing; renderer reply suppression; headless query coverage; background suspension and token revocation; and whether present “Move here” paths terminate the old view. Existing viewer tests cover substantial repaint fidelity, including modes and alternate screens, but do not establish the complete two-device lifecycle. The current 60-second spool and revision-bound snapshot are implementation choices, not a mobile reliability guarantee. [Viewer tests](../rust/loopflow/src/transport/viewer_tests.rs), [transport](../rust/loopflow/src/transport/README.md).

## Prototype acceptance cases

- Open a 40-column phone while Desktop runs a 120-column editor: zero PTY resizes, no new provider, Desktop input uninterrupted.
- Take over, rotate, reveal the keyboard, then take back on Desktop: one controller, one geometry owner, stale input refused, both screens converge.
- Issue cursor/device/mode queries with zero, one and two viewers, including during takeover: exactly one coherent response per query.
- Disconnect mid-output and mid-paste: repair display without duplicating accepted input or inventing a new conversation.
- Replay while output and resize continue: preserve alternate screen, Unicode, styles, keyboard modes and ordering; document image limitations visibly.
- Suspend the phone during an output flood: Desktop and execution remain responsive; recovery is bounded and scroll position remains local.
- Show native history and terminal for the same Session: both identify the same provider conversation and pending actions; changing views starts nothing.
