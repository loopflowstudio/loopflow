# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout documents the planned contract; implementation and authenticated
acceptance await LOO-718.
The listener, mobile settings and bulk protocol are outside this Task.

## Transport dependency

At November 12 02:37 UTC, GitHub's PR metadata still reported #1812 open and
unmerged, at head `4e0ccacdb3cb74212e0bdaacd1fc2ccf786cec43`.
Fetching `origin/main` returned `baa09168df59ae063d737a31f0e36b07e15c91d9`,
without `server/mobile.rs`, `server/remote.rs` or `server/ticket.rs`.
Reinspect when the shared transport lands; landing its PR is outside this Task.

Inspection of that head found
`remote.rs::connection` rejects unadmitted peers before either WebSocket upgrade.
`mobile.rs::permits_peer` binds admission to node/user identity;
`mobile-same-device` permits the host's own node without changing that identity.
`ticket.rs::Tickets::redeem` leaves tickets redeemable after foreign terminal
attempts and uses `Instant`. Its direct tests cover only bulk purposes, including
foreign attempts and background release of abandoned reservations.

The November 12 02:01 UTC `lf task status LOO-718 --json` inspection reported
`feature` / `demo`, with PR #1812 open and settlement not armed. LOO-809's
status read timed out after 25 seconds; its Graph position remains unverified.
Earlier observations and history references are in
`62b54e2153d74cdb5de68e8c81bc693cda85a94e:scratch/design.md`.
Desktop QA remains deferred in `questions.md`.

## Delete — do not maintain

Keep the transport's replacement of `server/service.rs::{Ticket, Tickets}`
with `server/ticket.rs`; do not extend the UID-only predecessor. Restore the
deleted `a_ticket_opens_one_bridge_for_its_user_within_its_lifetime` coverage
on the shared store. Retain bulk cases, service-level bridge tests and wire fixtures.

The transport represents local admission as `Admission::Local` after checking
the socket peer UID against the server's effective UID. Keep that admission
model; local/remote separation does not require a second per-ticket UID field.

## Remaining implementation and acceptance

Restore terminal consume-on-attempt without changing authentication policy:

| Boundary | Acceptance |
|---|---|
| Shared ticket store | Matching node/user succeeds once; replay fails. Different node, different user and both directions of local/remote mismatch consume the ticket. |
| Authenticated control and bridge | Issue a ticket, open one bridge, refuse replay. An admitted different node or local/remote mismatch consumes the ticket. |
| Both WebSocket endpoints | Reject another Tailscale user before upgrade. That connection cannot present a ticket; different-user redemption is tested at the store. |

Replace ticket timing with `SystemTime`; test expiry at 60 seconds with a clock
advance representing host sleep, expired-ticket sweeping, and unchanged bulk
redemption behavior, including release of abandoned bulk reservations without
another request. Leave unrelated throttles on `Instant` and retain watch resumption.

Keep attachment, retry and recovery rules in their shared sections; both local
and remote bridges can lose the acknowledgement after consuming a ticket.
The remote contract adds only transport differences. Keep it and
[phone access](../docs/server.md#phone-access) marked planned until integration,
then update the contract's revision 4 anchor to the transport's revision 5 section.

`git diff --check` — passed; prose-only edits, no build or test rerun.
