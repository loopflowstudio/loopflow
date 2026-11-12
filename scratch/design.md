# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout documents the planned contract; implementation and authenticated
acceptance await LOO-718.
The listener, mobile settings and bulk protocol are outside this Task.

## Transport dependency

At November 12 05:10 UTC, GitHub's PR metadata reported #1812 open and
unmerged, at head `28e2321c8e2ea0ca8d8ecb199f31c5bb1a0e844f`.
`origin/main` was at `d8db655b0edcb080c2fd34057caedcd988c293d0`; this checkout
last merged `649f11db4b460fd33662d44da24ba656ea048b64`. Between heads `4e0ccacd` and `28e2321c`, #1812 changed
`mobile.rs`, `boundary.rs` and added `wake.rs`; `ticket.rs` is unchanged.
`origin/main` does not contain
`server/mobile.rs`, `server/remote.rs` or `server/ticket.rs`.
Reinspect when the shared transport lands; landing its PR is outside this Task.

Inspection of #1812 found
`remote.rs::connection` rejects unadmitted peers before either WebSocket upgrade.
`mobile.rs::permits_peer` binds admission to node/user identity;
`mobile-same-device` permits the host's own node without changing that identity.
`ticket.rs::Tickets::redeem` leaves tickets redeemable after foreign terminal
attempts and uses `Instant`. Its direct tests cover only bulk purposes, including
foreign attempts and background release of abandoned reservations.

The November 12 04:35 UTC `lf task status LOO-718 --json` read reported
`feature` / `demo`, with PR #1812 open and settlement not armed.
Earlier observations and history references are in
`23d1372b07f5850f4f25f877cd1bcde4f82d0f86:scratch/design.md`.
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

Keep redemption, retry and recovery rules in their shared sections: failed
redemption consumes local and remote terminal tickets, and either bridge can
lose its acknowledgement. Bulk tickets retain their existing foreign-attempt behavior.
The remote contract adds only transport differences. Keep it and
[phone access](../docs/server.md#phone-access) marked planned until integration,
then update the contract's revision 4 anchor to the transport's revision 5 section.

`git diff --check` — passed after the November 12 compress (remote contract's duplicate device/account clause dropped); prose-only, no build or test rerun.
