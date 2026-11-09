# LOO-775 — Resume server watches

Reconciled 2026-11-09 after upstream sync. `subscribe.resume` carries the last applied opaque
`reading.checkpoint`. The token fingerprints the server incarnation, read,
arguments, selection and the shared subscription body identity. A matching
first reading is omitted; unknown/changed tokens get the complete current
reading. Explicit confirmations, unavailable readings and terminal `ended`
keep their semantics. No history, payload cache or retained server watch.
Revision 3 keeps older clients from silently speaking the changed wire shape.

Desktop keeps its existing stream and queued frames while reconnecting, so a
token cannot outrun the consumer's baseline. Pending read confirmations retry;
writes never do. Cancellation ends recovery; refusal and overflow end the stream.

## Concept review — 2026-11-09

No design change or open product choice. A Task list remains one observation
across a socket loss. Its reading and token stay together, including readings
still queued for the consumer; a new observation starts fresh. The server
recreates a watch without persisting its lifetime. Numeric confirmations still
ask for a current reading; opaque tokens only omit an unchanged baseline.
The shipping docs already describe this interaction; no skill changes are needed.

Keep these counterexamples in QA: Work changes and returns to its original state
during an outage (current state, no event replay); the server restarts (complete
replacement); a terminal reading arrives just before losing `ended` (resume must
still end); cancellation during recovery (no reopened watch). The queued-baseline
case already has the recorded socket-loss result below. These checks extend
verification, not the product model. QA now records retained Desktop Work-model
recovery through server replacement; mounted-window behavior remains unseen.
Review page: `scratch/concept-review.html`. Concept-review check: `git diff --check`
passed; prose-only changes, no builds rerun. QA and gate retain the checks above.

## Implemented

Revision 3 fixtures replace revision 2; baseline-bearing watches recover across
disconnects. One reading identity includes timestamp elision, and its normalization
copy avoids a second custom serializer. Desktop's initial
connection and recovery share `start()` so opening, cancellation checks and
pending-confirmation replay have one path. Desktop keeps only the newest
unanswered confirmation, matching the server's coalescing semantics, rather than
a replay queue with its own overflow failure.

## Remaining

- Gate and delivery remain. The branch includes upstream
  operation-name alignment (#1855), Git watch freshness (#1860), exact Session
  opening/recovery (#1863), and pre-disconnect keystroke handling (#1839).
  These remain separate from subscription recovery; sync again before delivery.
- QA exercised real server reconnects, changed and unchanged readings, server
  replacement, and terminal completion. A headless production Work-model check
  retained the last reading during an outage, applied offline edits after server
  replacement, received subsequent edits and cancelled during another outage.
  No mounted Desktop or phone recovery result is claimed; phone access stays in
  LOO-718. Scenarios and transcripts: `scratch/qa.html`. No product decision is open.
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

Sync checks (2026-11-09, merge `328593dcb`, inherited `LF_*` unset, stdin closed):
- `cargo test -p loopflow --lib server::service::tests::reconnect_resumes_an_unchanged_watch_without_a_complete_reading`: 1 passed; linker warned that `__eh_frame` exceeds 16MB. Gate owns broader checks.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter ServerProtocolTests/subscriptionsCarryTheirRequestId`: CLI/Desktop built; 1 passed. Build warned about unused results (including `ServerClient.swift:323`), deprecated APIs, redundant nil coalescing, weak-variable mutability and unnecessary `try`; gate owns static analysis.

Realign check (2026-11-09): `git diff --check` passed; `lf context` reports Product
memory at 15,987/16,000 tokens and scratch within both limits. Prose-only changes;
existing test results reused. Product memory was read in full; no child directories exist.

QA checks (2026-11-09; inherited `LF_*` unset, stdin closed):
- Real branch CLI/private Unix server: unchanged/changed/unknown resume, restored state, selection isolation, empty list, 61,440-byte body, cancellation, restart, terminal `ended`, and revision-2 refusal passed; all clients closed and server children reaped. Transcript: `scratch/qa.html`.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'ServerClientTests|ServerProtocolTests|WorkObservationTransportTests|DesktopHeadlessTests/workStates'`: CLI/Desktop built; 34 tests passed.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter WorkCLISubscriptionTests/serverReplacementKeepsWorkCurrent`: final version passed in 5.267 s, including cancellation during a second outage. Automated headless check, no mounted-window claim; gate owns broader checks.

Publication checks (2026-11-09): `lf sync --manual --json` finished cleanly with
`origin/main` at `4848816f7`, already an ancestor of HEAD. No production inputs
changed since QA. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
(44.93 s, inherited `LF_*` unset, stdin closed), and `git diff --check` passed.
PR copy now records the November 9 real-socket and headless Work-model checks;
broader affected suites and installation readiness remain with gate.
