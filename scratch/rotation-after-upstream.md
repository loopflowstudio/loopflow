# Carry follow-up after upstream advances

LOO-375 rotation on installed 0.13.7 restores its original branch after conflicts replaying sync merge 4dffe8c1a. That merge imported 0.13.6; today's target already contains later edits to those same upstream files. Replaying against parent one unnecessarily reapplies old upstream changes. The earlier test kept main fixed after synchronization and missed this case.

For a merge with an incoming parent already ancestral to the new target, replay relative to that parent, retaining branch-specific changes and merge resolutions. Otherwise retain first-parent replay. Ordinary commits and intentional empty checkpoints survive. Any conflict must restore the entire sequence, original branch, and dirty edits. Add an upstream-advances-after-sync regression with and without merge-authored edits, plus existing all-or-nothing conflict coverage.

Review: choosing parent two only when it is already in the target preserves merge-authored edits while avoiding stale upstream replay. Pure sync merges have no remaining tree delta and are omitted; ordinary authored empty commits remain. Per-commit replay restores the original target after any error, including Git read/launch failures, not only conflicts. The real failed LOO-375 merge has exactly the same tree as parent two.

Checks: cargo fmt and cargo clippy --all-targets -- -D warnings passed; 2 focused tests passed before final rollback restructuring; identical-command final-content test rerun is compiling in /tmp/infra-rotation-tests-final.log.

Final validation: both focused tests passed after rollback restructuring. Commit ca169a477; PR #1472 published, auto-merge requested through lf land (exit 0). Merge and installed retry remain pending.
