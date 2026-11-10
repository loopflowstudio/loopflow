# What T3 Code, Paradev, Orca and Paseo teach Loopflow

2026-11-07 · Four independent competitor investigations, with a shared review of LOO-593's actual server implementation. No competitor app was run. Code links pin the inspected commits; docs and public feedback were checked separately. Current main can differ from installed releases. Public praise is anecdotal, not a representative survey.

**The strongest common pattern is one authoritative host with several client surfaces. The largest product distinction is how each system treats the conversation when the surface changes.** A shared terminal, native-session resume, conversation fork and live ownership transfer are different capabilities.

The protocol/architecture note was sent to LOO-593. Its immediate finding is that the server's 64-message channel does not bound all response memory or admitted requests. The rest largely supports 593's existing ownership boundaries or informs later remote work.

## At a glance

| Product | Architectural center and strongest code | Mobile ↔ desktop | Terminal ↔ structured chat |
| --- | --- | --- | --- |
| T3 Code | Host-owned threads; shared client runtime; transactional intent/outbox; bounded replay; explicit provider handoffs | Same host through Connect/tunnel, direct network, tailnet or SSH routes; snapshot/replay and reconnect supervision | Both surfaces exist, but shell PTY is separate from agent chat. Best-effort native import; no universal live TUI/chat toggle found. |
| Paradev | Hosted worktree workspace; reusable host client; separate chat protocol/runtime/UI; precise terminal recovery | Account/org → relay → local host, or hosted sandbox; HTTP operations and WS streams | Both coexist. Current code forks in either direction and keeps the source running, contrary to same-session wording in docs. |
| Orca | Workspace IDE plus remote runtime; reusable process host; fenced session records; render-aware terminal recovery | Paired direct or E2EE-relayed RPC to desktop/headless host; terminal replay and journal catch-up | Terminal-backed chat can share the PTY. CLI → structured chat can resume original after writer checks. Structured → CLI forks/copies. |
| Paseo | Shared app/SDK over daemon/provider adapters; owned subscriptions; independent terminal worker | Direct WS or optional E2EE relay; latest-state directory sync, timeline recovery, terminal replay | Both coexist. Same native conversation can make a manual import → CLI resume → Reload round trip; no atomic live handoff established. |

