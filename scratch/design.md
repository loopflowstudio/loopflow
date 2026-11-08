# LOO-809: remote attach tickets

Bind single-use tickets to the remote transport's authenticated identity for
60 seconds, including host sleep. The planned contract lives in
`rust/loopflow/src/server/README.md#remote-attach-contract`, linked from
`docs/server.md`. Reuse the local attach flow and wire shapes.

Implementation and publication wait for LOO-718's remote admission. Importing
its listener, mobile settings and bulk protocol exceeds this Task's scope.

## Transport dependency — November 8, 2026

LOO-718 supplies the identity in open PR #1812. Its `server/mobile.rs::permits_peer`
returns `Admission::Remote { node: String, user: u64 }` from Tailscale WhoIs
`Node.StableID` and `UserProfile.ID`, checked against `Node.User`. Both WebSocket
paths authenticate through `host.permits`. Login text is not the ticket key.

Two revisions were inspected during realign; they are not interchangeable:

- Published PR head `b222504a8ededd0f26dbe01e404b1bcb9f446d14` uses 30-second
  UUID-text tickets and the older `ticket` / `admitted` wire flow.
- The clean `/Users/jack/src/loopflow.mobile` checkout at
  `682e2492bb791dbc7b3c786a68cdfcce00943138` uses the shared 60-second lifetime
  and 22-character base64url tokens.

Both revisions share `server/ticket.rs` between terminal and bulk transfer,
bind tickets to node and user, leave foreign attempts unconsumed and expire with
`Instant`. The inspected mobile checkout tests redemption after foreign attempts.
LOO-809 requires consume-on-attempt and expiration through host sleep.

Realign and implement checks: `lf task status LOO-718 --json` reports the interactive
`feature` demo step; `gh pr view 1812 --json state,mergedAt,headRefOid,autoMergeRequest`
reports OPEN, no merge and no auto-merge, at the published head above.
`git fetch origin main` left main at `95c17782a2dfd782a707f2a9beac6fe8cbdf9be3`,
without `server/mobile.rs` or `server/ticket.rs`; this checkout was at
`9b9672695e1289ce3fb59cde8fb3c824c8048fa6` with the planned docs.

## Delete — do not maintain

When the transport lands, replace the UID-only `service.rs::Ticket` / `Tickets`
with the transport's shared ticket authority, if that replacement has not already
landed. Move local identity isolation and rejection/lifetime coverage onto the
shared store; do not refactor the predecessor while waiting. Reconcile wire and
lifetime differences against the revision that lands.

## Remaining acceptance

Exercise issuance and redemption over authenticated client/bridge connections:
same identity succeeds once; changed node or user, local/remote crossover,
unknown, spent and expired tickets fail and close; a foreign attempt consumes
the ticket. Verify expiry at 60 seconds and unchanged owner control semantics.
Reconcile PR #1812's tests and keep bulk transfer behavior intact. All acceptance
remains outstanding for implementation, gate and QA; no product decision is
missing.

`git diff --check` — clean during implement and compress; prose-only changes,
no build or tests required.
