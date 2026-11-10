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
leaves foreign attempts unconsumed and expires tickets with `Instant`, contrary to this
Task's consume-on-attempt and host-sleep requirements. It has already removed
the UID-only store. `Admission::Local` follows a same-effective-UID check on
every Unix socket connection. The shared-store tests cover bulk single use,
foreign attempts remaining redeemable, expiry and sweeping; they do not cover
the terminal consume-on-attempt requirement.

At November 10 10:11 UTC, GitHub reports PR #1812 open and unmerged at the same
head, with `mergeable: false`. Freshly fetched main is
`9c3403a885356276c52c345b426dc9d7b4714be7`; both transport files remain absent.
`lf task status LOO-718 --json` still places the transport Task at `feature/demo`,
with PR #1812 open and settlement not armed.

Implementation and remote acceptance depend on LOO-718's authenticated transport
landing, followed by a recheck of its shared store. Landing that separate PR is
outside this Task's scope. No product decision is missing. Publication, merge
and Task completion remain outstanding.

Earlier dependency observations are in
`811c6b703480f05513897ea264b44b2ecd72aeaf:scratch/design.md`, including references
to older status reads.

## Delete — do not maintain

Integrating the inspected transport removes `server/service.rs::Ticket` and
`Tickets`, the current UID-only store. Use its shared `server/ticket.rs`
authority without retaining a second store. Move the identity, consume-on-attempt,
single-use, expiry and sweeping coverage from
`a_ticket_opens_one_bridge_for_its_user_within_its_lifetime` to the shared-store
tests, then delete that predecessor test. Retain bulk cases, service-level bridge
tests and wire fixtures. Until the transport lands, leave the UID-only store
alone: changing it would extend code this Task removes.

## Remaining implementation

Restore terminal consume-on-attempt in the shared store without changing bulk
redemption policy. Replace ticket timing with `SystemTime` so the 60-second
deadline includes host sleep; leave unrelated throttles on `Instant`. Retain the
transport's watch resumption.

After integration, update the docs' planned/local-only wording to match the
implemented endpoint. Read both shipping docs from this checkout:
`docs/server.md` (Phone access) and `rust/loopflow/src/server/README.md`
(Remote attach contract).

## Remaining acceptance

Exercise the remote attach contract through authenticated client and bridge
connections: matching node/user succeeds once; a different node, different user,
or local/remote mismatch fails and consumes the terminal ticket. Check expiry at
60 seconds with a clock advance representing host sleep, expired-ticket sweeping,
and unchanged bulk-transfer redemption behavior. Local UID tests alone cannot
satisfy these checks.

Desktop review and opening results are in `questions.md`.

Keep the remote exchange self-contained; link to the existing owner protocol
instead of repeating it. The UID-only store stays untouched until its replacement
lands, so this compression changes prose only.

`git diff --check` — clean; `lf context` — within both scratch limits. Prose only;
no build or tests needed.
