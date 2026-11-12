# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract)
on the shared transport from #1812 (merged into this branch at `3f3153d95c`).
The listener, mobile settings and bulk protocol are outside this Task.

## Status

Built: `ticket.rs` expires by `SystemTime`, any attempt spends a terminal
ticket, a bulk ticket survives another device's attempt. Tests cover the store
(5), remote `/v1/ticket` (replay, another node, a local ticket, unadmitted
peers refused before upgrade) and the local service (replay, a phone's
ticket). README contract and `docs/server.md` phone access are no longer
marked planned.

Checks Nov 12 after compress: `cargo test -p loopflow --lib` for
`server::ticket::`, `server::remote::tests::` and service tests passed
(ticket 5, remote 4, service 24); `documented_commands::` passed; clippy clean.

Remaining: gate runs the affected suites; QA inspects the docs visually.
