# History of lfd and mobile connectivity

2026-11-03 · Research requested by Jack Heart · Read-only implementation review

**Yes: a persistent Machine API would restore a responsibility previously served by lfd.** There were two materially different lfd generations, and both were deleted. The proposed optional webhook hub resembles the second generation's original purpose. Keeping execution in detached Session owners would distinguish the new design from the first generation, but “thin service” is a boundary to enforce, not a historically new idea.

The [deeper historical report](<reports/Lessons from earlier lfd attempts.md>) tests the shared-engine hypothesis. **Observed:** #304 fixed divergent failure bookkeeping around one executor; #825’s landed exec adapter did not replace all legacy handlers; the second daemon shared landing code but coordinated Local/Home owners. **Inference:** share complete operations and select lifecycle authority separately. Multiple adapters alone are not duplication, and current Desktop FFI already shares Rust operations. No evidence ranks duplication as the largest measured cause of either deletion.

## Chronology

| Date | Change | What it establishes |
| --- | --- | --- |
| Jan 14 | [Unix-socket daemon](https://github.com/loopflowstudio/loopflow/commit/6ea523cf27ebd57e59f21e1b6fa72c4f8ad97c54) | Python lfd centralized agent spawning, monitoring, triggers, session tracking and SQLite writes. `lfd install` registered launchd; the daemon survived app restarts. |
| Jan 28–Feb 5 | [Rust daemon](https://github.com/loopflowstudio/loopflow/commit/d99b430532), then [HTTP/WebSocket instead of gRPC, #274](https://github.com/loopflowstudio/loopflow/pull/274) | HTTP was chosen explicitly for mobile. lfd exposed Wave CRUD/run/stop/land, events and hooks; registered with Studio for remote discovery. It was an execution/control server, not merely a transport adapter. |
| Feb 24–26 | [Native iOS shells, #410](https://github.com/loopflowstudio/loopflow/pull/410), [foreground reconnect, #459](https://github.com/loopflowstudio/loopflow/pull/459) | Concerto had native Swift iPhone/iPad views, shared LoopflowCore state, remote-only iOS connections, reconnect and stream restart. GhosttyKit remained macOS-only. This was not the proposed native mobile terminal. |
| Apr 8 | [Mobile/relay/cursoring designs, #615](https://github.com/loopflowstudio/loopflow/pull/615) | Planned Studio WebSocket relay, mobile action/attention views and replay cursors. This PR added design documents; it does not prove the relay shipped. |
| May 18–19 | [Accountless remote pairing, #686](https://github.com/loopflowstudio/loopflow/pull/686) | `lf op pair` printed QR and `loopflow://pair` links. The phone reused HTTP/WebSocket, Keychain and certificate pinning. A 90-day lfd token replaced the need for a Studio account. Plain HTTP pairing was limited to Tailscale's address range; TLS used an external frontend. No embedded Tailscale, native lfd TLS, token renewal or new mobile write actions were included. |
| Jun 29–30 | [Remove Studio auth, #721](https://github.com/loopflowstudio/loopflow/pull/721), [native Mac host manager, #740](https://github.com/loopflowstudio/loopflow/pull/740) | Hosted discovery/token plumbing was removed because it made Loopflow centrally managed by default. Explicit host URL plus bearer token became the model. Native launchd became the preferred private Mac/Tailscale host deployment after Docker PATH/context/credential-helper/build friction. Standalone lfd without Desktop was already supported. |
| Jul 5–6 | [Collapse daemon subsystems, #801](https://github.com/loopflowstudio/loopflow/pull/801), [generic exec adapter, #825](https://github.com/loopflowstudio/loopflow/pull/825) | Wave execution moved toward a resident Wave model. Trigger machinery, Docker executor and Python clients were deleted. A state-free `POST /v0/exec` forwarded argv to `lf`, but legacy mutation handlers and remote REST reads survived. The landed commit includes Swift changes/deletions despite #825’s stale additive-only PR body. |
| Jul 14 | [First lfd deletion, #872](https://github.com/loopflowstudio/loopflow/pull/872) | Removed the daemon HTTP API, accumulated store/catalog, queue reconciler, fork machinery, token plumbing, exec endpoint, bundled daemon manager and what the PR calls the non-functional iOS shell. Retained one `lf` binary and a resident Wave server. |
| Jul 15 | [Wave Home and SSH, #895](https://github.com/loopflowstudio/loopflow/pull/895) | Remote authority became the owning OS account/location. Probe/start ran `lf` on that Home through SSH. The proposed GUI attach path was an SSH tunnel to the remote resident's loopback endpoint. |
| Jul 16 | [Second lfd introduced, #1012](https://github.com/loopflowstudio/loopflow/pull/1012) | Reintroduced lfd for exactly three routes: health, status, signed Linear webhook ingress. Durable inbox before acknowledgement, delivery deduplication and domain-event idempotency. Explicitly excluded a general API: reads and actions stayed `lf`. launchd/systemd user services supported. |
| Jul–Sep | [Home supervision, #1119](https://github.com/loopflowstudio/loopflow/pull/1119), [Sep inventory](https://github.com/loopflowstudio/loopflow/blob/42ed461d1ee718fb3f791750454d4eb99a3afd79/scratch/cutover/cut-i-lfd-research.md) | The small ingress daemon grew Wave lifecycle/reconciliation, landing claims/recovery, GitHub hooks, maintenance and installation coordination. It had both a launchd/systemd keeper and a tmux fallback lifecycle. Swift used `lf`; it no longer called lfd directly. |
| Sep 27–30 | [Deletion brief](https://github.com/loopflowstudio/loopflow/blob/42ed461d1ee718fb3f791750454d4eb99a3afd79/scratch/cutover/cut-i-delete-listeners.md), [second removal, #1360](https://github.com/loopflowstudio/loopflow/pull/1360) | Jack requested deletion of Wave listeners, residents and lfd. The landed cut removed webhook ingress, provider-delivery code, chat/runtime surfaces and daemon packaging: 35,333 lines removed, 421 added across 159 files. No schema migration changed in the landed cut. |

Dates above distinguish commit dates from merge dates where relevant. PR bodies and historical source are contemporaneous evidence, not instructions for current work.

## Why it was removed

The first deletion described speculative runtime infrastructure and collapsed ownership around Wave, Project and Task. It retained a persistent Wave server, so it was not a general rejection of all long-lived API processes.

The second deletion was an explicit functionality cut so planning and delivery could ship without maintaining the listener/resident/daemon/chat stack. The supporting inventory found surviving facts already had nonresident owners: Task polling called the same Linear reconciliation operation, CLI processes could supervise landings, authored files/store owned planning and metrics, and scheduled commands could perform periodic work. It also found duplicate webhook receivers and multiple daemon startup/supervision paths.

The Sep brief deliberately removed every HTTP server in `lf`, the second binary and keepers. That was the concrete product direction then. It does not establish a perpetual prohibition on a future mobile transport, but reintroducing one reopens the decision and its operating cost.

## SSH and Tailscale were already alternatives

The July Home design explicitly used SSH for remote commands and an SSH tunnel for remote resident attachment. The Sep inventory says remote Homes over Tailscale were unaffected by deleting lfd: SSH was already the transport; later lfd bound loopback and rejected nonloopback by default. It also noted webhooks needed public ingress and that laptop polling was the working path when public ingress was unavailable.

Thus two choices remain real: add a private network API adapter, or transport existing commands/attachment over SSH on Tailscale. The history does not settle which gives the better iOS experience.

## Comparison with the current proposal

- **Repeated:** native Swift client, remote HTTP/WebSocket service, standalone host deployment, launchd management, pairing, Tailscale-only private connectivity and stream recovery all have prior art here.
- **Potentially different, requiring proof:** current FFI already shares Rust reads/writes/subscriptions; detached Session owners permit a transport service that neither owns agent lifetime nor duplicates execution state. This difference must be proved by restarting the adapter without stopping Sessions.
- **Webhook hub:** the second lfd's initial scope is close, but it wrote into a Home's store. A separate public queued ingress service serving private Machines would have a different deployment/storage boundary. No evidence reviewed proves that exact separated hub was shipped.
- **Historical warning:** a general argv proxy already appeared in #825, and an explicitly tiny ingress-only daemon in #1012 grew into a Home supervisor. Calling a new component thin does not by itself prevent recurrence.
- **Old mobile proof limits:** native shells, simulator builds and HTTP/WebSocket pairing smoke tests existed. #872 later called the iOS shell non-functional. Those facts support partial implementation, not a demonstrated complete mobile terminal workflow.

## Evidence limits

Both local checkouts are shallow, but retained commits include the crucial histories and documents; GitHub PR reads corroborated merge status and intent. No deployed service inventory, historical production logs or hands-on old iOS build was examined. The April relay source is a proposal, not implementation proof. The Sep deletion's broad rationale is explicit; a claim that lfd was rejected for one specific incident, security breach or measured resource cost would exceed the evidence.
