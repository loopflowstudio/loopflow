# Worktree names after a repository rename

Jack reported a bare `managym-cpu-research` worktree where
`~/src/fantasia.managym-cpu-research` was expected. Git lists the actual checkout
at `/Users/jack/src/managym-cpu-research`, alongside live `etude.*`, `fantasia.*`
and two other bare-name worktrees. Its common directory is `fantasia/.git`.

The inference introduced in #1536 treats any sibling whose name lacks the
current repository prefix as evidence for bare names. Renaming the primary
checkout invalidates that test; every subsequent creation reinforces the wrong
convention. Existing tests cover dedicated host directories, but omit renamed
repositories and mixed sibling names.

Use the primary checkout's current name for all new siblings. Continue inferring
dedicated host directories from consistent live checkouts. Reuse existing
branch checkouts in place; do not move active work or require rename metadata.

Observed failure: installed `lf wt create location-probe --plan` in Fantasia
selected `/Users/jack/src/location-probe`. The new integration regression failed
on the same missing-prefix assertion before the repair.

The repair removes name-based inference for siblings. A disposable real-Git
fixture renames the primary checkout from `etude` to `fantasia` to `manabot`,
creates checkouts through `lf wt create` from an older sibling, and verifies
planning from both primary and linked checkouts. Mixed historical prefixes and
bare worktrees do not change new sibling names; existing branches are reused
and uncommitted work is preserved.

The candidate's read-only CLI plan in both `/Users/jack/src/fantasia` and
`/Users/jack/src/managym-cpu-research` selects
`/Users/jack/src/fantasia.location-probe`. Run the candidate with a disposable
`LF_HOME`: ordinary execution with the main Home intentionally dispatches to
the installed CLI, which still has the bug. No Fantasia worktrees were moved or
created. Installing/shipping the candidate remains outstanding.

Review: sibling placement no longer depends on historical directory spellings;
dedicated host-directory inference and existing-checkout reuse retain their
existing tests. No configuration or rename-history mechanism is needed.

Checks: network-isolated `cargo test -p loopflow --test integration` filtered to worktree (35), startup (1) and release-lock (15) suites, plus `cargo test -p loopflow --test release_tests` (70): 121 passed; `cargo fmt --check`, `git diff --check` and `cargo clippy --all-targets -- -D warnings` passed. Logs: `/tmp/lf-worktree-rename-{red,tests,shared-tests,clippy}.log`.
