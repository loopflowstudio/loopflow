# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout adds the planned contract only.
The listener, mobile settings and bulk protocol are outside this Task.
The existing local store consumes tickets on every attempt and checks the peer
UID, but uses `Instant`; it does not implement remote identity or sleep-aware
expiry.

## Transport dependency — November 11, 2026 UTC

At 18:01 UTC, GitHub reported PR #1812 open and unmerged at
`bfd4244858864279ca13450c9f5f102c5554643c`. Fetched main was
`49037487daf00729516d596f0d946252a7219dce`, still without `server/mobile.rs` or
`server/ticket.rs`. At 17:27 UTC, `lf task status LOO-718 --json` still reported
`feature` / `demo`. Implementation and authenticated acceptance require that
landing. Landing PR #1812 is outside this Task's scope. Reinspect its shared
store and endpoints after it lands; the remaining acceptance below still applies.

The 18:01 UTC `lf task status LOO-809 --json` refresh returned no output before
cancellation; the queued LOO-718 refresh did not run. Its Graph status above
remains the 17:27 observation. GitHub and fetched main confirm the dependency.

At the current PR head `bfd4244858864279ca13450c9f5f102c5554643c`,
both WebSocket paths authenticate with node/user identity,
but foreign terminal attempts remain unconsumed and expiry uses `Instant`.
Only bulk tickets have direct store coverage; background expiry already releases
abandoned bulk reservations. `mobile-same-device` permits the host's own node
without changing ticket identity.
The ticket store and mobile admission code are unchanged from the previously
inspected `7e779e650b72ccb806888713c4e3d8756445fb82`; the implementation and
acceptance below still apply.

Earlier observations and references to the PR description quote and conflicting
19:35 UTC status report remain in
`abd082d510a6edba67139c2658a5fa4dca5ec7b2:scratch/design.md`.
Desktop QA remains deferred in `questions.md`.

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

Read the shipping docs in this checkout before implementation:
`docs/server.md` and `rust/loopflow/src/server/README.md`.

Restore terminal consume-on-attempt in the shared store without changing bulk
redemption policy. Test matching node/user success once, different node,
different user, and both local/remote mismatches in the shared store; every
terminal redemption attempt consumes the ticket, including mismatches.

Through authenticated control and bridge connections, test ticket issuance,
one successful bridge, replay refusal, and consumption after an admitted
different node or local/remote mismatch. The transport admits only the host
owner's Tailscale user: another user's connection is rejected before WebSocket
upgrade and cannot present a ticket. Test that rejection on both endpoints
separately from different-user redemption in the shared store. Local UID tests
alone cannot satisfy remote acceptance; authentication policy stays unchanged.

Replace ticket timing with `SystemTime`; test expiry at 60 seconds with a clock
advance representing host sleep, expired-ticket sweeping, and unchanged bulk
redemption behavior, including release of abandoned bulk reservations without
another request. Leave unrelated throttles on `Instant` and retain watch resumption.

Keep the planned remote contract in `rust/loopflow/src/server/README.md` separate
from today's local UID-bound protocol. `docs/server.md` links its authentication
requirements and connection steps so they have one source.
After integration, remove the planned wording and update the remote example's
revision 4 anchor to revision 5 when the transport's protocol section arrives.
Keep Desktop controls in the client guide and protocol details in the server
README; the remote example links shared exact-owner recovery.

`git diff --check` — passed; prose-only edits, no build or test rerun.
