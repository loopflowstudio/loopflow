# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout contains the planned contract only. The listener, mobile settings
and bulk protocol are outside this Task.

## Transport dependency — November 10, 2026 UTC

Inspected PR #1812 at `2b52aef839da5fccdc5bb9769b3f95e6ea1d2a52`: both
WebSocket paths authenticate with the planned node/user identity, but the
shared ticket store leaves foreign attempts unconsumed and uses `Instant`.
Only bulk tickets have direct store coverage; terminal consume-on-attempt
coverage is missing. Background expiry already releases abandoned bulk
reservations.
The server tree is unchanged from the previously inspected
`23b48c659d574d32c0a2356deaec37a9be43cb22`; the current shared store still
checks admission before removing a ticket and uses `Instant` for all expiry.

At November 10 22:38 UTC, GitHub reports PR #1812 open and unmerged at
`f7f6a50bec1afca7d9dda456c998d82c23aa71ad`, with no auto-merge. This newer
revision has not been inspected; the code findings above describe the earlier
revision only. Freshly fetched
main remains `454f768944fb4b4aa4ee572beb1ca5418b18df6d`, without `server/mobile.rs`
or `server/ticket.rs`. LOO-718 remains at `feature`'s `demo` step.
Implementation remains blocked on LOO-718 landing, not on a product decision
or Desktop visual inspection.

LOO-718 must land before implementation and delivery; reinspect its shared store
and authenticated endpoints then. Landing that PR is outside this Task's scope;
no product decision is missing.

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
details and shared owner recovery, avoiding duplicate contracts.
Desktop visual inspection remains for QA; opening results are in `questions.md`.

`git diff --check` — passed; prose-only edits, no build or test rerun.
