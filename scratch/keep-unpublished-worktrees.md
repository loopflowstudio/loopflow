# LOO-666 — Keep unpublished worktrees

2026-10-27. Accepted outcome: Jack Heart's Task brief and comment
`ab839681-400b-44b9-9c3e-7b32b392dadc` authorize repair through merge.
Reconciled 2026-10-27 against `d98b9f4e4e`. Implementation and focused
verification are complete. Gate and delivery remain with the selected Graph;
no product decision is pending.

## Problem and demo

Jack needs hourly disk reclamation without losing unfinished or unpublished
work. Installed 0.14.19 deleted MTG-193's auxiliary calibration checkout because
its branch was absent from origin. A clean checkout can hold the only copy of
committed work. Neither remote absence nor inactivity establishes completion.

Demo: in a disposable repository, run `lf wt prune --dry-run`, then `lf wt prune`.
A clean never-pushed calibration branch with a unique commit remains, including
its file and branch ref; a clean checkout whose exact head has a merged PR is
removed. Repeat through `scripts/disk-cleanup.sh` with the real candidate binary
and isolated Home/source roots. Unpublished work survives the script's subsequent
build and branch cleanup too. A second run has nothing further eligible to remove.

This serves Infrastructure's preservation and reliable unattended-work objective
and its chapter's plumbing/continuity KRs. No quantitative target was supplied.

## Evidence

The original investigation and alternatives are preserved at
`315e2deda34e7e1ee6675ea8429d8fd6cec58b62:scratch/keep-unpublished-worktrees.md`.
At base `5c9c22ac36`, ordinary and hourly pruning both selected the manual policy:
remote absence or seven-day inactivity authorized forced removal. Existing tests
even required deletion of a never-pushed unique commit. PR enrichment already
matched the exact local head; reuse that evidence and the ownership/lease readers.

Non-force Git removal refuses tracked/untracked edits but can remove ignored
files. This hotfix does not promise ignored-artifact preservation in a finished
checkout. Earlier shell tests stubbed `lf`; the added fixture executes the built
CLI so it can detect the original deletion rather than just its invocation.

Upstream `0d72f072a4` is integrated by `d98b9f4e4e`; its document-context
changes do not alter cleanup. Infrastructure and its immediate Release child's
complete goal/memory files were reviewed. Release's operation-entry lesson
supports the existing real-CLI script fixture and the separate installed proof;
no change to the accepted cleanup approach follows.

## Chosen approach

One worktree-pruning classifier, used by ordinary CLI and hourly cleanup, requires
positive terminal PR evidence at the current local head: merged or explicitly
closed, with all existing protections applied first. Closed PR remains the
existing explicit abandonment signal; it is not described as merged delivery.
An open PR is retained. Missing PR evidence, a missing remote name, old activity,
local containment or tree equivalence alone never authorizes automatic deletion.
Fresh non-GitHub work and non-GitHub checkouts without positive terminal evidence
may remain; explicit `lf wt delete` continues to express deliberate disposal.

Remove the manual/automatic policy distinction rather than adding a cron flag or
a script-specific rule. Keep `remote_gone` as a listing observation with its
current wire shape; it conveys no permission to prune. No schema or DTO change
is needed. Local Git refs/contents, exact-head provider PR facts, and existing
ownership/liveness readers remain the sources of truth; there is no new ledger.

At execution, hold the existing checkout lease, verify the candidate's branch and
head still match the classified values, and repeat cleanliness checks. Use
non-force Git removal for pruning, followed only by local expected-head ref
deletion. Retain/report a changed or uncertain candidate. Make force explicit
at the shared removal primitive and audit its callers so explicit manual force
and existing owned cleanup operations keep their intended semantics. Do not
delete a remote branch as a side effect of pruning a checkout. This bounds the
existing race; it does not claim an atomic transaction with arbitrary external
Git writers. Non-force Git remains the last check for tracked/untracked edits.

Remote failures/empty answers provide no terminal evidence. Ownership or process
inspection errors still stop cleanup; failed cleanliness/head reads retain the
candidate. Keep current network batching/time bounds; no per-worktree provider
query, fetch loop, new cache, daemon or retry. Dry-run uses the same classification
and performs no checkout/ref deletion. Real pruning retains its existing main
refresh; acceptance fixtures hold facts constant across dry-run and mutation.

## Implementation and remaining work

