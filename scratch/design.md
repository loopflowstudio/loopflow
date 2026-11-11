# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract).
This checkout adds the planned contract only.
The listener, mobile settings and bulk protocol are outside this Task.
The existing local store consumes tickets on every attempt and checks the peer
UID, but uses `Instant`; it does not implement remote identity or sleep-aware
expiry.

## Transport dependency — November 11, 2026 UTC

At 14:27 UTC, PR #1812 remained open and unmerged, with no auto-merge, at
`ac2cba2a3b6b28be3b1e3fb97d609221981f126e`. Fetched main was
`49037487daf00729516d596f0d946252a7219dce`, still without `server/mobile.rs` or
`server/ticket.rs`. LOO-718 remained at `feature` / `demo`. LOO-809's status
read succeeded: `auto`, state `shipping`, step null, reason `running from start`,
no PR and no Wave. The earlier timed-out read is in
`62072a0e69a90602bec564e6d5477b10ca131977:scratch/design.md`.

At 15:04 UTC, `gh pr view 1812` and a fresh `git fetch origin main`
confirmed the same open PR head, no auto-merge and unchanged main, still without
the shared ticket store or remote endpoints. `lf task status LOO-718 --json`
again reported `feature` / `demo`. Implementation remains blocked on that
transport landing; no code or acceptance tests can proceed in this scope.

At that revision, both WebSocket paths authenticate with node/user identity,
but foreign terminal attempts remain unconsumed and expiry uses `Instant`.
Only bulk tickets have direct store coverage; background expiry already releases
abandoned bulk reservations. `mobile-same-device` permits the host's own node
without changing ticket identity.

Implementation and authenticated acceptance require LOO-718's transport.
Landing that PR is outside this Task's scope; reinspect its shared store and
endpoints after it lands. Desktop QA remains deferred in `questions.md`.

The full dependency observations, PR description quote and compared revisions
are in `91f7421d7efe2e5262be8414b393019cfd44b833:scratch/design.md`, which also
links the older, conflicting 19:35 UTC status report. Keep that history in Git;
this plan carries the remaining work.

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

Keep `docs/server.md` and `rust/loopflow/src/server/README.md` clear about what
this build provides: local UID-bound tickets today, the planned remote contract
in its own section. Keep remote identity and sleep-aware expiry there rather
than repeating unimplemented behavior in the local protocol or client guide.
After integration, remove the planned wording and update the remote example's
revision 4 anchor to revision 5 when the transport's protocol section arrives.
Keep Desktop controls in the client guide and protocol details in the server
README. The remote example keeps authentication and owner prerequisites beside
the connection steps and links shared exact-owner recovery instead of repeating it.

`git diff --check` — passed; prose-only edits, no build or test rerun.
