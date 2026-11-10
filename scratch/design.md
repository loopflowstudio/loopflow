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

At November 10 05:12 UTC, `gh pr view 1812` returned `state: OPEN`,
`mergedAt: null`, `autoMergeRequest: null` and the same head. Freshly fetched main is
`8155d1fcde597154ee9746ad72280bf6687a7da8`; `server/mobile.rs` and
`server/ticket.rs` are still absent. This checkout still uses the UID-only
store with consume-on-attempt and `Instant` timing. Remote implementation and
acceptance remain dependent on LOO-718 landing.

At November 10 05:46 UTC, PR #1812 remains open at the same head, with no
merge time or auto-merge request. Freshly fetched main is
`169485522edd24e3ea9c0946dfea79c881fe546b`; both transport files remain absent.
Implementation and remote acceptance remain blocked on LOO-718 landing.

Implementation and remote acceptance depend on LOO-718's transport, which is
not on main. Landing that separate PR is outside this Task's scope. No product
decision is missing; the contract remains planned.

Earlier dependency observations are in
`fc92556bbcb8c96e984df93c18e2882da29318c3:scratch/design.md` and
`b567998a6b700a12757ee1a613fbf5076b256e30:scratch/design.md`; stalled Task status
reads and revision comparisons are in
`cb1f9faf1d249c8dba15bdd69b6208ca113c98f5:scratch/design.md`.

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

`git diff --check` — clean; `lf context` — within both scratch limits. Prose only;
no build or tests needed.
