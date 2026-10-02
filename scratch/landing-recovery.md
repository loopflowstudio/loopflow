# PR 1393 landing recovery

Jack Heart reported `lf ci-fix -b && lf pr land -c` stopping with `needs integration (dirty)`. The earlier ci-fix saw pending CI and made no repair. Current inspection found a completed Rust CI failure in `engine::skills::tests::sync_skills_compiles_builtin_skills`: the test still read the removed `loopflow/SKILL.md` export.

`lf sync` reproduced a delete/modify conflict in the retired builtin `loopflow.md`. Resolution retained its deletion and transferred main's removal of `lf ask` guidance to `repo_operate.md`. Sync completed and GitHub subsequently reported the PR clean. This establishes the current integration blocker; it does not establish precisely when GitHub's earlier mergeability changed.

Updated the export test to read `repo/operate/SKILL.md` and assert both retired skill names are absent for both vendors. Review found no need to restore compatibility exports or change production behavior.

`cargo test -p loopflow --lib engine::skills::tests` — passed, 4 tests; `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed; replay of `lf pr land -c` pending.

The installed lf reports 0.12.29 and watches landing; current source documents a newer finite reconciliation workflow. The legacy invalid-Run warnings are separate from the reproduced merge conflict and remain outside this repair.
