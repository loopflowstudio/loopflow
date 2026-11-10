# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout contains the planned contract only. The listener, mobile settings
and bulk protocol are outside this Task.

## Transport dependency — November 10, 2026 UTC

Inspected PR #1812 at `e0f6d4c19aa5aecd1388248a2d2b6fd5c7fedf29`: both
WebSocket paths authenticate with the planned node/user identity, but the
shared ticket store leaves foreign attempts unconsumed and uses `Instant`.
Terminal consume-on-attempt coverage is missing.

At November 10 20:49 UTC, PR #1812 remains
open and unmerged at `dde39d48f4a6865104ba8911ac9d0afb218e13fa`, still mergeable
with no auto-merge. Freshly fetched main is
`454f768944fb4b4aa4ee572beb1ca5418b18df6d`; `server/mobile.rs` and
`server/ticket.rs` remain absent. The fresh `lf task status LOO-718 --json`
read still places the transport Task at `feature/demo`, with PR #1812 open and
settlement not armed. The newer PR head has not been inspected for code changes.

LOO-718 must land before implementation and delivery; reinspect its shared store
and authenticated endpoints then. Landing that PR is outside this Task's scope;
no product decision is missing.

Earlier dependency observations are in
`811c6b703480f05513897ea264b44b2ecd72aeaf:scratch/design.md`, including references
to older status reads; the conflicting PR status at 19:35 UTC is retained in
`074561f725255dfe5c56d46fed3969eed725b75a:scratch/design.md`.
The detailed transport inspection and 20:14 UTC status are in
`7f2ffc44ddb84377d31bf05bb689d5b3cf0a16a3:scratch/design.md`.

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
These docs own the contract; this plan keeps only remaining integration work.
The phone overview keeps the connection steps; protocol details and shared
owner recovery stay in the server README to avoid repeating them here.
Desktop visual inspection remains for QA; opening results are in `questions.md`.

`git diff --check` — passed; prose-only edits, no build or test rerun.
