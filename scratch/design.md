# LOO-805: Git read retention

Scope retention to subscription readings. Served one-shot reads, including
`task.changes`, consult Git afresh while subscriptions remain active. Keep shared
subscription reads and side-by-side checkout listing fast. No open product question.

Implemented: retention belongs to a checkout subscription reading and follows
its bounded checkout/detail workers and nested request workers. Shared answers,
refresh-on-change and parallel checkout reads remain; starting a subscription
does not clear the map. Process-wide activation and the write-only bypass are
removed. The server README states the freshness contract and uses `parent` for
the comparison base. Git observations remain non-atomic while a checkout changes.

Upstream checked on 2026-11-08: fetched `origin/main` at `f61c153eb` is already
in this checkout, including base PR #1813 and request-ID retirement (#1856).
No implementation mismatch found during realignment.

Concept review (2026-11-08): no design change and no open product choice.
The interaction remains edit → request changed files on the existing connection;
subscription retention needs no client switch or persisted record. Reviewed
normal reads, failure/retry, worker unwind, and a second subscription sharing
answers. Replacing shared retention with a per-subscription map would lose the
required sharing; promising an atomic checkout snapshot would exceed the brief.
Keep the current contract. The same-connection workflow passed live QA below.
Review page: `scratch/concept-review.html`. This review inspected code and prior
check results; it did not rerun tests.

Remaining: gate, review, CI repair if needed, merge and post-merge verification
before Task completion. Sync again before delivery as other follow-ups land.
Gate coverage: `test(git::tests::) | test(journal::request::tests::) |
test(lf::commands::subscribe::) | test(lf::commands::waves::) | binary(served_reads)`.

Docs presentation: `lf open --host desktop --file rust/loopflow/src/server/README.md`
failed because this Task has no workspace in the attached Desktop window; no
presentation changed.

Check: `TMPDIR=/private/tmp cargo test -p loopflow --lib --test served_reads subscription_git_retention` (inherited `LF_*` unset) — 7 passed after compression; macOS linker emitted its unwind-table size warning. `cargo fmt` and `git diff --check` passed. Clippy and broader affected suites remain with gate.

QA (2026-11-08): `cargo build -p loopflow --bin lf` and
`uv run python scratch/qa-live.py` passed against the public socket protocol.
Two Task subscriptions stayed open while immediate reads saw edits, staging,
200 new files and their removal; invalid-input recovery and reconnect passed.
`scratch/qa.html` contains the transcript and limits. No product repair needed.
The isolated server was stopped, temporary state removed, and checkout probes
restored. Desktop rendering and signed installation were not exercised headless.
