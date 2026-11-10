# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout contains the planned contract only. The listener, mobile settings
and bulk protocol are outside this Task.

## Transport dependency — November 10, 2026 UTC

The inspected PR #1812 head is
`73826eaf005d00e1aeae26dd1cf2a91cf5f72981`. It supplies
`Admission::Remote { node: String, user: u64 }` through
`server/mobile.rs::permits_peer`: Tailscale WhoIs `Node.StableID` and
`UserProfile.ID`, checked against `Node.User`. Both WebSocket paths authenticate
through `host.permits`: `/v1/machine` for control and `/v1/ticket` for redemption.
Terminal redemption answers `bridged {owner_lf_process_id}`, matching this
Task's planned wire contract; bulk redemption answers `admitted`.

That revision shares `server/ticket.rs` between terminal and bulk transfer,
leaves foreign attempts unconsumed and expires tickets with `Instant`, contrary
to this Task's consume-on-attempt and host-sleep requirements.
`Admission::Local` follows a same-effective-UID check on
every Unix socket connection. The shared-store tests cover bulk single use,
foreign attempts remaining redeemable, expiry and sweeping; they do not cover
the terminal consume-on-attempt requirement.

At November 10 20:11 UTC, GitHub reports PR #1812 open and unmerged at
`e0f6d4c19aa5aecd1388248a2d2b6fd5c7fedf29`, now `mergeable: MERGEABLE`,
with no auto-merge. Its inspected `server/ticket.rs` still uses `Instant` and
leaves foreign terminal attempts unconsumed. Freshly fetched main is
`9c3403a885356276c52c345b426dc9d7b4714be7`; both transport files remain absent.
The November 10 20:11 UTC `lf task status LOO-718 --json` read still places the
transport Task at `feature/demo`, with PR #1812 open and settlement not armed.

The checkout still uses the UID-bound `service.rs::Tickets`: redemption removes
the ticket before checking identity, but expiry uses `Instant`. Remote identity
binding and host-sleep expiry remain unimplemented; the docs label the remote
contract as planned.

LOO-718 must land before implementation and delivery; reinspect its shared store
and authenticated endpoints then. Landing that PR is outside this Task's scope;
no product decision is missing.

Earlier dependency observations are in
`811c6b703480f05513897ea264b44b2ecd72aeaf:scratch/design.md`, including references
to older status reads; the conflicting PR status at 19:35 UTC is retained in
`074561f725255dfe5c56d46fed3969eed725b75a:scratch/design.md`.

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
redemption behavior. Leave unrelated throttles on `Instant` and retain watch
resumption.

After integration, update `docs/server.md` (Phone access) and
`rust/loopflow/src/server/README.md` (Remote attach contract): remove their
planned/local-only wording and replace the general attach section's UID-only
description with the local/remote admission binding. Keep same-device ticket
use, exact-owner recovery and attachment without provider launch.

Keep the exact-owner parameter, mismatch and recovery rules together in the
shared recovery section so local and remote callers use one contract.
Keep the missing-owner error beside the remote example's owner prerequisite,
so attachment requirements and their failure appear together.
The shipping docs are `docs/server.md` and `rust/loopflow/src/server/README.md`;
the remote example links the handshake and states the running-owner prerequisite
and missing-owner result. Reuse these docs during implementation.
Desktop visual inspection remains for display-capable QA; opening results are
in `questions.md`.

`git diff --check` — passed; prose-only edits, no build or test rerun.
