# LOO-666 — Keep unpublished worktrees

2026-10-27. Accepted outcome: Jack Heart's Task brief and comment
`ab839681-400b-44b9-9c3e-7b32b392dadc` authorize repair through merge.
Implementation design: selected here; implementation and delivery remain open.

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

## Findings at base 5c9c22ac36f327d11368654218902a63f3f88b1b

- `git/worktrees.rs::apply_network_enrichment` sets `remote_gone` whenever a
  nonempty remote branch set omits a local name. `worktree_prune_reason` treats
  this as terminal. `manual_prune_removes_recent_remote_gone_branch` already
  constructs a never-pushed unique commit and asserts its deletion: replace this
  incorrect regression contract.
- `lf/commands/ops/mod.rs::prune_repo_worktrees` always chooses
  `WorktreePrunePolicy::manual()`. The hourly script calls ordinary `lf wt prune`,
  so changing only `automatic()` would not fix the installed route. Seven-day
  inactivity also deletes undelivered work through `abandoned_prune_reason`.
  A non-GitHub remote counts as known-empty PR evidence even when its ref read
  fails, so an absent remote is not the only unsafe path.
- PR enrichment already uses `head_pull_request`: merged/closed evidence applies
  only to the exact local head. Extra local commits invalidate the earlier PR.
  Preserve this mechanism. `local_states` deliberately calls contained/no-delta
  branches fresh; ancestry alone cannot distinguish a new checkout from finished
  work. Its `squash_merged && !fresh` prune arm is unreachable for ordinary local
  states (squash equivalence sets fresh).
- `task_ownership` reads unfinished Task paths plus process activity, and checks
  the selected and production registries. Read errors stop cleanup. Existing
  `running_paths`, path protections, persistent-worktree checks and checkout leases
  remain authoritative; no Task state or Session lifecycle redesign is needed.
- `remove_worktree_target` calls `worktree_remove`, whose leased implementation
  always invokes Git with `--force`, then deletes the branch without an expected
  head. Classification precedes removal, leaving room for changes between them.
  `ops/wt.rs` already checks expected heads for branch deletion; use that existing
  mechanism for the local ref rather than adding another branch deletion policy.
- `ops/branch_prune.rs` already requires positive evidence at each copy's exact
  head and protects checked-out branches. Its behavior survives unchanged; it is
  the downstream consumer that must not erase the retained calibration ref.
- Existing Python cleanup tests stub `lf` as success. They prove file-retention
  rules, but cannot detect this bug. The acceptance fixture must execute real
  worktree pruning, not only assert the shell invoked it.
- Disposable Git probe: nonempty origin omitted a clean local branch with one
  unique commit; non-force worktree removal refused untracked files. Git still
  removed a checkout containing only ignored files. Non-force removal therefore
  does not promise preservation of ignored artifacts in a finished checkout.

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

### Alternatives and failure cases

- Merely deleting the `RemoteGone` arm leaves stale-age deletion exposed.
- Recording whether a branch was ever pushed cannot prove its current commits
  were delivered; it adds state without resolving the safety question.
- Treating ancestry or squash equivalence as completion confuses newly created
  checkouts with finished work. Keep their listing semantics, not deletion rights.
- Disabling cleanup permanently loses the accepted disk-reclamation outcome.

Success is boring: finished checkouts disappear while an old unpushed experiment
is still available next month. Failure would be a hidden caller retaining the
old age policy, using an earlier PR head, or forcing away edits after listing.
The deletion inventory and end-to-end fixture specifically cover these cases.

## Delete — do not maintain

- Remove `WorktreePrunePolicy`, `abandoned_prune_reason`, `branch_is_stale`, their
  exclusive timestamp plumbing, and the policy argument at all callers.
- Remove prune reasons `RemoteGone`, `Stale`, the unreachable `SquashMerged`
  route, and unused `Terminal` if the final caller inventory confirms no use.
  Preserve listing `remote_gone`, `squash_merged` and `fresh` observations.
- Replace tests asserting remote-gone/stale deletion with preservation assertions.
  Delete exclusive policy/age tests that no longer describe supported behavior.
- Remove forced removal as an implicit property of every worktree deletion;
  make callers' intended semantics explicit in the existing primitive.
- Update the pruning contract comments and user-facing documentation in place.
  No second cleanup policy, publication-history table, fallback, compatibility
  adapter or silent-success branch is part of this change.

## This slice and remaining work

One indivisible hotfix: classifier deletion, safe removal, caller cutover,
regression coverage and documentation land together.

1. Replace unsafe regression expectations; remove age/absence policy and its
   callers. Focused test: `cargo test -p loopflow --test integration worktree_tests::`.
2. Carry observed head to execution; use the existing lease and expected-head
   mechanisms, explicit force semantics and non-force pruning. Cover changed
   head/dirty candidates and preserve explicit `wt delete --force` behavior.
   Removal callers are `ensure_agent_worktree`, `ops/wt.rs::apply_delete`, the
   two owned release cleanup sites, and the Git helper test; retain their
   existing explicit disposal behavior and cover any changed release boundary.
3. Add real-CLI fixtures with local bare origin and deterministic `gh` answers,
   following `wt_prune_branches.rs`'s GitHub-URL/local-transport fixture. Exercise
   both normal/squash merged exact heads, closed heads, added commits, unavailable
   evidence and existing protection boundaries. No live GitHub mutation.
4. Extend `python/tests/test_disk_cleanup.py` to run the mechanical script with
   the built candidate for real `wt prune` operations. Isolate HOME/LF_HOME,
   account-home resolution and source roots through the existing test harness;
   pin LF_BIN inside the network boundary. Only external provider/cache tools are
   stand-ins. The current stubbed-lf unit tests remain useful for Home protections.
5. Publish and merge through the selected Graph. The Infrastructure operator
   owns patch publication/installation, installed disposable acceptance, then
   restoration of only the existing hourly entry. Merge alone is not installed
   acceptance; no competing release publisher or schedule change.

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
uv run python scripts/test.py --reuse-passing --rust-filter 'test(git::worktrees::) | test(worktree_tests::) | test(ops::wt::) | test(ops::branch_prune::) | test(wt_prune_branches::) | test(build_cleanup::)'
uv run pytest python/tests/test_disk_cleanup.py
```

The affected runner supplies formatting/static analysis, including
`cargo clippy --all-targets -- -D warnings`; reconcile the exact test selection
with the final removal-primitive caller inventory. The Python acceptance uses
the built candidate (build `cargo build -p loopflow --bin lf` if the harness has
not materialized it). Linux CI owns portable Rust coverage; macOS owns the
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

Check: 2026-10-27 `uv run python` disposable Git probe — passed unpublished-head and untracked-file preservation premises; observed ignored-only removal. No production cleanup or implementation tests run in kickoff.
