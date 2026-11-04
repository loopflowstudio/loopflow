# Machine authority and an optional public hub

Research for Jack Heart · 2026-11-03 · Proposal, not an implementation commitment

**Recommendation:** make the private Machine service the remote entry point to existing Loopflow operations. Design an eventual hub as durable event ingress that can additionally become an explicitly authorized automation client. Keep planning authority, Graph execution and Session lifetime in their existing owners. A hub may request work; it does not need a second workflow engine or a copy of the Machine's execution database.

This supports native mobile over host Tailscale now, an always-on remote Machine without Desktop, and public webhooks later. Hub availability should have no bearing on direct mobile access.

## What already owns what

**Observed:** a Machine is one OS user's authority in one data directory. It owns processes, credentials, local records and locks; its address can change without changing its identity. Two OS users on one Tailscale host can therefore be two Loopflow Machines. Pairing must identify the Machine, not merely the Tailscale node. Planning intent is synchronized through its configured provider or journal; execution facts remain on their producing Machine. See [Machines](../docs/architecture/machines.md) and [planning authority](../docs/architecture/planning.md).

The existing [`Read`](../rust/loopflow/src/lf/commands/read.rs) and [`Write`](../rust/loopflow/src/lf/commands/write.rs) types call the same operations as CLI commands. Reads also expose subscriptions. They are useful transport boundaries, but the typed write set does not yet represent every execution command. Adding HTTP alone would not produce a complete automation API.

The significant limitation is **Task ownership is not a distributed exclusion lock**. [`read_log`](../rust/loopflow/src/store/sqlite/task_ownership.rs) preserves an open local claim when another Machine claims later. It exposes `superseded_by`; `notify_superseded_in` explicitly leaves execution running. This preserves work during disagreement. It cannot guarantee that two consumers of one webhook will not both start work.

Actual transfer has richer behavior: [`send_task`](../rust/loopflow/src/ops/human_session/moves.rs) stops Graphs, checkpoints the checkout, transfers conversation history and Graph position, and records a reusable handoff. A hub changing a routing row is not equivalent to moving a running Task.

The two previous lfd generations demonstrate both demand and implementation difficulty. A generic execution adapter and a narrowly scoped webhook daemon were already tried; neither stayed small by declaration alone. The architectural improvement to prove is that replacing or losing a network service cannot change execution ownership. See [lfd history](lfd-history.md).

## Compare the hub choices

| Choice | What it makes easier | Cost or limitation |
|---|---|---|
| Durable event inbox only | Public ingress, signature verification, offline retention; Machines remain fully autonomous | Every installation still needs trigger policy and a deliberate execution recipient; inbox alone does not solve automation |
| Inbox plus authorized automation client | Shared trigger setup, event-to-action audit, routing and dispatch while laptops sleep | Hub holds meaningful execution authority; needs command receipts, revocation and explicit attribution |
| Central execution/control service | Global scheduler and fleet-wide policy | Moves authority into a mandatory control plane, duplicates or relocates existing Graph/Task responsibilities; hardest migration |

**Recommendation:** choose the second as the eventual architecture, with execution authorization disabled unless configured. Its first implementation could expose only the inbox. Do not commit the product permanently to an inbox-only hub if the desired benefit is “this Linear event starts useful work.” Conversely, connecting an inbox must not silently confer that authority.

