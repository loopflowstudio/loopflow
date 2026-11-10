# LOO-790 served observations

Reconciled 2026-11-09. Served reads and CLI subscriptions capture
external fact dependencies through `observe`; bare reads and writes observe
inline. A process-owned cache keeps answers and revisions in memory, polls only
facts already read, and is shared across requests and subscriptions. Readers
keep the existing pinned SQLite snapshot and selected-answer deduplication.
Observation intervals belong to facts, never operation names. Errors are answers
and recovery advances the revision. Unused facts expire; no schema changes.

Implemented: the cache replaces per-read wake policies, the checkout watcher,
Git retention, config-file tracking and the Session observation probe.
Subscriptions compare external fact revisions and existing non-usage store
domains; per-read store-domain selection is outside this change. Process answers
use `OsProcess` equality. Quiet availability and planning staleness use observed
deadlines; rolling history uses a minute-resolution clock.

Acceptance: shared reads reuse observations; external Git/file/process/PR/CI changes
wake dependent subscriptions without store writes; unrelated facts do not rerun
a reading; argument changes replace dependencies; fresh CLI/writes bypass cache;
nested workers inherit scope and unwinding restores it; failed observations
recover. No external facts enter SQLite; server replacement starts fresh and
unused facts expire. Screen-shaped reads remain a separate Task.

Concept review, 2026-11-09: no design change or open product choice. Keep the
experience as “leave the Task open; edits and CI changes arrive when observed.”
No refresh command, persisted observation record or additional client state is
needed. Cache recovery does not change the existing protocol: a served watch
that ends with `unavailable` must be opened again by its client; a restarted
server supplies a complete baseline. Bare reads and writes remain direct.
The review page is `scratch/concept-review.html`.

Counterexamples for gate: a missing checkout appears without a store write;
a provider fails and later recovers; a quiet deadline passes while no process
or row changes. Each must reach the actual dependent reading. An unrelated
external fact must not rerun it, and a changed unselected field must not emit
a frame. These are the existing acceptance requirements, not new design work.

Remaining: hosted CI and landing. Local checks now cover the affected callers,
quiet-time transitions and full clippy. QA exercised live served Git/file reads,
process exit and missing-checkout recovery; results are in `scratch/qa.html`.
No product decision is open.
Fetching origin/main during realign found it still at the checkout base
`28b0af568`; delivery still needs a fresh sync if main advances.

Documentation now distinguishes CLI subscriptions from bare reads and describes
the existing store-domain behavior. The documentation pass's
`lf open --host desktop --file docs/server.md --file rust/loopflow/src/server/README.md`
failed: "has no workspace in this Desktop window; nothing changed."

Gate coverage: `observe::tests::`, `lf::commands::subscribe::tests::`,
`harness::fact_writer::tests::`, `journal::request::tests::`,
`journal::os_process::tests::`, `journal::tests::`, `lf::commands::top::tests::`,
`skills::catalog::`, `graph::tests::`, `store::sqlite::sessions::`,
`ops::task::tests::`, affected PR observers in `engine::pr` and `ops::pr`,
`binary(served_reads)`, `test(session_list_subscribe::)`.

Checks (inherited LF variables unset):
- `cargo test -p loopflow --lib observe::tests::` — passed after compression (7 cache tests plus 3 matching Linear tests).
- `cargo test -p loopflow --lib lf::commands::subscribe::tests::` — passed after compression, 6 tests.
- `cargo test -p loopflow --lib harness::fact_writer::tests::served_availability_follows_the_quiet_deadline_without_a_store_write -- --exact` — passed.
- `cargo fmt` and `git diff --check` — passed; full clippy and served protocol acceptance belong to gate.
The macOS test linker warns that `__eh_frame` exceeds 16 MB; tests pass.
Compression's first stop-state regression failed on macOS: its native observer
maps live states to `S`, including stopped processes. Keep that existing platform
behavior; the stop-state regression runs on Linux in CI, with exit coverage on both.

QA, 2026-11-09: live isolated server and CLI walkthrough passed; details and
limits in `scratch/qa.html`. Updated `.lf/skills/qa.md` with the working recipe.
No production repairs. Real provider quiet-time and GitHub CI transitions remain
unexercised in the private home, which has no provider/GitHub login.

- `cargo build -p loopflow --bin lf` — passed (QA).
- `swift build --package-path swift -Xswiftc -gnone` — passed (QA).
- `uv run python scratch/qa_served.py` — passed with Swift-encoded live messages (QA).
- `uv run python scratch/qa_cli_subscription.py` — passed, no server running (QA).
- `scripts/test_desktop.sh --filter 'WorkCLISubscriptionTests/serverReplacementKeepsWorkCurrent|DesktopHeadlessTests/workStates' -Xswiftc -gnone` — 2 passed with inherited LF variables unset and stdin closed (QA).

Publication check, 2026-11-09: `lf sync` succeeded with main still at
`28b0af568`. The affected Rust run passed 232/233 tests; served account routing
still asserted an immediate answer after editing configuration. Updated that
test to await the expected observed answer after each mutation, retaining its
CLI parity assertions. The repaired test and clippy still need to run.
Website and architecture passed. Swift stopped in
`PortfolioServiceTests.desktopWritesReadBackThroughCLI`: the mutation returned
the imported repository but the following served read retained the old folder
selection. That fixture now waits for observation convergence, and requires the
repository count before indexing. Stopped its orphaned private server (PID 63832).
The first recheck ran no product suites: resource inventory exceeded its
15-second deadline measuring `/Users/jack/.cache/uv`; the preceding postflight
reported 131.1 GiB free. Rechecking through the normal runner.
The recheck passed all 111 selected tests (the repaired account test and Git,
config, Wave and server callers). Full clippy found one needless borrow in
`ops/human_session/workspace.rs`; removed it. Swift and final clippy are pending.

Final publication checks (inherited LF variables unset):
- `scripts/test.py --reuse-passing --rust-filter <Gate coverage above>` — architecture passed; 232 Rust tests passed, one repaired account assertion; website 73 passed. The initial Swift run stopped at the repaired portfolio assertion.
- `scripts/test.py --base HEAD --reuse-passing --rust-filter 'test(account_holds_and_rotation_use_the_same_machine_state_as_the_cli) | test(git::tests::) | test(config::tests::) | test(lf::commands::waves::tests::) | test(server::)'` — all 111 Rust tests passed; full headless Swift 782 tests passed, app launch-and-quit passed, Swift boundaries passed. Clippy's one needless borrow was fixed afterward.
- `cargo fmt`, `git diff --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings` — passed after that fix.
No private server remained after verification. Real GitHub/provider QA and
mounted-window interaction retain the limits in `scratch/qa.html`.