One indivisible hotfix: classifier deletion, safe removal, caller cutover,
regression coverage and documentation land together. The code now removes
age/absence authorization and carries the exact PR-matched head from listing
through the checkout lease to non-force removal and conditional local ref deletion.
Explicit disposal and the two owned release cleanup sites retain force semantics.
`release_lock_tests.rs` also calls the removal primitive; its fixtures now name
force explicitly. `Listing.pull_requests_known` was exclusive age-policy plumbing
and is removed. No schema or wire DTO changed.

Portable real-CLI fixtures cover normal/squash merges, closed PRs, moved heads,
missing/malformed/empty evidence, dirty/untracked files, persistent checkouts,
unfinished Tasks, unreadable ownership, failed process inspection and a live cwd.
Removal tests cover branch/head/file changes, an occupied lease, conditional local
ref deletion and explicit force. The macOS script fixture executes the built CLI
through ordinary pruning, build cleanup, branch pruning and a second run.
Its temporary `getpwuid_r` interposer redirects OS-account reads as well as HOME;
this is a source-fixture proof, not installed acceptance. Existing script tests
continue to own database/sidecar/account-home/newest-backup protections.

Remaining:

1. Gate runs the affected suites below once, including the release-lock callers.
2. Publish and merge through the selected Graph. The Infrastructure operator
   owns patch publication/installation, installed disposable acceptance, then
   restoration of only the existing hourly entry. No competing release publisher
   or schedule change is part of implementation.

## Done when

The gate scenario creates a nonempty origin and worktrees for: recent and old
never-pushed unique commits; remote-deleted undelivered work; merged/closed PRs
at exact heads; new commits after those PRs; dirty tracked and untracked files;
fresh/open PR work; protected unfinished Task/process/persistent checkouts.
Assert actual paths, file bytes and branch SHAs after dry-run, ordinary prune,
and the real cleanup script. Only clean unprotected exact-head terminal work is
removed. A prior PR at another head is insufficient, even if origin lacks the
branch. Ref/PR outages, malformed responses, empty answers, unreadable ownership
and unknown process inspection never manufacture removal authority. Existing
build-cleanup, branch-prune, database/sidecar, account-home and newest-backup
protections continue to pass.

Headless gate commands, with inherited LF_* unset and stdin from `/dev/null`:

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(git::worktrees::) | test(worktree_tests::) | test(ops::wt::) | test(ops::branch_prune::) | test(wt_prune_branches::) | test(build_cleanup::) | test(release_lock_tests::) | test(git::tests::local_branch_deletion_retains_a_new_head)'
uv run pytest python/tests/test_disk_cleanup.py
```

The affected runner supplies formatting/static analysis, including
`cargo clippy --all-targets -- -D warnings`. The selection includes the
release-lock callers of the shared removal primitive. The Python acceptance
builds its candidate with `cargo build -p loopflow --bin lf` before running it.
Linux CI owns portable Rust coverage; macOS owns the
mechanical script checks. No display or person is required. Installed verification
repeats the same disposable scenario with the installed executable/script.

## Recovery and exclusions

The operator reports preserved history at
`refs/heads/recovery/mtg193-calibration-20261028` and a verified complete-history
bundle. Known readable commits include `950e2066e`, `625283822`, `7616dd740`;
the assigned MTG-193 checkout and output/profile directories survived. No
pre-deletion inventory proves whether uncommitted/ignored data existed. Recovery
is not claimed complete. Auxiliary-checkout restoration is coordinated with the
ongoing MTG-193 conversation; its active checkout and Task remain untouched.
Recovery receipts stay with their existing owner, not copied into this checkout.

LOO-543's Session-loss investigation, LOO-664's sync fix, a general cleanup
redesign, Git gc/prune recovery operations and account configuration are excluded.
Ignored build-output retention remains governed by the existing script contract;
this hotfix does not redefine ignored files as source or claim they are backed up.

Check: 2026-10-27 `git diff --check` — passed; `lf context` — memory/scratch within both limits. Reused unchanged-code results: `cargo test -p loopflow --lib git::worktrees::tests::prune_`, `cargo test -p loopflow --lib git::tests::local_branch_deletion_retains_a_new_head`, `cargo test -p loopflow --test integration wt_prune_branches::worktree_prune` — 7 passed; `cargo fmt` and `cargo clippy --all-targets -- -D warnings` — passed. Prior explicit-force and macOS script acceptance receipts: `315e2deda34e7e1ee6675ea8429d8fd6cec58b62:scratch/keep-unpublished-worktrees.md`. Gate owns affected suites; the operator owns installed acceptance.