Paseo demonstrates that distinction directly: connecting a daemon does not grant `hub.execute`. That grant includes creating and continuing agents and interacting with existing work, while terminal and credential administration require separate permissions. The useful lesson is that enrollment and execution authorization are separate. Its permission is broad, not isolation of hub-created work. [Paseo security](https://paseo.sh/docs/hub/security)

Paseo's concepts page describes a hub executing ordered workflow steps. Its current API instead documents self-contained triggers and calls the older workflow configuration legacy. This documentation transition is a reason to study behavior and authority boundaries rather than copy its object model. Loopflow already has Graphs. [Paseo concepts](https://paseo.sh/docs/hub/concepts), [Paseo API](https://paseo.sh/docs/hub/api)

## Proposed flow and authority boundaries

```text
Swift mobile ── host Tailscale ──► private Machine service
                                      │
                             existing lf operations
                                      │
                       Work / Graph runner / Session owners

GitHub / Linear ── public HTTPS ──► optional hub
                                     │ durable inbox + dispatch receipts
                                     │
Machine-initiated outbound connection ◄┘
       delivers a request into the same Machine command boundary
```

An outbound connection lets a public hub serve private Machines without becoming a terminal relay or joining each tailnet. GitHub Actions uses outbound HTTPS for runner communication; this is an established deployment pattern. Its interchangeable-runner routing should not be copied for a Task with a persistent checkout and active conversation. [Self-hosted runner communication and routing](https://docs.github.com/en/actions/reference/runners/self-hosted-runners)

Proposed authority map:

| Component | Owns | May request |
|---|---|---|
| Mobile/Desktop | Pairing credential, preferences, cached views | Operations on explicitly paired Machines |
| Machine service | Authentication, network connections, command admission receipts | Existing Rust operations and attachment to Session owners |
| Existing Machine runtime | Work store, execution records, local credentials, actual processes | Provider operations through current adapters |
| Planning provider/journal | Shared planning intent and exchange ordering | No independent authority over OS processes |
| Optional hub | Provider connections, received events, trigger revision, dispatch outcome | Authorized operations on a selected Machine |

A hub trigger should identify one existing operation: for example, start an agreed Graph for a Task, or submit direction to an existing Session. The returned Task/Session/LfProcess identifiers become its reference. The Graph runner continues to choose steps and record their results. If future hub automation itself needs a multistep Graph, run the existing Graph implementation in a clearly designated execution environment; do not introduce a parallel “HubWorkflow” model.

Machine credentials remain on the Machine. Hub webhook signing secrets and any provider credentials required for ingress or reconciliation remain at the hub. A phone needs neither agent-provider credentials nor hub administrator credentials. External event actors and the principal authorizing automation must both remain visible; a GitHub comment must not become an anonymous local instruction.

## Delivery, offline behavior and multiple Machines

**Provider facts:** GitHub requires a timely response, recommends asynchronous processing, and preserves `X-GitHub-Delivery` across redelivery. It does not automatically retry failed webhook deliveries; repair requires manual/API redelivery or reconciliation. [GitHub best practices](https://docs.github.com/en/webhooks/using-webhooks/best-practices-for-using-webhooks), [failed deliveries](https://docs.github.com/en/webhooks/using-webhooks/handling-failed-webhook-deliveries)

Linear requires public HTTPS and a 200 response within five seconds. Failures retry after one minute, one hour and six hours; prolonged failure can disable the hook. It supplies a delivery UUID and HMAC signature. Verify authenticity and freshness on public receipt, then retain the verified receipt so an offline Machine can consume it later without pretending it just arrived from Linear. [Linear webhooks](https://linear.app/developers/webhooks)

**Proposed semantics:** persist the verified envelope before acknowledging the provider. Keep the provider delivery key distinct from the logical action key. Duplicate delivery suppression prevents duplicate inbox rows; a stable action key prevents a repeated trigger evaluation from launching the same action again. Neither alone prevents duplicate external effects after a crash.

For state-change events, refresh through existing provider acquisition/reconciliation before acting. Late events must not overwrite newer planning state. For explicit requests, retain the original request and actor as evidence; fetching current state cannot reconstruct a missed instruction. Reconciliation can repair current state, but it cannot guarantee recovery of every missed event's exact intent.

Distinguish **event fanout** from **execution dispatch**. Multiple Machines may refresh from one event. Only one named recipient receives a given execution dispatch. Existing Tasks follow their ownership and transfer path; unowned/new work needs a configured Machine selection policy. An unavailable owner means queued/offline, not permission for the first connected Machine to take over.

The receiving Machine needs a durable association from `(authorized principal, request ID)` to accepted operation, content and result/process ID. Reusing an ID with different content is an error; retrying the same request returns the recorded acceptance. An interrupted reply means “outcome unknown,” followed by receipt lookup, rather than launching again. This protocol is also necessary for mobile cellular reconnects and is worth designing now.

If a Task moves while a dispatch is pending, resolve its authority again before accepting new execution. A request already accepted stays attached to its original Machine until its outcome is known or the ordinary transfer operation settles it. Automatically retrying the same request elsewhere would bypass local deduplication. No globally exclusive lease is implied by today's ownership log.

## What to get right now, and what remains unproved

Reuse existing command operations, Work readers, provider receipts and Graph execution. [Planning writes](../rust/loopflow/src/store/sqlite/planning_write.rs) deliberately exclude execution columns; preserve that boundary. Reuse [detached Session ownership](../rust/loopflow/src/transport/README.md) so a service restart does not become an agent restart. Add a remote admission contract and attribution, not a second durable model of running work.

Current gaps: no general network server or hub inbox, no comprehensive authenticated remote command surface, and no generic durable request-ID contract in the inspected read/write adapters. Existing provider mutation receipts solve particular provider effects; they are not proof of remote-command idempotency. No implementation or failure experiment was performed in this research.

Before committing to hub automation, prove:

1. Kill the API after execution starts but before its reply; reconnect returns the original process and starts nothing twice.
2. Replay one event concurrently to two Machines; both can refresh, exactly one is dispatched execution.
3. Move a Task during pending dispatch and during accepted work; preserve existing transfer behavior and report uncertainty explicitly.
4. Disconnect the hub and restart Desktop; direct mobile and active Graphs still work.
5. Receive events while a Machine sleeps; expose queue age and use an explicit expiry/reconfirmation policy for stale action requests.

Unresolved product choices are the scope of delegated automation, attribution for unattended work, stale-request policy, and whether a managed hub is wanted. These need not delay mobile. The irreversible design mistake would be conflating Machine identity, transport reachability, planning ownership and permission to execute into one “connected server” flag.
