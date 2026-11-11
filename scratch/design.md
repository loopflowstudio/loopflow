# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout documents the planned contract; implementation and authenticated
acceptance await LOO-718.
The listener, mobile settings and bulk protocol are outside this Task.

## Transport dependency — November 11, 2026 UTC

At 22:06 UTC, `gh pr view 1812 --json state,mergedAt,headRefOid,url,autoMergeRequest`
reported `OPEN`, `mergedAt: null`, `autoMergeRequest: null`, and head
`2a23883cb399fc5d7de180a024cc8fac4e49fdbe`. Fetched main remained at
`1570bef610a0a0cee0402ecc37d3a31461b213a8`, still without
`server/mobile.rs` or `server/ticket.rs`. Landing PR #1812 is outside this Task;
reinspect its shared store and endpoints after it lands.

The implementation findings below describe the previously inspected head,
`bfd4244858864279ca13450c9f5f102c5554643c`. Both WebSocket paths authenticate
with node/user identity, but foreign terminal attempts remain unconsumed and
expiry uses `Instant`. Only bulk tickets have direct store coverage; background
expiry releases abandoned bulk reservations. `mobile-same-device` permits the
host's own node without changing ticket identity.

The last successful LOO-718 status read, at 17:27 UTC, reported `feature` / `demo`.
The 18:04 LOO-809 status read was cancelled without a result at 18:05; its service
state remains unknown. Earlier observations and their history references are in
`966ba96e44ba13b13f86f6ca09aca9f90e3740f3:scratch/design.md`; keep the latest
dependency observation here rather than repeating unchanged polls.
Desktop QA remains deferred in `questions.md`.

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

Restore terminal consume-on-attempt in the shared store. Test matching node/user
success once, replay refusal, different node, different user, and both directions
of local/remote mismatch. Each mismatch must consume the ticket.

Through authenticated control and bridge connections, test ticket issuance,
one successful bridge, replay refusal, and consumption after an admitted
different node or local/remote mismatch. The transport admits only the host
owner's Tailscale user: another user's connection is rejected before WebSocket
upgrade and cannot present a ticket. Test that rejection on both endpoints;
different-user redemption belongs in the shared-store test. Keep authentication
policy unchanged.

Replace ticket timing with `SystemTime`; test expiry at 60 seconds with a clock
advance representing host sleep, expired-ticket sweeping, and unchanged bulk
redemption behavior, including release of abandoned bulk reservations without
another request. Leave unrelated throttles on `Instant` and retain watch resumption.

Keep the remote contract linked to shared attachment and recovery rules rather
than repeating them. Mark it planned until integration, then update its revision
4 anchor to the transport's revision 5 section. Keep phone prerequisites in
`docs/server.md` consistent.

`git diff --check` — passed; prose-only edits, no build or test rerun.
