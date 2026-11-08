# LOO-809: remote attach tickets

The Task brief requires binding tickets to the identity authenticated by the
remote transport, with one use and a 60-second lifetime. Remote admission is
outside the local revision-2 protocol in this checkout.

## Contract

- `docs/server.md`: phone attachment requirements.
- `rust/loopflow/src/server/README.md#remote-attach-contract`: client contract,
  identity binding, ticket rejection and recovery.

These docs describe intended behavior; remote attachment is not implemented in
this checkout. The remote contract reuses the documented local attach flow and
adds identity binding and expiry through host sleep; keep wire shapes unchanged.

## Transport dependency — November 8, 2026

LOO-718 supplies the identity in open PR #1812, checkout `/Users/jack/src/loopflow.mobile`,
commit `682e2492bb791dbc7b3c786a68cdfcce00943138`. Its `server/mobile.rs::permits_peer`
returns `Admission::Remote { node: String, user: u64 }` from Tailscale WhoIs
`Node.StableID` and `UserProfile.ID`, checked against `Node.User`. Both WebSocket
paths authenticate through `host.permits`. Login text is not the ticket key.

That branch already implements terminal tickets in `server/ticket.rs`, alongside
bulk transfer. Its foreign redemption leaves the ticket usable, unlike this
Task's documented consume-on-attempt behavior. It uses `Instant` for expiration;
the 60-second lifetime must also elapse while the host sleeps.

Implementation depends on that remote admission becoming available in this
checkout. Importing its listener, mobile settings and bulk protocol would exceed
LOO-809. No speculative remote enum or second ticket store is added here.

## Delete — do not maintain

When the transport lands, replace the UID-only `service.rs::Ticket` / `Tickets`
with the transport's shared ticket authority, if that replacement has not already
landed. Retain local identity isolation and move its rejection/lifetime coverage
onto the surviving store. Do not maintain parallel local and remote stores.

## Remaining acceptance

Exercise issuance and redemption over authenticated client/bridge connections:
same identity succeeds once; changed node or user, local/remote crossover,
unknown, spent and expired tickets fail and close; a foreign attempt consumes
the ticket. Verify expiry at 60 seconds and unchanged owner control semantics.
Reconcile PR #1812's existing tests on the surviving implementation.

November 8 compress check: `git fetch origin main` / `gh pr view 1812 --json state,mergedAt,headRefOid,url` — main still equals HEAD at `95c17782a2dfd782a707f2a9beac6fe8cbdf9be3`; PR #1812 is open, now at `b222504a8ededd0f26dbe01e404b1bcb9f446d14`. The implementation observations above refer to the earlier pinned commit.

`git diff --check` — clean. Prose-only compression; no build or test required.
