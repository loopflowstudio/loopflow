# LOO-718 native iOS implementation

Jack Heart requested sustained improvement toward an excellent iOS app, prioritizing
polish, performance and UX over feature count. Continue in this checkout and
PR #1812. Jack authorized the local LOO-593 protocol merge described below;
there is no PR landing or Auto-Ship authorization. Apple registration, signing for
distribution and TestFlight remain LOO-721; no Apple account work has been
performed here. The dated proof history (November 3–4) and its failure logs are
committed at `4070315ab3:scratch/mobile-ios.md`; this note keeps the live design,
decisions and remaining work.

## Selected scope and decisions

- **Scope (November 3, Jack Heart):** existing open Tasks → the selected existing
  Session terminal, with typing, editing, submission and prompt responses. No Wave
  view, generic shells, inspectors or changes/checks screen. Desktop typography and
  colors; Jack's November 3 steer reduced the default row to Paste, New line and
  keyboard visibility, superseding the arrow/modifier strip, with choice,
  Cancel/Confirm and Stop controls appearing contextually. Contextual controls
  stay absent until the backend supplies trustworthy program state. The
  implementation added a fourth control, Keys (arrows, Tab, Escape inline), on
  November 4 as the interim way to answer choices and interrupt; that addition
  has no recorded approval (see Unresolved product choice). Touch selection must
  work natively.
- **New-Task flow (November 3):** Jack selected describe-first flow 1 after
  reviewing three prototypes (“1 looks good”): a compact sheet, editable
  typed/dictated request, visible changeable destination, explicit Create & start
  through shared recoverable admission. Studies: `explore.html`,
  `prototype-tasks.html`; prototype notes at `4070315ab3:scratch/mobile-prototypes.md`
  and `mobile-new-task-flows.md`.
- **Connection (November 4):** Scan QR code is the default, Enter address the
  fallback. The QR carries only the Machine's HTTPS address. Host Tailscale
  supplies connectivity and caller identity; the Machine admits a distinct untagged
  node of the approved owner through `tailscale whois` and saves node/user with the
  endpoint. No Serve route, embedded node, phone ticket, relay or hub. Automatic
  discovery is deferred; the announcement probe experiment never passed a physical
  device test (`4070315ab3:scratch/mobile-tailscale-discovery.md`, probe sources
  `4070315ab3:scratch/tailscale-discovery/`).
- **Backend (November 4, LOO-593):** one shared server for Desktop and phone; Rust
  reads/subscriptions/commands with no second owner, scheduler or store; existing
  Session and Graph owners keep execution. Its design, deletion list and proof live
  with PR #1813 (`4070315ab3:scratch/mobile-backend.md`, `server-write-direction.md`,
  `direct-write-demo.md`, `server-cutover-*.md`, `server-performance/`). This
  checkout merged that branch at `4070315ab3`.
- **Protocol (November 5, Jack Heart):** server protocol revision 2 and the
  additions below. Design and survey: `4070315ab3:scratch/server-protocol-2.md`,
  `rex-server-comparison.md`.
- **Lanes:** A local server (LOO-593), B native client (LOO-718), C phone over host
  Tailscale, D Desktop Work cutover, E recoverable create/start, F start work from
  the phone. All are implemented in this checkout; supported release still needs
  the acceptance below. Full table: `4070315ab3:scratch/mobile-task-plan.md`.

## Current design

Native Swift (`swift/ios.yml` includes the shared project; SwiftTerm pinned).
UIKit supplies text input, composition, selection and accessible reading; the
renderer can be revisited on evidence without changing the protocol.

RemoteMobileClient holds one server connection while the app is in use. It
watches one all-repository WorkViewPlan subscription for Task primary Sessions,
rejects unavailable or truncated readings, caches the last list per verified
Machine, and offers explicit Open Session for a missing owner; navigation and
reconnect never start a provider. Attach asks `session.attach` for the Session’s current owner and ticket, then bridges
the owner's unchanged binary protocol on a connection of its own. OwnerStream
handles fragmented frames, nonce-qualified events, snapshot assembly at canonical
geometry, gaps and size bounds. Input and geometry are separate claims: opening
observes, Use keyboard takes control and waits for its replacement screen, the
bounded writer never retries raw input. Backgrounding disconnects; returning
opens one fresh observer attachment and preserves provider-owned drafts.
Observers keep the owner's grid; control fits the phone. Find, Latest, link
opening, compact-height chrome, Sessions sheet, program activity header, saved
requests and the describe-first creation sheet are implemented as recorded in the
proof history.

### Server protocol revision 2 on the phone — November 5

Jack Heart's steer: adopt revision 2, attach terminals by ticket rather than a
protocol switch, and bound control messages. Revision 2 is PR #1813's shape
(LOO-593, approved November 5), merged here: operations named by dotted `kind`,
`response {id, trace_id, result}` typed per kind, `subscribe {id, operation}`,
`checkpoint {id, checkpoint}` with integer checkpoints, readings without a
generation. Decisions taken in this slice:

- **One control connection** per client: hello/welcome, requests by id,
  subscriptions, checkpoints and cancel (`wire.rs`, `ServerProtocol.swift`). The
  phone opens it on first use per foreground session, reopens after a loss, and
  closes it on background (`MobileClient.suspend`). Each request waits at most
  15 s, then cancels its own id; the connection stays.
- **This Task's operations in that scheme:** `view.select {expression}` (every
  view plan; the phone's Task list and watch), `session.list` gains `task`,
  `task.start` and `session.start` run as the server's own `lf` child
  (`Write::delegated`), `mobile.status`, `mobile.configure {configuration}`,
  `terminal.attach {owner}` and `request.upload {bytes}` answer `{ticket}`.
- **Tickets** (`server/ticket.rs`): single use, 30 s, bound to the admission that
  asked (local UID, or remote node + user). Redeemed on a second connection:
  `/v1/ticket` remotely, a fresh Unix connection locally; first line `ticket`,
  answer `admitted`, then the purpose. One implementation serves both carriers.
- **Terminal attach:** `terminal.attach {owner}` answers `{ticket}`; the terminal connection
  redeems it and carries the unchanged owner protocol as binary frames. The
  bridge stays a byte copy with no store read; the control connection never
  becomes a byte stream.
- **Control limit 1 MiB** (`welcome.limits.max_message`); ticketed connections
  carry up to 64 MiB; larger is refused with `unavailable`. A control line over
  the limit is answered with a null-id `invalid` whose data names `max_message`,
  then the connection closes.
- **Per operation:** no paging. Any answer or reading above the limit spills
  whole: the server sends `bulk {id, ticket, bytes}` on the control connection
  and the client fetches the original message by ticket, off its reader, so other
  readings never wait behind it (`task_file`, `history`, `task_diff`, oversized
  view plans and session pages). Requests above the limit (Task save) ask
  `request.upload {bytes}` for a ticket, upload the one request line, and receive
  the answer on the control connection under the request's id. Measured on this Machine: plans
  61 KB for 43 Tasks, history 29 KB, a sessions page 179 KB; the spill is the
  exception path today.
- **Deleted:** revision-1 `MachineConnection` framing (four-byte prefix, 64 KiB
  chunk reassembly, one operation per WebSocket), `MachineRequest/Response/
  Operation`, `Reply::Bridged`, the `bootstrap` request and `tests/fixtures/dto/machine/`;
  with the merge, the untyped `Reply`/`ServerReply` envelope, `generation` on
  readings and the `command`, `bulk_request`, `all_view_plans` and `task_create`
  fixtures. Nothing speaks revision 1. Compress (November 5) also deleted
  `welcome.capabilities` and the phone's per-operation capability lists: the
  negotiated revision is the one compatibility check, and the Swift
  `mobile.status`/`mobile.configure` encoders, which no client sends (Desktop
  reads mobile setup through `lf server mobile`).

Checks (November 5, concept-review fixes): `cargo test -p loopflow --lib server::`
20 pass; `--test transport_tests server_` 17 pass (includes the portfolio-scoped
`view.select` watch); `--test served_reads` 5 pass (realign rerun; the earlier `task.hold`
name mismatch no longer reproduces); Desktop `--filter 'ServerClient|ServerProtocol|Registry'`
42 pass; iOS `LoopflowiOSTests` 81 pass, 2 skipped; clippy clean.
The host-Tailscale acceptance (`transport_tests mobile_native_acceptance`) and
physical-device proofs belong to demo.

The November 5 concept review's four items are in code: `view.select` and its
watch read only portfolio repositories (`lf desktop plans --all`); the portfolio
is `portfolio.get`/`portfolio.set`; `Write::TaskStart` is `task.start` and
`Write::TaskCreate` is `task.create` (Swift `.taskStart`); the operation table
lists every kind. The merge's `transport_tests` compile break (moving
`Terminal.child` out of a `Drop` type) is fixed with `wait_exit`.

### Reconcile with LOO-593 — November 7

Jack Heart's latest steer settles attachment by Session. The local merge takes
PR #1813's pinned `971897b8b982b3332b349d96db57c9aa0b2f04d5` into LOO-718 with
`lf sync --manual`; the preceding November 5 names are historical where they
conflict with this contract:

- One `session.attach {session, owner_lf_process_id?}` request resolves the owner
  and returns `{ticket, owner_lf_process_id, expires_in_ms}`. The phone reconnects
  by Session, with no cached owner ID and no provider start. An exact owner is
  available for Desktop's existing-owner recovery.
- One ticket store covers Unix and WebSocket terminal and bulk connections.
  Redemption is `attach {ticket}`; terminal tickets answer `bridged` before the
  unchanged binary stream, bulk tickets answer `admitted`. Lifetime is 60 s to
  match LOO-593. Terminal/bulk connections consume no control slot.
- Keep the 1 MiB control and 64 MiB bulk limits and off-reader fetches. Existing
  whole-message spill decisions stand; no second paged read shape is added.
