# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout documents the planned contract; implementation and authenticated
acceptance await LOO-718.
The listener, mobile settings and bulk protocol are outside this Task.

The current docs describe [phone access](../docs/server.md#phone-access) as
planned and keep local attachment and retry rules in the
[shared attachment contract](../rust/loopflow/src/server/README.md#terminal-attachment).

## Transport dependency

At November 12 02:37 UTC, GitHub's PR metadata still reported #1812 open and
unmerged, at head `4e0ccacdb3cb74212e0bdaacd1fc2ccf786cec43`.
Fetching `origin/main` returned `baa09168df59ae063d737a31f0e36b07e15c91d9`,
without `server/mobile.rs`, `server/remote.rs` or `server/ticket.rs`.
Implementation remains blocked on the shared transport landing; landing its
PR is outside this Task. The earlier 02:01 UTC read reported no auto-merge
request and main at `d382d0b72e4b7c41773e4c4070322387f8b4ce65`.

Inspection of that head found
`remote.rs::connection` rejects unadmitted peers before either WebSocket upgrade.
`mobile.rs::permits_peer` binds admission to node/user identity;
`mobile-same-device` permits the host's own node without changing that identity.
`ticket.rs::Tickets::redeem` leaves tickets redeemable after foreign terminal
attempts and uses `Instant`. Its direct tests cover only bulk purposes, including
foreign attempts and background release of abandoned reservations. The
November 12 02:01 UTC reinspection found the same admission and ticket behavior.

The November 12 02:01 UTC `lf task status LOO-718 --json` inspection reported
`feature` / `demo`, with PR #1812 open and settlement not armed. LOO-809's
status read timed out after 25 seconds; its Graph position remains unverified.
Earlier observations and history references are in
`9e2a9aeaa1a0951a806e3a92ccd914c84ffd303f:scratch/design.md`.
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

Keep attachment and recovery rules in their shared sections; the remote
contract adds only transport differences. Mark it planned until integration,
then update its revision 4 anchor to the transport's revision 5 section.
Keep phone prerequisites in `docs/server.md` consistent.
Ticket retry guidance belongs in the shared attachment contract because local
and remote bridges can both lose the acknowledgement after consuming a ticket.

`git diff --check` — passed; prose-only edits, no build or test rerun.
