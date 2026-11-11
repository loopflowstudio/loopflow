# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout currently adds documentation of the intended contract only;
remote ticket behavior and authenticated acceptance remain unimplemented.
The listener, mobile settings and bulk protocol are outside this Task.
The current `server/service.rs::Tickets` consumes local tickets before checking
UID and expiry, with direct coverage in
`a_ticket_opens_one_bridge_for_its_user_within_its_lifetime`; its clock is still
`Instant`. Remote acceptance cannot run against this local-only implementation.

## Transport dependency — November 11, 2026 UTC

At 20:53 and 21:27 UTC, `gh pr view 1812 --json state,mergedAt,headRefOid,url,autoMergeRequest`
reported `OPEN`, `mergedAt: null`, `autoMergeRequest: null`, and head
`bfd4244858864279ca13450c9f5f102c5554643c`, unchanged from 20:50 UTC.
Fetched main remained at
`1570bef610a0a0cee0402ecc37d3a31461b213a8`; `git ls-tree` confirmed neither
`server/mobile.rs` nor `server/ticket.rs` was present. Implementation and
authenticated acceptance depend on LOO-718 landing; landing PR #1812 is outside
this Task. Reinspect its shared store and endpoints after it lands.

Inspection of that PR head found both WebSocket paths authenticate with node/user identity,
but foreign terminal attempts remain unconsumed and expiry uses `Instant`.
Only bulk tickets have direct store coverage; background expiry already releases
abandoned bulk reservations. `mobile-same-device` permits the host's own node
without changing ticket identity.

The last successful LOO-718 status read, at 17:27 UTC, reported `feature` / `demo`.
The 18:04 LOO-809 status read was cancelled without a result at 18:05; its service
state remains unknown. Earlier observations, cancelled reads and references to
the conflicting status report remain in
`9fbc70728eeaac9884bfa3dce86ffe20ed5123ff:scratch/design.md`.
Desktop QA remains deferred in `questions.md`; no product choice is open.

## Delete — do not maintain

The inspected transport revision already replaces `server/service.rs::Ticket`
and `Tickets` with `server/ticket.rs` and deletes
`a_ticket_opens_one_bridge_for_its_user_within_its_lifetime`. Keep that cut;
restore the terminal coverage below on the shared store. Retain bulk cases,
service-level bridge tests and wire fixtures. Do not extend the UID-only store
while waiting for transport to land.

The transport represents local admission as `Admission::Local` after checking
the socket peer UID against the server's effective UID. Keep that admission
model; local/remote separation does not require a second per-ticket UID field.

## Remaining implementation and acceptance

Restore terminal consume-on-attempt in the shared store without changing bulk
redemption policy. Test matching node/user success once, different node,
different user, and both local/remote mismatches in the shared store; every
terminal redemption attempt consumes the ticket, including mismatches.

Through authenticated control and bridge connections, test ticket issuance,
one successful bridge, replay refusal, and consumption after an admitted
different node or local/remote mismatch. The transport admits only the host
owner's Tailscale user: another user's connection is rejected before WebSocket
upgrade and cannot present a ticket. Test that rejection on both endpoints
separately from different-user redemption in the shared store. Local UID tests
alone cannot satisfy remote acceptance; authentication policy stays unchanged.

Replace ticket timing with `SystemTime`; test expiry at 60 seconds with a clock
advance representing host sleep, expired-ticket sweeping, and unchanged bulk
redemption behavior, including release of abandoned bulk reservations without
another request. Leave unrelated throttles on `Instant` and retain watch resumption.

Read the shipping docs before implementation: `docs/server.md` contains phone
prerequisites and Desktop controls; `rust/loopflow/src/server/README.md` contains
the ticket exchange and protocol details. Shared attachment and recovery
rules have one source; the remote contract adds identity and sleep-aware expiry.
Its example describes the WebSocket endpoints and links to shared semantics,
avoiding a second description of the binary protocol and recovery behavior.
Keep pending behavior marked planned until integration, then update the remote
example's revision 4 anchor to revision 5 with the transport's protocol section.

`git diff --check` — passed; prose-only edits, no build or test rerun.
