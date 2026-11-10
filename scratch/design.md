# LOO-802: Typed subscription events

`event {id,seq,event}` accompanies complete readings at revision 4. Existing
merged-PR records, immutable steers and Graph step exits supply occurrences.
The first reading baselines history in its pinned snapshot. Selection cannot
hide events; unavailable membership retains the prior scope. Events share one
connection sequence and bounded delivery reservation, with no replay cursor.
Swift event consumers receive readings and events in one ordered stream.

Implemented: each pull returns the reading with its events; the separate event
drain and shared buffer are gone. Queued frames hold either encoded text or a
typed event, assigned its sequence after cancelled frames are discarded.
Readings and events share Graph-step conversion. No migration or parallel event
log is needed. Reading-only consumers keep checkpoint resumption; Swift event
consumers reconnect with a fresh reading. Sequence gaps end all event consumers
on the connection, while unknown kinds end only the affected consumer.

Remaining: review, publication, affected-suite gate, CI repair if needed,
landing and merged-behavior verification before Task completion. No missing
product decision was found in this pass. Board animations and thread UI changes
are outside this Task; the event payloads and ordered client stream enable them.

Concept review (2026-11-09): no design change or open product choice; advance.
The important distinction remains current state versus a newly observed
occurrence. A reading can already contain the arriving comment: append by
comment ID, while the event can still trigger presentation. Keep delivery
sequence separate from occurrence identity; overlapping subscriptions can carry
the same occurrence. Reconnecting restores state without replaying reactions.
The counterexamples fit the existing contract: a Task leaving a plan still
delivers its final observed ship; a newly included Task does not replay old
ships; a connection-wide gap invalidates every event consumer, while an unknown
kind affects only its subscription. No second event log or replay lifecycle is
needed. Next show a live comment, a ship leaving a plan, and a reconnect with
missed occurrences recovered as state. The review page is
`scratch/concept-review.html`; these UI examples illustrate consumers, not
additional implementation scope. Existing usage docs already teach this model;
no skill instructions need changing.

Upstream check on 2026-11-09: fetched `origin/main` is `7cf2d25b3`, the parent
of this branch's implementation commit `bd76f4cf3`; divergence is 0 behind,
1 ahead. Base PR #1813 and reading-checkpoint resumption (#1865) are included.
Sync before delivery remains necessary if main advances.

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

QA (2026-11-09): `scratch/qa.html` records the real private-server walkthrough
and links its command/socket transcript. Ten scenarios passed: released Session
state, comment payloads, overlapping/selected watches, cancellation, real Graph
exits, disconnect recovery, CLI agreement, plan/dashboard bursts, empty input,
server restart and Swift wire interoperability (some grouped). Swift decoded 16
live readings, 20 steer events and two step events. No typed-event defect found.
Live GitHub ship/plan departure, mounted Desktop, automatic ServerClient recovery
and saturation/fault injection remain unverified by this walkthrough.

Task checkout also warmed primary conversations, leaving terminal owners after
the command Graphs. Stopped every owner/descendant tied to these disposable
checkouts; updated the QA skill and driver to isolate provider homes and clean
them up. The full walkthrough passed again; no private QA lf process remained.

- `cargo build -p loopflow --bin lf`: passed; QA built the branch CLI.
- `swift build --package-path swift --target Loopflow -Xswiftc -gnone`: passed.
- `uv run python scratch/qa_events.py` with the compiled Swift probe: passed;
  foreground servers reaped and private terminal owners stopped. Gate still owns
  the affected suites above.
