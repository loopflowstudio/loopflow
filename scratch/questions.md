# LOO-499 direction — October 17, 2026

Jack Heart requested launching pursue after selecting resume as an alias for
connect, matching <work>/session skills on first creation, and case-insensitive
Task ticket identifiers everywhere. The earlier missing-design-review blocker
is superseded. See [loo-499.md](loo-499.md) for the reconciled contract, selected
routine implementation choices and remaining acceptance.

No further product decision currently blocks implementation. Demo/review and
merge approval remain with Jack Heart in primary Session
83e6ebf1845641fcb7332b668465bd40.

Jack Heart requested removing `--choose` on October 17 during implementation
review. Keep ordinary scope connect, exact Session opening and fresh creation;
remove the explicit primary-selection option and its plumbing.

Removal checks: cargo fmt; two focused cargo tests (Task primary selection and
contextual connect/resume/fresh); cargo clippy --all-targets -- -D warnings — passed.
Review: exact Session opening preserves the existing Task primary; removing the
CLI override leaves automatic selection and its store owner necessary.

Jack Heart authorized final polish, ship and landing LOO-499 on October 17.
The unassociated-checkout recovery hint now uses a shell-quoted canonical
absolute repository path. The contextual connect/resume/fresh test executes
the printed recovery command with a same-name repository under the lookup root;
it and cargo clippy --all-targets -- -D warnings passed after cargo fmt.
LOO-506 owns canonical ID storage and is separately authorized for autonomous execution.

Gate interruption and repaired cause: the native steering callback did not
deliver the operator review correction, so gate was interrupted before landing.
The prepare/message combination must fail before any Session admission, matching
the declared CLI conflict; silently dropping the message is not the contract.
The connection operation now enforces that boundary and has a focused regression
for connect and resume asserting unchanged primary/count and no provider launch.
The original live-client fresh-preparation test remains intact. Resume ship with
this fix; no design or merge judgment is pending from Jack Heart.

Before interruption, gate completed formatting/Clippy, a source-only architecture
check, and the container upgrade proof. /tmp/loo-499-upgrade.log records PASS for
populated 0.14.3 promotion with Task/Session identity, history, edits, move and
retry. The normal architecture runner only flagged ignored historical prompt
files, so source-only verification was used. Rust/website/Swift gate suites
were not completed; logs are under .lf/tmp/gate/run-39590 and
/tmp/loo-499-gate.log. Reuse prior proof only where still applicable; do not
claim the interrupted Rust run or pending UI suites passed.

Repair validation: cargo fmt; cargo test -p loopflow --test session_lifecycle_tests preparation_ (2 passed); cargo clippy --all-targets -- -D warnings — passed with inherited LF variables removed and closed stdin.

Final gate decisions: copied Sessions are read-only archives under docs/data-clones.md.
The clone fixture now reads its retained Task primary through session list instead
of using execution preparation as a lookup; no exception to Machine routing or
continuation authority is added. Remote alias routing, the obsolete rejection of
resume SESSION, and the retained-request fixture are repaired and verified.
The final check record is in scratch/loo-499.md. No product decision blocks delivery.
