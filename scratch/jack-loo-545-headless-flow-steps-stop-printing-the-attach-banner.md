# LOO-545: attach banner only for people who can type

Status: implemented 2026-10-20.

TaskInput registers its controls hint with the transport owner (`Request::InputHint`,
held by an open connection) instead of printing it. After a controlling attach or a
registration, the owner sends `Response::InputHint` to the one controlling viewer;
observers and the pipe tail never contain it. Without an owner, the hint prints only
when stdin is a terminal. `/detach` now keeps the execution's own control connections
(input hint, native client) and drops only attached viewers.

Check (after compress): `cargo test -p loopflow --test transport_tests` 15/15 pass; clippy clean.
Remaining: installed headless `lf task run` demo (no banner in run output) belongs to demo/review.