Sources: [T3 connection runtime](https://github.com/pingdotgg/t3code/blob/71c2d3a302092b94511eb7e6ab4e6844a66dbf22/docs/internals/connection-runtime.md), [Paradev fork implementation](https://github.com/ParadevAI/Paradev/blob/61645f4852d3f7ecca7f58a6f6d68d7bc9dc460c/apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspace/%24workspaceId/hooks/usePaneRegistry/hooks/useSurfaceFork/useSurfaceFork.ts), [Orca fork arguments](https://github.com/stablyai/orca/blob/e799cb419e89e1aa52f49feec0e273f8f613d737/src/shared/agent-launch/startup/agent-resume-argv.ts), [Paseo reload lifecycle](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/agent/agent-manager.ts#L1743).

## The architectural comparison

Loopflow gives durable intent and execution authority first-class representations: Waves, Tasks, reusable Skill Graphs, LfSessions, provider conversations, actual Processes and attachment claims. A direct Skill remains useful without the rest. Its Rust engine shares domain behavior across CLI and native surfaces; 593 makes Desktop use that engine through a supervised local server. The server bridges existing execution owners instead of becoming the owner of every PTY. [Loopflow models](/Users/jack/src/loopflow.agent/docs/architecture/reference-models.md), [593 protocol](/Users/jack/src/loopflow.loo-593-decide-the-desktop/rust/loopflow/src/server/README.md).

The competitors emphasize an interactive workspace or thread and build a portable experience around it. All four also have meaningful automation/orchestration; calling them mere GUI wrappers would miss their current architecture. Their distinctive advantage over the inspected Loopflow surfaces is how much of the presentation, connection management and interaction model already works across desktop and phone. Their TypeScript/React ecosystems make more client behavior shareable; Loopflow's Rust/Swift split instead places greater weight on a clean wire contract and native presentation.

**T3's best idea is making app conversation continuity explicit across providers.** It distinguishes reusing a runtime, changing model, restarting/resuming and creating another provider conversation with a context handoff. The handoff selects actual history within a budget and records omissions. Its transactional command receipt/event/projection/outbox boundary separates accepted intent from provider side effects. This enables a single visible thread to use multiple providers without claiming their native contexts are identical. [Transition policy](https://github.com/pingdotgg/t3code/blob/71c2d3a302092b94511eb7e6ab4e6844a66dbf22/apps/server/src/orchestration-v2/ProviderSessionTransitionPolicy.ts), [EventSink](https://github.com/pingdotgg/t3code/blob/71c2d3a302092b94511eb7e6ab4e6844a66dbf22/apps/server/src/orchestration-v2/EventSink.ts).

**Paradev's best idea is a transport-independent workspace host with carefully separated surfaces.** Its host client works without Electron, and structured chat has its own protocol/runtime/UI. Terminal recovery tracks bytes, epoch, geometry, visibility and focus. That enables a rich workspace on multiple devices while leaving execution on the host. Its fork UI is also more honest than its current docs: separate writers receive separate conversations. [Host client](https://github.com/ParadevAI/Paradev/blob/61645f4852d3f7ecca7f58a6f6d68d7bc9dc460c/packages/host-client/src/transport/transport.ts), [terminal code](https://github.com/ParadevAI/Paradev/blob/61645f4852d3f7ecca7f58a6f6d68d7bc9dc460c/packages/host-service/src/terminal/terminal.ts).

**Orca's best idea is precise recovery across heterogeneous execution and display surfaces.** The reusable process host serves desktop, CLI and remote modes. Terminal resume checks what the client rendered, retained coverage, geometry and stream identity; structured history has separate cursor/reset semantics. Its session model includes process leases and fences, making it closer to Loopflow's authority concerns than marketing alone suggests. [Process host](https://github.com/stablyai/orca/blob/e799cb419e89e1aa52f49feec0e273f8f613d737/src/packages/process-host/README.md), [session model](https://github.com/stablyai/orca/blob/e799cb419e89e1aa52f49feec0e273f8f613d737/src/shared/agent-session/lifecycle/agent-session-record.ts), [terminal resume ledger](https://github.com/stablyai/orca/blob/e799cb419e89e1aa52f49feec0e273f8f613d737/src/main/runtime/rpc/methods/terminal/terminal-stream-resume-ledger.ts).

**Paseo's best idea is a reusable remote-agent boundary that preserves provider-native history.** The shared SDK owns observations; physical sockets own subscription lifetimes; native providers own durable transcript history. Its directory sync keeps only latest entity values and bounded deletion records, while creation receipts persist resource identity and uncertain outcomes. An independent terminal worker preserves PTYs through daemon replacement. These are relatively small mechanisms with clear owners. [Subscriptions](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/session/owned-subscriptions/index.ts), [creation receipts](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/creation/index.ts), [terminal worker](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/terminal/terminal-worker-launch.ts).

These are strengths, not proven exclusive inventions. Multi-provider support, worktrees and remote access are shared across the group.

## What “sync” means in practice

```text
Desktop client ─────┐
Phone client ───────┼── authenticated connection ── authoritative host
Web / CLI client ──┘                              ├─ agents + provider history
                                                 ├─ files + Git worktrees
                                                 └─ terminal execution owners

Reconnect recovers a view of the host. It does not move the agent to the phone.
```

The important distinctions are in recovery and admission:

- T3 shares connection supervision across clients, separates connection readiness from synchronized data, and uses bounded snapshot/replay. It can select routes to a stable environment identity. Its Connect path is a trusted tunnel/bootstrap architecture; that should not be casually labeled E2EE. [Remote design](https://github.com/pingdotgg/t3code/blob/71c2d3a302092b94511eb7e6ab4e6844a66dbf22/docs/internals/t3-connect.md).
- Paradev routes an authenticated account/organization to the host. Terminals resume by byte position; structured chat replays a durable journal. Its smallest-visible-viewer geometry policy differs from Loopflow's explicit claimant. [Relay](https://github.com/ParadevAI/Paradev/blob/61645f4852d3f7ecca7f58a6f6d68d7bc9dc460c/apps/relay/src/host-tunnel.ts).
- Orca pairs devices and uses direct or endpoint-encrypted relay transport. Device-bound one-use terminal tokens, coverage checks and flow control keep stale streams from affecting replacements. [Tokens](https://github.com/stablyai/orca/blob/e799cb419e89e1aa52f49feec0e273f8f613d737/src/main/runtime/rpc/methods/terminal/terminal-stream-tokens.ts).
- Paseo independently recovers directory state, timeline history and terminal screen. Its protected relay mode uses Noise; legacy compatibility has an explicitly documented replay limitation. Its timeline is rebuilt from provider history, not an authoritative durable app event log. [Directory sync](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/directory-sync/index.ts), [security construction](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/docs/relay-endpoint-security.md).

None of these mechanisms establishes offline multiwriter repository synchronization or live migration of a provider process between computers. The selected host needs to remain available for new work.

## What people praise

| Product | Supported by public experience reports | Counterevidence / limits |
| --- | --- | --- |
| T3 | Multi-environment inbox, provider switching, quote chips, per-turn diffs and remote sessions | Startup/resource overhead and uneven harness customization; small HN sample |
| Paradev / Superset | Intuitive worktree workflow and access to native terminal agents | Editor/navigation gaps, setup friction, UI/bloat criticism; some comments describe coworkers rather than direct use |
| Orca | Managing multiple projects, floating workspaces, keyboard flow and orchestration | Focus bugs, coordinator cost and preference for CLI in some workflows |
| Paseo | Cohesive smooth UI and relay connection convenience | Voice timeouts/formatting and too many taps for model/agent switching |

Sources: [T3 firsthand HN discussion](https://news.ycombinator.com/item?id=50366117), [Paradev worktree account](https://www.reddit.com/r/SideProject/comments/1rthj7v/i_built_a_supersetshstyle_workflow_inside_a_fork/), [Orca floating workspace feedback](https://github.com/stablyai/orca/issues/36351), [Orca multi-project account](https://note.com/gadgettakayuki/n/n49e9874d175c?hl=en), [Paseo month-long user report](https://www.reddit.com/r/ClaudeCode/comments/1s4tml6/i_built_a_super_polished_mobile_app_for_claude/).

A recurring UX benefit is keeping context visible while moving between agents, projects and devices. T3's quotation chips are especially worth examining: selecting an artifact fragment becomes precise prompt context without manual transcription. “Has voice,” “has mobile,” and “has a diff viewer” are feature facts; the available feedback does not prove every such feature is loved.

## What went to 593

The [full note](competitor-protocol-notes.md) separates one immediate candidate from future design lessons:

1. **Bound admitted work and retained bytes.** The 64-message channel leaves completed replies retained in waiting producers, and request admission is uncapped. T3 demonstrates accounting across those boundaries. This is source-derived; no memory failure was reproduced.
2. **Preserve distinct recovery contracts.** Current complete Work readings are reasonable. Terminal byte continuation and future structured-chat history need different rules. Do not introduce universal event sourcing to copy competitors.
3. **Keep ownership with existing execution owners.** Preserve separate terminal/control connections, exact-owner reconnect, explicit geometry claims and no replay of uncertain input. Add only missing coverage for sleep/resize/replacement/overflow races.
4. **Keep unknown mutation outcomes honest.** Future retry-safe creation can use durable request identity and receipts, but a request ID or in-memory dedupe cache is insufficient across restart.
5. **Name fork, resume and live attachment separately.** Current competitor docs obscure this distinction; their code makes it consequential.
6. **Keep remote admission separate from the local socket.** A relay around revision 2 is not a remote authorization design.

## Evidence and remaining questions

Detailed implementation and source records: [T3 Code](competitor-t3code.md), [Paradev](competitor-paradev.md), [Orca](competitor-orca.md), [Paseo](competitor-paseo.md).

1. Which installed release pairs contain the current fork behavior in Paradev and Orca? Source and docs disagree; this investigation establishes current code, not every shipping binary.
2. How do the four products compare under the same phone-sleep, output-flood and host-update workload? Code inspection identifies mechanisms; it does not measure reliability or latency.
3. Does 593's expected local workload justify a particular admission/byte budget, and how should oversized one-shot results be handled? The missing aggregate bound is visible; its practical limit needs a measured workload.
