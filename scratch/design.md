# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract),
linked from `docs/server.md`. This checkout contains the planned contract only.

Shipping docs: `docs/server.md` links to the exchange in
`rust/loopflow/src/server/README.md`, including identity mismatch, consumption
and retry behavior. Read both before implementation.

The listener, mobile settings and bulk protocol are outside this Task.
Keep the contract marked planned until implemented.

## Transport dependency — November 9, 2026 UTC

The 22:46 UTC realign check inspected PR #1812 head
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

The 22:04 UTC `lf task status LOO-718 --json` placed `feature` at `demo`, with
PR #1812 open and settlement not armed. Subsequent status reads did not respond
and were terminated; this is the last successful Graph reading.

At 23:19 UTC, `gh pr view 1812` returned `OPEN`, `mergedAt: null` and
`autoMergeRequest: null` at that same head. Compared with the previously inspected
`baf3989a02a5fa6be666ae94d11448c664be2380`, `mobile.rs` and `ticket.rs`
are unchanged; `service.rs` incorporates watch resumption. Retain that work
when integrating. `git fetch origin main` resolved to
`28b0af568d120b56113c0dfb0eff9439d0c014d5`, with neither `mobile.rs` nor
`ticket.rs` in its tree.

Implementation and remote acceptance depend on LOO-718's merged transport.
Landing that PR is outside this Task's scope. No product decision is missing.

Earlier dependency observations are in
`839c3609e663efe781e67e3f6ed7fab598ab038b:scratch/design.md`.

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
throttles on `Instant`.

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
