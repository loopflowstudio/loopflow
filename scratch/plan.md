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
behavior are replaced. Keep one reading identity, including timestamp elision.
Its normalization copy avoids a second custom serializer. Desktop's initial
connection and recovery share `start()` so opening, cancellation checks and
pending-confirmation replay have one path. Desktop keeps only the newest
unanswered confirmation, matching the server's coalescing semantics, rather than
a replay queue with its own overflow failure.

## Remaining

- Graph's later review/QA/gate steps own broader acceptance, sync and delivery.
- Gate reaches `server::`, `lf::commands::subscribe::tests::`, `served_reads`,
  `transport_tests`' server watch/restart cases, and Swift's server/Work observation
  consumers. Protocol revision changes also reach installation readiness.

Shipping docs: `rust/loopflow/src/server/README.md` (“Reconnect a watch”, including
the resumed wire exchange) and `docs/server.md` (“Keep editing through a
connection loss”, including Work updates). Implementation and review should read
both from this checkout; `docs/dev/server.md` retains the revision-3 reference.
Docs-step check (2026-11-08): Desktop open failed: “has no workspace in this
Desktop window; nothing changed.”
Docs-step check (2026-11-09): `lf open --host desktop --file rust/loopflow/src/server/README.md --file docs/server.md` opened both docs; `git diff --check` passed. Prose-only edits; no build needed.
Implementation check (2026-11-09, inherited `LF_*` unset, stdin closed): `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'ServerClientTests/disconnectedWatch'` built CLI/Desktop and passed both recovery tests, including recovery with the baseline still queued; no production change needed.
Compress check (2026-11-09, inherited `LF_*` unset, stdin closed): `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter ServerClientTests` built CLI/Desktop; 9 tests passed. `git diff --check` passed. Gate owns broader suites.

Checks (2026-11-08; inherited `LF_*` unset, stdin closed):
- `cargo test -p loopflow --lib server::`: compress 28 passed. Initial implement run: 27 passed, stale checked-in schema failed, then regenerated from the built CLI.
- `cargo test -p loopflow --lib server::schema::tests::the_checked_in_schema_is_what_describe_prints`: passed after regeneration.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'ServerClientTests|ServerProtocolTests'`: app built; 23 tests passed, including dropped-watch recovery and pending confirmation.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'ServerClientTests|ServerProtocolTests|WorkSubscriptionsTests'`: compress build and 23 tests passed; 100 unanswered confirmations resume with only the newest. Last filter matched no tests; consumer suite ran separately below.
- `uv run --no-sync python scripts/swift_test_limit.py python scripts/test_network.py --desktop swift test --skip-build --disable-sandbox --package-path swift --no-parallel --filter WorkObservationTransportTests`: 8 passed.
- `cargo test -p loopflow --lib lf::commands::subscribe::tests::`: 5 passed.
- `cargo test -p loopflow --test served_reads a_client_built_from_the_description_alone_speaks_the_protocol`: passed with `resume: null` in the raw client fixture.
- `cargo fmt --check` and `git diff --check`: passed. Clippy and broader affected suites belong to gate.
