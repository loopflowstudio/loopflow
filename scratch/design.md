# LOO-809: remote attach tickets

Reuse the local attach flow and wire shapes. The planned identity, lifetime and
rejection contract lives in
`rust/loopflow/src/server/README.md#remote-attach-contract`, linked from
`docs/server.md`; keep client behavior there instead of repeating it here.

Implementation and publication wait for LOO-718's remote admission. Its listener,
mobile settings and bulk protocol are outside this Task. No product decision is
missing.

## Transport dependency — November 8, 2026

LOO-718's PR #1812 supplies `Admission::Remote { node: String, user: u64 }` through
`server/mobile.rs::permits_peer`: Tailscale WhoIs `Node.StableID` and
`UserProfile.ID`, checked against `Node.User`. Both WebSocket paths authenticate
through `host.permits`.

Two inspected revisions differ:

- Published head `b222504a8ededd0f26dbe01e404b1bcb9f446d14`: 30-second UUID-text
  tickets and the older `ticket` / `admitted` wire flow.
- Clean mobile checkout `682e2492bb791dbc7b3c786a68cdfcce00943138`: shared
  60-second lifetime and 22-character base64url tokens.

Both share `server/ticket.rs` between terminal and bulk transfer, bind tickets to
node and user, leave foreign attempts unconsumed and expire with `Instant`.
The mobile checkout tests redemption after foreign attempts, contrary to this
Task's consume-on-attempt contract.

`lf task status LOO-718 --json` still reported the interactive
`feature` demo step; `gh pr view 1812 --json state,mergedAt,headRefOid,autoMergeRequest`
again reported OPEN, no merge or auto-merge, at the published head above during
the November 8 17:26 UTC implement retry. `git fetch origin main` advanced main
to `dad2621c1528ab59c031ed79c417e02f7e55264b`; `git ls-tree` still found neither
`server/mobile.rs` nor `server/ticket.rs`. The transport dependency remains;
no implementation or publication can proceed within this Task's scope.

## Delete — do not maintain

When the transport lands, replace any remaining UID-only `service.rs::Ticket` /
`Tickets` with its shared authority. Move local identity isolation and rejection/
lifetime coverage onto that store; do not refactor the predecessor while waiting.
Reconcile wire and lifetime differences against the landed revision. Retain
consume-on-attempt; replace ticket timing with `SystemTime` so the 60-second
deadline includes host sleep. Leave unrelated throttles on `Instant`.

## Remaining acceptance

All remote acceptance remains for implementation, gate and QA: authenticated
client/bridge issuance and redemption; same identity succeeds once; changed node
or user, local/remote crossover, unknown, spent and expired tickets fail and
close; a foreign attempt consumes the ticket. Verify expiry at 60 seconds,
including sleep, and unchanged owner control. Reconcile PR #1812's tests while
keeping bulk transfer behavior intact.

`git diff --check` — clean on the November 8 implement retry; prose-only edits,
no build or tests required. `lf context` — scratch within both limits.
