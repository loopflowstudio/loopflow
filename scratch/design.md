# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract),
linked from `docs/server.md`. This checkout contains the planned contract only.

The listener, mobile settings and bulk protocol are outside this Task.
Keep the contract marked planned until implemented.

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
leaves foreign attempts unconsumed and expires tickets with `Instant`, contrary to this
Task's consume-on-attempt and host-sleep requirements. It has already removed
the UID-only store. `Admission::Local` follows a same-effective-UID check on
every Unix socket connection. The shared-store tests cover bulk single use,
foreign attempts remaining redeemable, expiry and sweeping; they do not cover
the terminal consume-on-attempt requirement.

At November 10 03:57 UTC, `gh pr view 1812` returned `state: OPEN`,
`mergedAt: null`, `autoMergeRequest: null` and the same head. A fresh fetch kept main at
`6f7a6c9caf7f68554445df008593d71baf95cfbe`; `git ls-tree` still found neither
`server/mobile.rs` nor `server/ticket.rs`.

At November 10 04:31 UTC, the same PR query still returned `state: OPEN`,
`mergedAt: null`, `autoMergeRequest: null` and the same head. Freshly fetched
main advanced to `c7094eb9736a8c66a789558019d6cc9b7b476b84`; neither transport
file exists there. The dependency remains unresolved.

Implementation and remote acceptance depend on LOO-718's transport, which is
not on main. Landing that separate PR is outside this Task's scope. No product
decision is missing; the contract remains planned.

Earlier observations, stalled Task status reads and revision comparisons are
recorded in `cb1f9faf1d249c8dba15bdd69b6208ca113c98f5:scratch/design.md`.

## Delete — do not maintain

Integrating the inspected transport removes `server/service.rs::Ticket` and
`Tickets`, the current UID-only store. Use its shared `server/ticket.rs`
authority without retaining a second store. Carry forward the behavior covered
by `a_ticket_opens_one_bridge_for_its_user_within_its_lifetime`: identity,
consume-on-attempt, single use, expiry and sweeping. Add missing terminal cases
to the shared-store tests; retain its bulk cases, service-level bridge tests and
wire fixtures for the surviving protocol.

## Remaining implementation

Recheck the landed revision before integrating. Restore terminal consume-on-attempt
in the shared store without changing bulk redemption policy. Replace ticket timing
with `SystemTime` so the 60-second deadline includes host sleep; leave unrelated
throttles on `Instant`. Retain the transport's watch resumption.

After integration, update the docs' planned/local-only wording to match the
implemented endpoint.

## Remaining acceptance

Exercise the remote attach contract through authenticated client and bridge
connections: matching node/user succeeds once; a different node, different user,
or local/remote mismatch fails and consumes the terminal ticket. Check expiry at
60 seconds with a clock advance representing host sleep, expired-ticket sweeping,
and unchanged bulk-transfer redemption behavior. Local UID tests alone cannot
satisfy these checks.

Desktop review and opening results are in `questions.md`.

Compression removes the repeated client-slot limit from attachment docs (the
local protocol introduction owns it) and condenses remote retry instructions.

`git diff --check` — clean; `lf context` — within both scratch limits. Prose only;
no build or tests needed.
