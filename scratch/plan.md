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

- Delivery remains. The branch includes upstream
  operation-name alignment (#1855), Git watch freshness (#1860), exact Session
  opening/recovery (#1863), and pre-disconnect keystroke handling (#1839).
  These remain separate from subscription recovery; sync again before delivery.
- QA exercised real server reconnects, changed and unchanged readings, server
  replacement, and terminal completion. A headless production Work-model check
  retained the last reading during an outage, applied offline edits after server
  replacement, received subsequent edits and cancelled during another outage.
  No mounted Desktop or phone recovery result is claimed; phone access stays in
  LOO-718. Scenarios and transcripts: `scratch/qa.html`. No product decision is open.
- Gate passed the server, subscription, served-read, DTO and transport recovery
  suites, headless Swift consumers, and isolated installation replacement below.
  CI owns the full Rust matrix and Linux installation checks.

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

Gate plan (2026-11-09): synced onto `4848816f7`; no new upstream changes.
`server/{service,wire,client}.rs` and subscription identity reach `server::`,
`lf::commands::subscribe::tests::`, `served_reads`, DTO fixtures, session-list
subscriptions and transport's `server_` cases (watch updates, restart, readiness,
terminal reconnect). Swift protocol/client and Work recovery reach the runner's
headless app/model/view suite. Docs and QA skill changes reach website and
architecture checks. Run the changed-aware runner with those Rust filters and
reuse enabled, followed by the two isolated installation/registration checks
outside its network sandbox. Use rustup stable 1.99.1 (CI equivalent), resolved
TMPDIR, inherited `LF_*` unset and closed stdin. Full Rust matrix stays with CI.

Gate preflight attempt: resource inventory exceeded its 15 s uv-cache deadline;
no suites ran. Direct `du -sk ~/.cache/uv` then completed in 12.13 s with
14,275,436 KiB (13.6 GiB, below the 16 GiB limit), no permission errors; disk
had 171.6 GiB free. This was a slow shared-cache scan, not disk exhaustion.
Retry the unchanged check plan after that successful inventory; no limits changed.

Gate results (2026-11-09, rustup stable 1.99.1; same production tree as QA):
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(server::) | test(lf::commands::subscribe::tests::) | binary(served_reads) | test(dto_fixtures::) | test(session_list_subscribe::) | (binary(transport_tests) & test(server_))'`: PASS, four suites in 491 s; 108 Rust, 73 website and 783 headless Swift tests, architecture, rustfmt, Clippy with warnings denied, and Swift boundaries. Resource preflight/postflight passed. Log: `/tmp/loo775-gate-retry.log`; artifacts: `.lf/tmp/gate/run-64497/`.
- `cargo test -p loopflow --test transport_tests server_ -- --ignored --test-threads=1` with the prepared `LOOPFLOW_TEST_LF`, outside the network sandbox: PASS, 2 tests in 13.71 s; isolated service registration/disabled choice and signed installed-helper replacement retained the owner and Graph. Log: `/tmp/loo775-installation.log`.
- `git diff --check`: PASS. No production repair needed. Mounted Desktop/phone recovery and payloads near 32 MiB remain unverified; no display-dependent result claimed. Full Rust matrix and Linux installation remain CI-owned.
