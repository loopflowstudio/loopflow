# Historical uncertainty acceptance

Jack Heart authorized autonomous implementation and delivery on October 5, 2026:
complete delivered LOO-326 with explicit acceptance of historical Exec
`5f239ead-89f9-49c4-92c4-4c2f8b97ca94` remaining unknown. Preserve its history;
acceptance grants neither process control nor checkout deletion.

Implemented: `task complete --accept-unknown-exec` records exact IDs and the
completion summary in Task history. Completion rechecks eligibility: no process
receipt, current caller/Session/Flow owner, current Exec, or established exit.
Acceptance survives a refused completion for retry, but cannot settle a PR.
Admission and cleanup retain the original execution checks. Compression removed
the redundant liveness lookup; any receipt already disqualifies acceptance.

Remaining: affected gate/CI and delivery of this change, then completion through
the published installed CLI with the exact accepted Exec and a summary naming
Jack Heart's decision. Report actual Task status, unchanged unknown Exec fields
and retained checkout. The focused test proves the gate and preservation rules
but calls store completion directly; it does not establish the public command's
acceptance/writeback path or installed completion. The original fault repairs
are shipped per the supplied steers; this change addresses their remaining
Task-completion boundary. No new product decision is required.

Checks: `cargo test -p loopflow --lib accepted_historical_uncertainty_completes_without_releasing_execution_protection -- --test-threads=1` (inherited LF/LOOPFLOW authority cleared, source LF_BIN pinned), `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`: passed; affected suites and installed completion remain with gate/delivery.
