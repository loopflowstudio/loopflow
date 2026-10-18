# Foundations review

The October 16 operator findings and complete responses are preserved at
`refs/loopflow/evidence/loo-497-before-compress:foundation-review.md`.

The suspected path/plan-ID mismatch was disproved: `RepositoryWorkspace` resolves
and forwards a durable repository ID; the opening path is separate.
`stateUsesDurableRepositoryIdentity` exercises that boundary. No conversion shim.

`nested_query_failure_retains_prior_members_but_confirmed_departure_prunes`
proves missing saved-query evidence retains intent and subsequent complete reads
prune departures without writing saved bytes. It also covers stale deletion.
`selection_reads_compose_with_the_work_snapshot_and_report_failed_reads` proves
savepoint composition and errors for an unknown repository.

Compression removed repeated Work reads per saved query, subtree revalidation,
a redundant membership filter and a read-before-delete transaction. All catalog
entries originally validated against the same Work snapshot; October 17 compression
narrows validation to the edited selection and its references so unrelated
ambiguity cannot block edits. Conditional deletion checks revision atomically.
Shared Work identity is imported from its owner.

Publication review reproduced the broad selection failure: the binary-selection
fixture changed `LF_HOME`/`LF_BIN` without the shared environment lock. It passed
alone but raced other fixtures in the broad run. The fixture now takes that lock;
production dispatch is unchanged. Publication remains a review checkpoint, with
native integration and the released-frontier gate still deferred as documented.

Check (October 16, foundations): `uv run python scripts/test_network.py cargo test -p loopflow --lib selection < /dev/null` — 34 passed after the lock repair; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` — passed; prior unchanged Swift/DTO checks reused, native integration remains gate-owned. The later post-merge upgrade pass in `design.md` resolves the released-frontier deferral.
