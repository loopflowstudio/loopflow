# LOO-775 — Resume server watches

Working design, 2026-11-08. `subscribe.resume` carries the last applied opaque
`reading.checkpoint`. The token fingerprints the server incarnation, read,
arguments, selection and the shared subscription body identity. A matching
first reading is omitted; unknown/changed tokens get the complete current
reading. Explicit confirmations, unavailable readings and terminal `ended`
keep their semantics. No history, payload cache or retained server watch.
Revision 3 keeps older clients from silently speaking the changed wire shape.

Desktop keeps its existing stream and queued frames while reconnecting, so a
token cannot outrun the consumer's baseline. Pending read confirmations retry;
writes never do. Cancellation ends recovery; refusal and overflow end the stream.

## Delete — do not maintain

No remaining predecessors. Revision 2 fixtures and disconnect-always-ends-watch
behavior are replaced. Keep one reading identity, including timestamp elision;
borrow the body until normalization needs a copy. Desktop keeps only the newest
unanswered confirmation, matching the server's coalescing semantics, rather than
a replay queue with its own overflow failure.

## Remaining

- Graph's later review/QA/gate steps own broader acceptance, sync and delivery.
- Gate reaches `server::`, `lf::commands::subscribe::tests::`, `served_reads`,
  `transport_tests`' server watch/restart cases, and Swift's server/Work observation
  consumers. Protocol revision changes also reach installation readiness.

Shipping docs: `rust/loopflow/src/server/README.md` (“Reconnect a watch”).
Docs-step check (2026-11-08): Desktop open failed: “has no workspace in this
Desktop window; nothing changed.”

Checks (2026-11-08; inherited `LF_*` unset, stdin closed):
- `cargo test -p loopflow --lib server::`: compress 28 passed. Initial implement run: 27 passed, stale checked-in schema failed, then regenerated from the built CLI.
- `cargo test -p loopflow --lib server::schema::tests::the_checked_in_schema_is_what_describe_prints`: passed after regeneration.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'ServerClientTests|ServerProtocolTests'`: app built; 23 tests passed, including dropped-watch recovery and pending confirmation.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'ServerClientTests|ServerProtocolTests|WorkSubscriptionsTests'`: compress build and 23 tests passed; 100 unanswered confirmations resume with only the newest. Last filter matched no tests; consumer suite ran separately below.
- `uv run --no-sync python scripts/swift_test_limit.py python scripts/test_network.py --desktop swift test --skip-build --disable-sandbox --package-path swift --no-parallel --filter WorkObservationTransportTests`: 8 passed.
- `cargo test -p loopflow --lib lf::commands::subscribe::tests::`: 5 passed.
- `cargo test -p loopflow --test served_reads a_client_built_from_the_description_alone_speaks_the_protocol`: passed with `resume: null` in the raw client fixture.
- `cargo fmt --check` and `git diff --check`: passed. Clippy and broader affected suites belong to gate.
