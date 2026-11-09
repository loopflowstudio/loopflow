# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract),
linked from `docs/server.md`. This checkout contains the planned contract only.

Shipping docs: `docs/server.md` and `rust/loopflow/src/server/README.md`.
Read both before implementation; the latter owns the exchange and ticket rules.

The listener, mobile settings and bulk protocol are outside this Task.
Keep the shared attach rules in one place; the remote section adds only its
exchange example, authentication and expiry requirements.
The remote contract references the shared rules once to avoid maintaining
duplicate descriptions of owner selection and ticket rejection.

## Transport dependency — November 9, 2026 UTC

The 20:14 UTC realign check inspected PR #1812 head
`f0c77e31fa569f13748ad29506af56670bcde3b1`. It supplies
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

The 20:49 UTC implementation check found PR #1812 still OPEN without auto-merge and with
`mergedAt: null` (`gh pr view`).
`git fetch origin main` returned `4848816f73ccbacd5f1eb26bff7f54814d7c0c8b`;
`git ls-tree` found neither `mobile.rs` nor `ticket.rs` there.
These inspected changes are not a landed implementation.
The PR head remains the inspected `f0c77e31fa569f13748ad29506af56670bcde3b1`.
`lf task status LOO-718 --json` confirms `feature` at `demo`, with PR #1812
open. The LOO-809 status read returned no output before interruption; its
current Graph position was not reverified.

Implementation and remote acceptance depend on LOO-718's merged transport.
Landing that PR is outside this Task's scope. No product decision is missing.
Graph decision: stop for the external dependency; implementation and publication
remain incomplete until the authenticated transport lands. The existing checkout
and documentation edits remain available for continuation.

Earlier dependency observations are in
`cda7cf5c96f5896800c12d34f8c71126308fbb87:scratch/design.md`.

## Delete — do not maintain

Replace any remaining `server/service.rs::Ticket` and `Tickets` UID-only store
with the landed transport's shared `server/ticket.rs` authority. Do not extend
the predecessor while waiting. Move its exclusive
`a_ticket_opens_one_bridge_for_its_user_within_its_lifetime` test to that shared
store, retaining identity, single-use, expiry and sweep coverage. Keep
service-level bridge tests and wire fixtures for the surviving protocol.

## Remaining implementation

Recheck the landed revision before integrating. Restore terminal consume-on-attempt
in the shared store without changing bulk redemption policy. Replace ticket timing
with `SystemTime` so the 60-second deadline includes host sleep; leave unrelated
throttles on `Instant`. The deletion section owns the store and test cutover.

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

`git diff --check` — clean; `lf context` — within both scratch limits. Prose only;
no build or tests needed.
