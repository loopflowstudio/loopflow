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

Gate Nov 12 at `53d2b62ee7` (runner stopped at its uv-cache `du` deadline
with 124.7 GiB free; plan run by hand): architecture, test inventory, fmt pass;
nextest `server::ticket|remote|service|schema::tests` + `documented_commands::`
44/44; clippy `--all-targets -D warnings` clean; website 73 passed; iOS
`test_ios.py` succeeded.

Remaining: QA inspects the docs visually.

## Concept review (Nov 12)

One concept, a ticket: a single-use, 60 s, admission-bound pass for a second
connection. The remote form adds no concept; `Admission::Remote {node, user}`
is the binding the brief asked for, and both endpoints admit before upgrade.

Spending differs by purpose, for a reason the docs don't yet give: a terminal
ticket costs one `session.attach` to replace, so a ticket seen from the wrong
admission is revoked; a bulk ticket holds a payload whose place in its request
ID's order can't be re-requested, so another device's attempt leaves it alone.
Since admission is authenticated, a foreign party can never redeem either kind;
the rule only decides who can destroy one.

Built from this review: the README's Bulk bullet and Terminal attachment state
one rule (revoke when replacing is a re-request); `Tickets::issue` no longer
sweeps, and the expiry test checks only that an expired ticket is refused
before the sweep.
