# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout documents the planned contract; implementation and authenticated
acceptance await LOO-718.
The listener, mobile settings and bulk protocol are outside this Task.

Shipping docs to read during implementation: `docs/server.md` (phone access)
and `rust/loopflow/src/server/README.md` (attachment contract and recovery).
The existing examples and requirements remain the chosen direction; no product
question is open.

## Transport dependency

At November 12 00:44 and 01:18 UTC, `gh pr view 1812` still reported `OPEN`,
`mergedAt: null`, `autoMergeRequest: null`, at head
`4e0ccacdb3cb74212e0bdaacd1fc2ccf786cec43`. Fetching `origin/main` returned
`d382d0b72e4b7c41773e4c4070322387f8b4ce65`, without `server/mobile.rs` or
`server/ticket.rs`. Implementation and authenticated acceptance await LOO-718
landing; landing its PR is outside this Task. Reinspect the shared store and
endpoints when it lands.

Earlier dependency observations are recorded in
`6d8057efc7e0b4dcdc4605d924b173017e716a98:scratch/design.md`.

The November 11 23:27 UTC inspection of that head found
`remote.rs::connection` rejects unadmitted peers before either WebSocket upgrade.
`mobile.rs::permits_peer` binds admission to node/user identity;
`mobile-same-device` permits the host's own node without changing that identity.
`ticket.rs::Tickets::redeem` leaves tickets redeemable after foreign terminal
attempts and uses `Instant`. Its direct tests cover only bulk purposes, including
foreign attempts and background release of abandoned reservations. Reinspection
of `ticket.rs` at November 12 00:44 UTC found the same behavior. The current
checkout still uses the UID-bound store in `service.rs`; its redemption removes
the ticket before checking identity, as the local attachment docs describe.

The last successful LOO-718 status read, November 11 17:27 UTC, reported
`feature` / `demo`. On November 12 00:44 UTC, the Task status inspection did
not return and was cancelled; current Graph positions remain unverified.
Earlier observations and their history references are in
`4f9f541ecce0e6db4055d52b61fb55a9a8eab953:scratch/design.md`; repeated status
notes are consolidated here. Desktop QA remains deferred in `questions.md`.

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

The remote contract states its planned status at the entry point; the local
attachment section describes only the running protocol.

`git diff --check` — passed; prose-only edits, no build or test rerun.
