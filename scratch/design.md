# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout contains the planned contract only. The listener, mobile settings
and bulk protocol are outside this Task.

## Transport dependency — November 11, 2026 UTC

Inspected PR #1812 at `51bbe6e7232a46858b308e778487762e2332cdab`: both
WebSocket paths authenticate with the planned node/user identity, but the
shared ticket store leaves foreign attempts unconsumed and uses `Instant`.
Only bulk tickets have direct store coverage; terminal consume-on-attempt
coverage is missing. Background expiry already releases abandoned bulk
reservations.
The server tree is unchanged from the previously inspected
`f3ff3041d0718ddc16dad51bc9a384c44ab2fe9a`.

At November 11 00:27 UTC, GitHub reports PR #1812 open and unmerged at
that revision, with no auto-merge. Freshly fetched main remains
`454f768944fb4b4aa4ee572beb1ca5418b18df6d`, without `server/mobile.rs`
or `server/ticket.rs`. LOO-718 remains at `feature`'s `demo` step.

Implementation and delivery wait for LOO-718 to land; reinspect its shared store
and authenticated endpoints then. Landing that PR is outside this Task's scope.
No product decision is missing; Desktop visual inspection remains for QA.

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
details and shared owner recovery, avoiding duplicate contracts. Desktop opening
results are in `questions.md`.

`git diff --check` — passed; prose-only edits, no build or test rerun.