- Preserve typed errors, request-local unknown-operation refusals, read
  projections (`select`) and subscription `ended`. The generated
  `lf server describe --json` schema includes this Task's phone operations.
- Portfolio naming follows LOO-593: `desktop.portfolio` and
  `desktop.portfolio.set`. The portfolio-wide `view.select` uses
  `lf desktop view list --all`, extending the existing list command.
- Retain shared request diagnostics and execution attribution. The two existing
  delegated launch operations remain child processes; serving reads or attaching
  a terminal creates no second execution or usage owner.

Validation on November 7: Rust server tests 33 pass; served reads 8 pass;
transport server tests 17 pass (2 opt-in tests ignored), ticket isolation and
control-slot tests 2 pass, startup/recovery tests 4 pass. The remote transport
proof now redeems a Session attachment over a separate TLS WebSocket and exchanges
the unchanged owner frames. Desktop protocol/client/query/pairing tests 52 pass;
iOS suite 81 executed, 2 skipped, no failures, followed by 18 passing focused
pairing/creation tests after fixture alignment. The schema snapshot agrees with
`lf server describe --json`; `cargo fmt` and all-target clippy pass. Physical-device and live host-Tailscale acceptance
remain below; fixture identity is not that proof.

## Unresolved product choice — November 5 concept review

The default key bar diverges from Jack Heart's accepted row. Evidence: the
steer (comment d72340bb) names one default row of Paste, New line and keyboard
visibility and supersedes the four-arrow strip; `SessionTerminalView.swift`
ships Paste / New line / Keys / keyboard, where Keys swaps in arrows, Tab and
Escape (commit 7e745926e3, November 4). The contextual controls the steer pairs
with that row (choices, Cancel/Confirm, Stop) are deferred until trustworthy
provider state exists, so Keys is today the only software way to send Escape
(interrupt) or arrow through a provider menu. Digits and Return already answer
numbered prompts by typing.

Question for Jack: until contextual controls exist, (a) keep Keys in the default
row as the interim, (b) remove it and answer prompts by typing only, accepting no
software interrupt, or (c) remove Keys and add only an Escape/Stop control.

## Remaining work and acceptance

The Simulator proofs over real HTTPS/WSS with a second-node identity fixture and
stand-in, Claude and Codex providers passed through November 4 (see the history).
Still open, in priority order:

1. Rerun the host-Tailscale native acceptance on revision 2 (terminal by ticket,
   32 MiB paced output, service interruption, creation recovery). Earlier
   sustained-output runs twice missed deadlines with misleading socket attribution
   and passed on rerun; the cause is unresolved and no batching change remains.
2. Physical phone over installed Tailscale: identity, network and account
   transitions, camera scanning, share sheet, cellular. Everything so far is
   Simulator evidence.
3. Installed Desktop lifecycle: Desktop and the phone reconnect after server
   replacement. Since #1833 (merged here at `c866a09fbb`) a reinstalled or
   superseded lf ends no idle owner and owners announce no successor; the phone's
   owner-changed disconnect path went with it. Still unproven: an attached phone
   bridge across `lf install`.
4. Phone ergonomics the Simulator cannot judge: CJK conversion, dictation,
   physical keys, VoiceOver, sustained-output energy, rapid submission timing
   (Codex's paste-burst detector is a candidate explanation for one timeout).
5. Multi-Machine selection, usage-accounting parity and contextual approval/Stop
   controls once the backend supplies action semantics.

Telemetry stays shared: command/Process attribution and connection diagnostics
without duplicate provider usage or persisted terminal contents.

## Demo

The offline fixture app (`--fixture`) shows Task selection, retained Session
editing, Session switching, Find and foreground recovery; `demo.html` and
`demo-assets/` hold the walkthrough and inspected captures (fixture screenshots
do not establish the live path). The live demo is Desktop's Mobile settings QR →
phone scan → open Task → edit the same Session → reconnect to the same provider.
Jack's November 4 review record: `4070315ab3:scratch/mobile-native-demo.md`.

## Research index

November 3 research for Jack Heart, historical evidence rather than direction,
is committed at `9e4587bb53fc4102e8682a3bba61e9776a6de5c2:scratch/` (lfd-history,
mobile-architecture, mobile-system, research-authority-hub, research-protocol,
research-recovery, research-runtime-lifecycle, research-shared-sessions,
research-trust-setup, research-native-mobile, rex-workspace-task-update,
desktop-shell-task-brief, mobile-task-brief) and `4070315ab3:scratch/reports/`,
`research_notes/`. HTML studies and `demo-assets/` stay in this checkout.

## November 9 sync checks

- `cargo test -p loopflow --lib server::`: 33 passed; stale schema regenerated, then `server::schema::tests::the_checked_in_schema_is_what_describe_prints` passed. macOS linker warned that the unwind table exceeds 16 MiB; gate/CI own static analysis.
- `swift test --package-path swift --filter 'ServerProtocolTests|RegistryQueryTests|ServerClientTests'`: 47 passed after aligning the mobile operation names with main. Device and host-Tailscale acceptance remain for demo.
