# Paused CI repair

PR #1544 run 38445800489 failed three Rust tests. Nonterminal ownership now reads only completion/abandonment, retaining NULL completion as unfinished and avoiding candidate schema/Workflow dependencies. Planning admission checks terminal planning before reading execution state; valid retained planning needs no candidate Paused column. The CLI contract explicitly accepts Task pause/resume while keeping removed root commands absent. Review preserved the released-schema fixtures rather than adding candidate columns to conceal the failures.

Check: all three failed Rust tests plus pause/resume retention PASS; cargo fmt, all-target Clippy (-D warnings), git diff --check PASS; /tmp/task-paused-ci-repair.log. Other CI jobs passed on the preceding head; new-head CI remains required. No live DB or demo app changes.
