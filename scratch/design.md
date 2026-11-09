# LOO-809: remote attach tickets

Reuse the local attach flow and wire shapes documented in
`rust/loopflow/src/server/README.md#remote-attach-contract`, linked from
`docs/server.md`. This checkout contains that planned contract only; tickets
still bind to Unix UID and expire with `Instant`.

Shipping doc paths: `docs/server.md` and `rust/loopflow/src/server/README.md`.
The remote example requires independent bridge authentication and sends `attach`
first, without `hello`.

The listener, mobile settings and bulk protocol are outside this Task.
The remote contract reuses the local owner-selection, binary protocol and control description;
only the exchange example and remote ticket rules need their own section.
Keep consume-on-attempt and rejection rules in the shared attach description,
with the remote contract referring to them.

## Transport dependency — November 9, 2026 UTC

LOO-718's PR #1812 supplies `Admission::Remote { node: String, user: u64 }` through
`server/mobile.rs::permits_peer`: Tailscale WhoIs `Node.StableID` and
`UserProfile.ID`, checked against `Node.User`. Both WebSocket paths authenticate
through `host.permits`.

Two inspected revisions differ:

- Earlier published head `b222504a8ededd0f26dbe01e404b1bcb9f446d14`: 30-second UUID-text
  tickets and the older `ticket` / `admitted` wire flow.
- Clean mobile checkout `682e2492bb791dbc7b3c786a68cdfcce00943138`: shared
  60-second lifetime and 22-character base64url tokens.

Both share `server/ticket.rs` between terminal and bulk transfer, bind tickets to
node and user, leave foreign attempts unconsumed and expire with `Instant`.
The mobile checkout tests redemption after foreign attempts, contrary to this
Task's consume-on-attempt contract.

The November 9 implementation check at 18:54 UTC found PR #1812 OPEN at
`2f89c2558347e63e40a679a0d59adefa1cf2d84b`, without auto-merge (`gh pr view`).
That revision's ticket implementation has not been inspected; the two revisions
above remain historical findings. `git fetch origin main`
returned `237a107261572d08e4ffa3c8baf72b1c164c110e`; `git ls-tree` found neither
transport file. The separate mobile checkout findings above are not a landed
implementation. `lf task status LOO-718 --json` still reports `feature` at `demo`,
with PR #1812 open. Reading `service.rs` confirmed UID-only tickets, `Instant`
expiry and removal before identity validation; no code repair is needed before
the transport integration.

Implementation and remote acceptance depend on LOO-718's merged transport.
Landing that PR is outside this Task's scope. No product decision is missing.

## Delete — do not maintain

When the transport lands, replace any remaining UID-only `service.rs::Ticket` /
`Tickets` with its shared authority; do not refactor the predecessor while waiting.
Move `a_ticket_opens_one_bridge_for_its_user_within_its_lifetime` onto that store,
retaining identity, single-use, expiry and sweep coverage. Keep the service-level
bridge rejection test and wire fixtures: they exercise the surviving protocol.

## Remaining implementation

Reconcile wire and lifetime differences against the landed revision. Retain
consume-on-attempt; replace ticket timing with `SystemTime` so the 60-second
deadline includes host sleep. Leave unrelated throttles on `Instant`.
The terminal ticket contract must not silently change bulk transfer redemption
policy when both use the shared store.

After integration, update the docs' planned/local-only wording to match the
implemented endpoint.

## Remaining acceptance

Exercise the remote attach contract through authenticated client and bridge
connections: matching node/user succeeds once; a different node, different user,
or local/remote mismatch fails and consumes the terminal ticket. Check expiry at
60 seconds with a clock advance representing host sleep, expired-ticket sweeping,
and unchanged bulk-transfer redemption behavior. Local UID tests alone cannot
satisfy these checks.

Retain local attachment and its consume-on-attempt behavior through the shared
store, including the unchanged ticket response and bridge wire fixtures.

Desktop review and opening results are in `questions.md`.

`git diff --check` — clean; `lf context` — within both scratch limits. Prose only;
no build or tests needed.
