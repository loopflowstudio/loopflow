# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout adds the planned contract only.
The listener, mobile settings and bulk protocol are outside this Task.
The existing local store consumes tickets on every attempt and checks the peer
UID, but uses `Instant`; it does not implement remote identity or sleep-aware
expiry.

## Transport dependency — November 11, 2026 UTC

At November 11 13:04 UTC, PR #1812 remained open and unmerged at
`ac2cba2a3b6b28be3b1e3fb97d609221981f126e`. Fetched main remained
`976b53db0c5affa519af66bfe9bf58ee83fba347`, without `server/mobile.rs` or
`server/ticket.rs`. The PR's ticket, service and mobile modules had no diff
from the previously inspected `21d26bceaa2fdac6fc09c3cb3a952cdd86c597ba`.
Both Task status reads again timed out after 20 seconds; live Graph positions
remain unverified. The Graph decision remains stop for the missing LOO-718
transport; implementation and publication cannot proceed on the local-only store.

At November 11 12:19 UTC, PR #1812 was open and unmerged
at `21d26bceaa2fdac6fc09c3cb3a952cdd86c597ba`. Fetched main was
`976b53db0c5affa519af66bfe9bf58ee83fba347`, without `server/mobile.rs` or
`server/ticket.rs`. The PR description still states: “The pull request is
published for review; no Task settlement is requested.” At 12:15 UTC,
`lf task status LOO-718 --json` reported `feature` at `demo`, with PR
settlement not armed. At 12:19 UTC, fresh `lf task status LOO-718 --json`
and `lf task status LOO-809 --json` reads each timed out after 20 seconds
without a status response. Their live Graph positions were not verified.

Inspection found both WebSocket paths authenticate with node/user identity,
but foreign terminal attempts remain unconsumed and ticket expiry uses
`Instant`. Only bulk tickets have direct store coverage; background expiry
already releases abandoned bulk reservations. The ticket store and service
are unchanged from `48bd007c13b7198844af56eaaf10fe3a8aac7db9`.
Mobile admission permits the host's own node with `mobile-same-device`;
ticket identity remains node/user.

Implementation, authenticated acceptance checks and delivery wait for LOO-718
to land; reinspect its shared store and endpoints then. Landing that PR is
outside this Task's scope. Desktop QA status is in `questions.md`.

Earlier observations, compared revisions and the conflicting 19:35 UTC status
reference remain in
`ad8ceb4f4d8948d2b3887c757382713abbbf1141:scratch/design.md`.
This compression keeps the latest dependency report here and prior reports in Git.

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
Keep connection steps and Desktop controls in the client guide; protocol details
and shared exact-owner recovery belong in the server README. Keep authentication
before the two-connection example, with the running-owner prerequisite and
missing-owner error beside it, so neither steps nor retries repeat the rules.

Shipping docs are in `docs/server.md` and `rust/loopflow/src/server/README.md`.
The remote contract distinguishes rejection before WebSocket upgrade from a
ticket redemption attempt; use these in-place docs during implementation.

`git diff --check` — passed; prose-only edits, no build or test rerun.
