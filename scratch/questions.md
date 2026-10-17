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
