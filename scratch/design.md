# LOO-802: Typed subscription events

`event {id,seq,event}` accompanies complete readings at revision 4. Existing
merged-PR records, immutable steers and Graph step exits supply occurrences.
The first reading baselines history in its pinned snapshot. Selection cannot
hide events; unavailable membership retains the prior scope. Events share one
connection sequence and bounded delivery reservation, with no replay cursor.
Swift event consumers receive readings and events in one ordered stream.

Delete — do not maintain: the separate event drain and shared event buffer;
return each reading with its events so they cannot be pulled apart. Queued
frames hold either encoded text or a typed event awaiting its sequence.
Readings and events share Graph-step conversion. No replaced capability,
migration or parallel event log; reading-only consumers keep checkpoint resumption.

Remaining: the active auto Graph continues with sync, review, QA,
publication, gate and landing. Base PR #1813 has merged; sync before delivery.
Gate owns affected suites:
`test(server::) | test(lf::commands::subscribe::) | test(store::sqlite::task_comments::) | test(dto_fixtures::) | binary(served_reads)`,
plus Swift, website/architecture and clippy through the affected-suite runner.

Checks:
- `cargo test -p loopflow --lib -- server::events:: server::outbox:: server::service::tests::typed_events lf::commands::subscribe::`: 16 passed after compression; build emitted macOS `__eh_frame` size warning. Gate runs affected suites after sync.
- `cargo test -p loopflow --lib server::`: 40 passed during compression, before the final payload buffer-drop adjustment.
- `cargo test -p loopflow --lib server::events::`: 7 passed.
- `cargo test -p loopflow --lib server::service::tests::typed_events_share_connection_order_survive_selection_and_stop_on_cancel`: passed.
- Built library test binary, `server::wire::tests::`: 2 passed.
- Built library test binary, `server::schema::tests::`: 7 passed.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'ServerProtocolTests|ServerClientTests.*(eventWatchReconnects|eventRecovery|disconnectedWatchResumes)'`: Rust/Desktop build and 20 Swift tests passed.

Earlier docs preview: `lf open --host desktop --file rust/loopflow/src/server/README.md --file docs/dev/server.md`
returned “Error: task_82f1213c2bdf4537a349942b6f1a7939 has no workspace in this Desktop window; nothing changed.”
