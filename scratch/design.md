# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout adds the planned contract only.
The listener, mobile settings and bulk protocol are outside this Task.
The existing local store consumes tickets on every attempt and checks the peer
UID, but uses `Instant`; it does not implement remote identity or sleep-aware
expiry.

## Transport dependency — November 11, 2026 UTC

At November 11 14:23 UTC, PR #1812 remained open and unmerged at
`ac2cba2a3b6b28be3b1e3fb97d609221981f126e`. Fetched main was
`49037487daf00729516d596f0d946252a7219dce`, still without `server/mobile.rs` or
`server/ticket.rs`. The PR's ticket, service and mobile modules still had no
diff from `21d26bceaa2fdac6fc09c3cb3a952cdd86c597ba`.
The 14:23 status reads found LOO-718 still at `feature` / `demo`;
LOO-809 timed out after 20 seconds, leaving its live Graph position unverified.
At 13:43 the PR had no auto-merge; the current metadata read did not expose that
field. The PR description still
stated: “The pull request is published for review; no Task settlement is requested.”

Inspection found both WebSocket paths authenticate with node/user identity,
but foreign terminal attempts remain unconsumed and ticket expiry uses
`Instant`. Only bulk tickets have direct store coverage; background expiry
already releases abandoned bulk reservations. The ticket store and service
are unchanged from `48bd007c13b7198844af56eaaf10fe3a8aac7db9`.
Mobile admission permits the host's own node with `mobile-same-device`;
ticket identity remains node/user.

Implementation and authenticated acceptance checks still require LOO-718's
unmerged transport. Landing that PR is outside this Task's scope. The shared
store and endpoints need reinspection after it lands. Desktop QA remains
deferred in `questions.md`; no product decision is open.

Earlier observations and compared revisions are in
`313f59abc56e30106f21994fe8e35d4552099571:scratch/design.md`, including the
reference to the older, conflicting 19:35 UTC status report.

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

## Implementation and acceptance

Read the shipping docs in `docs/server.md` and
`rust/loopflow/src/server/README.md` before implementing. The remote example
requires the same authenticated device and account on both connections.

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

After integration, remove planned/local-only wording from `docs/server.md`
and `rust/loopflow/src/server/README.md`; describe local/remote admission binding
and update the remote example's revision 4 anchor to revision 5.
Keep Desktop controls in the client guide and protocol details in the server
README. The remote example keeps authentication and owner prerequisites beside
the connection steps and links shared exact-owner recovery instead of repeating it.

`git diff --check` — passed; prose-only edits, no build or test rerun.
