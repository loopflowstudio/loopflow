# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract)
on the shared transport. The listener, mobile settings and bulk protocol are
outside this Task.

## Transport dependency

#1812 (LOO-718's transport) merged November 12 15:54 UTC; this branch merged
`origin/main` `3f3153d95c`, which contains `server/{mobile,remote,ticket}.rs`.

## Delete — do not maintain

`server/service.rs::{Ticket, Tickets}` is gone on main, replaced by
`server/ticket.rs`; nothing of the UID-only store remains. Local admission is
`Admission::Local` (peer UID matched the server's); no per-ticket UID field.

## Status

Implemented Nov 12:

- `ticket.rs`: `SystemTime` expiry; any attempt spends a terminal ticket; bulk
  tickets survive another device's attempt. `issue_at`/`redeem_at` take `now`.
- Store tests: one bridge per ticket, foreign node/user/local↔remote spend,
  60 s with a sleep-sized clock jump, sweep, bulk foreign attempt, abandoned
  download release.
- Remote `/v1/ticket`: bridges one owner, refuses and spends replay, another
  node and a local ticket; unadmitted peers refused before upgrade on both paths.
- Local service: replay and a phone's ticket refused and spent.
- README contract and `docs/server.md` phone access no longer marked planned.

Checks Nov 12: `cargo test -p loopflow --lib` (server::ticket::, server::remote::tests::,
two service ticket tests) 13 passed; `documented_commands::` passed; clippy clean.
Gate runs the affected suites.
