# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout adds the planned contract only.
The listener, mobile settings and bulk protocol are outside this Task.
The existing local store consumes tickets on every attempt and checks the peer
UID, but uses `Instant`; it does not implement remote identity or sleep-aware
expiry.

## Transport dependency — November 11, 2026 UTC

Inspected PR #1812 at `48bd007c13b7198844af56eaaf10fe3a8aac7db9`: both
WebSocket paths authenticate with the planned node/user identity, but the
shared ticket store leaves foreign attempts unconsumed and uses `Instant`.
Only bulk tickets have direct store coverage; terminal consume-on-attempt
coverage is missing. Background expiry already releases abandoned bulk
reservations.
The ticket store, remote carrier and service are unchanged from
`6ad2a0a45c9f40e6cbc60febf3879fa34953a34d`. Mobile admission now permits
the host's own node when the Machine opts in with `mobile-same-device`;
ticket identity remains node/user.

At November 11 10:20 UTC, GitHub still reports PR #1812 open and unmerged
at that head. Freshly fetched main is
`976b53db0c5affa519af66bfe9bf58ee83fba347`,
still without `server/mobile.rs` or `server/ticket.rs`.
The 09:46 UTC read found main at
`6d8fad3375a81890d00965ba46ea396360f7db2a`, no auto-merge, and LOO-718
at `feature`'s `demo` step with PR settlement not armed.
The 10:20 UTC Task-status refresh produced no result before cancellation;
current Graph state is unknown. The GitHub and fetched-tree reads independently
confirm the transport remains unmerged.

Implementation and delivery wait for LOO-718 to land; reinspect its shared store
and authenticated endpoints then. Landing that PR is outside this Task's scope.
The absent remote transport prevents the authenticated connection acceptance
checks. Desktop QA status is in `questions.md`.

Earlier status reads and inspection references are in
`1325549f96cb65e22e580d1e35095296fd43d5c0:scratch/design.md`, including the
reference to the conflicting PR status at 19:35 UTC.

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
redemption policy. Through authenticated client and bridge connections, test that
matching node/user succeeds once; a different node, different user, or
local/remote mismatch fails and consumes the terminal ticket. Local UID tests
alone cannot satisfy these checks.

Replace ticket timing with `SystemTime`; test expiry at 60 seconds with a clock
advance representing host sleep, expired-ticket sweeping, and unchanged bulk
redemption behavior, including release of abandoned bulk reservations without
another request. Leave unrelated throttles on `Instant` and retain watch resumption.

After integration, update `docs/server.md` (Phone access) and
`rust/loopflow/src/server/README.md` (Remote attach contract): remove their
planned/local-only wording and describe local/remote admission binding.
Keep exact-owner selection and recovery together for both transports; keep
the running-owner prerequisite and missing-owner error beside the remote example.
The phone overview keeps connection steps; the server README owns protocol
details and shared owner recovery. Desktop status and recovery controls live in
the client guide, avoiding duplicate descriptions. The inspected
transport speaks revision 5: update the remote example's `hello`/`welcome`
anchor from revision 4 when integrating it.

`git diff --check` — passed; prose-only edits, no build or test rerun.
