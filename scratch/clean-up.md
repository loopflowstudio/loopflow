# Automatic worktree cleanup

Accepted for core implementation — 2026-10-09. Jack Heart requested
`lf pursue -b` after reviewing the investigation and proposed direction. The
worktree-cleanup keystone is the selected PR; artifact eviction and history
rotation remain named follow-ups. Their detailed policies remain draft.
Implementation authorization does not request one-off deletion on either machine.

> “lf is aggressive about cleaning up worktrees so that we are not causing a bunch of waste on our customers computers”

**Accepted direction, 2026-10-09:** Jack Heart considers Etude experiment output
disposable too. In response to excluding the 17.9 GiB `.runs` directory from
cache cleanup, Jack said: “I think i would say it can”. Experiment output is not
automatically archival data. Protection while running and explicit retention for
selected results are proposed mechanics; expiry and byte budgets remain draft.
Jack also asked whether the 62 GiB Loopflow home can be rotated. Rotation and
compression are in scope for design; expiry of usable conversation history has
not been approved.

## What to build

Make the installed product automatically reclaim finished checkouts and declared
disposable artifacts, retry deferred cleanup, and preserve unfinished source.

## Placement

**Infrastructure**: machine resource stewardship and reliable lifecycle cleanup.
Selected from `lf wave list --json` on 2026-10-09. No Task filed or Flow launched.

## Shape and boundary

Additive series. The keystone is one cleanup owner plus automatic worktree
collection. It replaces today's overlapping deletion decisions in one PR.
Independent follow-ups cover artifacts in retained checkouts and other storage;
they must use this owner rather than introduce another cleaner.

## The demo

Finish a Task while another Process still uses its checkout. Cleanup reports
“retained: running Process.” Exit that Process. Without another agent, test run,
or manual prune, the next maintenance pass removes the checkout. A neighboring
unfinished Task, dirty checkout and branch with post-merge work remain intact.
`lf wt prune --dry-run --json` explains each decision and estimated bytes.

## Current system

- `ops/task/lifecycle.rs::cleanup_completed_task` deletes settled Task PR
  checkouts. Task movement and successful Flow completion call it.
- `ops/pr_landing.rs::cleanup_landed_pr` handles taskless landings, but its
  reconciliation query excludes already-merged landings. A skipped cleanup is
  not an enduring retry obligation. A Flow completion retry exists for Tasks;
  it does not cover every later blocker disappearing.
- `engine/worktrees.rs::prune_worktrees` serves manual `lf wt prune`. Automatic
  policy and targeted prune helpers exist without production callers.
- Manual prune treats remote disappearance, closed PRs and seven-day inactivity
  as deletion reasons. These do not prove that local work is disposable.
- `scripts/resource_envelope.py` cleans build roots during this repository's
  verification. It is not installed customer maintenance.
- `ops/cron.rs` already has a per-repository, every-minute OS job running
  `lf task reconcile --json`. Reuse that execution path; do not add a daemon.

Detailed measurements and evidence limits: [cleanup-evidence.md](cleanup-evidence.md).

## Data structures

Reuse Task, TaskPr, Process, Git worktree registration and persistent-worktree
metadata as facts; no cleanup Task status or second worktree registry.

`CleanupDecision { path, observed_head, action, evidence, estimated_bytes }`;
`action = RemoveCheckout | Retain(reason)`. Size may be unknown, never silently
zero. A report distinguishes planned, removed, deferred and failed paths.
Evidence records exact-head settlement, not merely a branch's old PR state.
These are transient projections, not another SQLite authority. No schema
migration is proposed for the keystone.

## Key functions

In `ops/wt.rs`, shared by CLI, lifecycle and maintenance:

- `plan_cleanup(store, repo, observations) -> CleanupPlan`: classify all
  registered checkouts, including previously settled owners.
- `apply_cleanup(store, plan, budget) -> CleanupReport`: acquire existing
  checkout admission and Git leases, re-observe, then remove eligible paths.
- `run_cleanup_pass(store, repo, budget) -> CleanupReport`: bounded collection
  for the scheduled repository. No home-directory crawl, PM fetch or agent.

Lifecycle events attempt targeted cleanup. Extend the existing repository tick
with cleanup independent of delivery/network success. Cheap settled-owner checks
run each tick; a full reconciliation scan runs hourly. Ensure that schedule when
lf first creates work in a repository, including taskless work; repair its
executable on upgrade and remove it on uninstall. CLI-only installs get the same
behavior. Unsupported scheduling remains explicit; a throttled ordinary-command
trigger is fallback, not an agent conversation. Proposed bound: eight removals
and 30 seconds admitting new work per pass; a started removal finishes. Oldest
deferred candidates go first. A nonblocking machine lock prevents overlapping
passes; misses retry next pass. Reuse bounded cron receipts for reports and scan
timestamps, not a new durable cleanup queue. Measure candidate sizes off the
interactive path; a scan deadline yields unknown bytes, not a foreground stall.

## Constraints

Automatic checkout removal requires known Loopflow ownership, no unfinished
Task using the path, exact-head settlement/disposition, and no live or unknown
execution. Preserve primary and persistent checkouts. Preserve uncommitted,
untracked and unclassified ignored content: Git “clean” alone is insufficient.
Known regenerable artifacts can go with the checkout; ignoring a file is not
permission to erase it. Establish explicit artifact contracts before exempting
such roots. Existing Task checkout links and persistent metadata are ownership
evidence; a familiar directory name is not. For new taskless worktrees, record
lf creation provenance in their Git administrative directory at creation. Unowned
legacy trees are report-only. Explicit regenerable-root declarations and valid
tool cache tags can classify ignored artifacts; unknown ignored files retain
the checkout. Cleanup must preserve or relocate referenced local Session evidence
before removing a checkout, not assume every `.lf/` file is expendable.

Recheck under admission/lease before deletion, including new branch commits,
processes and filesystem changes. Failed observation retains the affected path,
not every repository. Existing external-process inspection must preserve unknown
liveness rather than convert inspection failure into an empty process set.
Remove only proven disposable artifacts before Git's non-forced worktree removal
provides the final dirty-file check; never force-remove arbitrary files. Delete
local refs conditional on their observed head. External writers cannot be completely fenced by
an lf-only lock; uncertainty is retained and named.

Automatic collection deletes local disposable checkouts, not remote branches,
PRs, Task outcomes or Session history. Explicit abandonment keeps its existing
authority. Local branches are removed only when their exact head is settled;
retaining a small unresolved ref is preferable to losing commits. Cleanup failure
never reverses completion or fails unrelated work. Dry-run makes no cleanup
mutations, including Git metadata pruning.

## Delete — do not maintain

- Replace `WorktreePrunePolicy::{manual,automatic}`, `abandoned_prune_reason`,
  `branch_is_stale`, `worktree_prune_reason`, and the unused targeted-prune APIs
  with the shared classifier. Delete tests exclusively asserting stale/remote-gone
  deletion; retain dirty, persistent, unknown-PR and new-commit protection tests.
- Remove `prepare_landed_delete`, whose automatic path also deleted remote refs.
  Explicit `prepare_delete` / abandonment authority remains separate.
- Remove duplicate automatic eligibility/deletion branches in
  `cleanup_completed_task` and `cleanup_landed_pr`; keep their completion and
  persistent-branch behavior, delegating filesystem collection to the owner.
- Move CLI-only `protected_worktree_paths` into shared observation. Replace
  cleanup's fail-open `running_workspace_paths` use without breaking display.
- Remove `prune_stale_worktree_metadata` from preview. Keep metadata repair only
  on apply, after ownership/path inspection.
- Remove orphan `engine/worktree.rs` create/remove helpers and exclusive tests
  if the verified whole-repository caller search remains empty. Keep any used
  root-resolution API.

## Forbidden outcomes

An hourly invocation of today's broad manual prune; age equaling abandonment;
ignored equaling disposable; a successful Flow equaling Task completion;
cleanup requiring an LLM; mandatory foreground recursive disk scans; deleting
another machine's state; moving junk to an unlimited trash directory.

## Implementation checkpoint — 2026-10-09

The shared collector is in `ops/wt/cleanup.rs`. Manual prune, completed Tasks
and taskless landings use it; repository reconciliation also attempts a local
pass before network delivery observation. The predecessor prune policies,
targeted-prune APIs, prompt-log pruner, automatic remote-branch deletion helper
and orphan engine create/remove helpers are removed. Explicit abandonment keeps
its separate authority. This is an internal checkpoint, not a shipping boundary.

Current eligibility uses Task checkout links or recorded landing paths, and
exact merged heads. New lf-created checkouts carry Git-administrative provenance;
provenance alone does not settle their source. Unknown settlement retains the
checkout. Valid `CACHEDIR.TAG` declarations classify wholly ignored directories;
other ignored data stays. No arbitrary-name artifact exemptions were added.

Review fixes: non-forced Git removal is the last dirty check; local refs use
compare-and-delete; failed worktree creation cannot confer lf provenance on a
preexisting checkout; assume-unchanged/sparse index entries retain the checkout;
cache tags survive partial content removal and signature reads are bounded to 43 bytes. A removed checkout is still reported
as removed if its local ref changed and was retained.

### Remaining in this PR

1. **Installation and retry coverage:** automatically ensure the repository tick
   on first Task or taskless work; repair its executable on upgrade and retire
   it with the supported uninstall/disable lifecycle. Current coverage requires
   an already-installed repository tick or explicit reconciliation. There is no
   current `self uninstall` command; installation integration must identify its
   actual retirement boundary rather than assume one exists. Unsupported hosts
   still need explicit coverage and the throttled ordinary-command fallback.
2. **Bound the whole pass:** current apply limits are eight removals and 30 seconds
   admitting removals; planning is not yet deadline-bound. Share cheap observations
   rather than re-reading all Tasks per candidate. Add hourly full reconciliation,
   oldest-deferred ordering, background size estimates and cron-receipt summaries.
   Foreground JSON truthfully reports unknown byte estimates as null.
3. **Remaining recovery/safety cases:** targeted missing-registration repair,
   interruption after tag removal but before empty-directory removal, release-store
   execution/admission fencing during experimental collection, and full referenced
   Session-evidence coverage. Missing paths currently retain registration. A
   published abandoned PR without exact-head disposition is retained.
4. **Gate/demo:** complete the scheduled skipped-then-idle demo through the installed
   command, install lifecycle tests, and the remaining acceptance matrix below.
   The current focused tests exercise local retry after unknown execution resolves,
   repeated passes, remote-ref retention, ignored data/cache contracts, post-merge
   commits, unfinished Tasks, missing paths and checkout/Git admission conflicts.

## Done when

Headless gate: `cargo test -p loopflow cleanup`,
`cargo test -p loopflow worktree`, `cargo test -p loopflow pr_landing`, and
`cargo clippy --all-targets -- -D warnings` pass. New cleanup tests prove the
demo, post-merge edits, ignored user data, missing providers, unavailable process
inspection, concurrent admission, missing paths, interrupted removal and repeated
passes. Scheduler tests use generated service definitions and a test invocation;
no login session or permission dialog required. DTO fixture tests accompany JSON
changes in each consumer language. Gate owns execution, not this design pass.

## Follow-ups

1. **Disposable artifacts in retained checkouts:** product-level declaration of
   disposable roots, idle/age and machine-wide byte budgets; preserve source and
   Task identity while evicting cold build and experiment output. Etude `.runs`
   belongs in this category by Jack Heart's direction; it does not require
   byte-for-byte reproducibility. Proposed behavior: protect running producers
   and consumers, expire inactive unpinned runs, and explicitly retain selected
   results. Preserve durable conclusions in source/docs rather than implicitly
   keeping every raw run forever. Replace the Python build-cleanup
   policy with this product owner; retain repository-specific test admission.
   Include persistent checkouts and interrupted-removal residue with proven
   ownership. The same policy handles `target`, `.venv` and `.runs` when declared;
   do not infer permission from a name or `.gitignore` alone.
2. **Rotate diagnostics and recovery generations:** size- and age-bounded logs,
   compress closed raw traces, expire unreferenced diagnostic segments and
   temporary captures, and retain two validated migration backups. Proposed
   diagnostics defaults: seven days hot, 30 days total, 4 GiB machine-wide;
   caps apply to disposable diagnostics, not credentials or sole conversation
   evidence. Keep writer ownership and one retention implementation; extend the
   existing backup policy rather than add a rival pruner. Ad hoc historical
   backups need explicit recovery disposition. A cap on all diagnostic roots
   prevents per-Session limits from multiplying without bound.
3. **Compact conversation storage:** lossless cold archive and on-demand reading
   for lf-owned completed Session payloads; retire duplicate captures only after
   proving their retained owner. Provider account homes contain about 10 GiB of
   transcripts and several GiB of provider databases, not just logins. Do not
   truncate native rollout files, remove SQLite/WAL files, or gzip paths a
   provider expects to read. Provider-owned histories need a tested archive and
   restore path that preserves supported resume entry points, or an explicit
   history-expiration decision. Credentials and live histories remain untouched.

## Measure

On laptop and mini: registered/retained/eligible checkout counts; estimated
allocated bytes by category; observed free-space delta after collection; oldest
eligible retention age. APFS sharing, hardlinks and concurrent writers mean
directory sums are estimates, not guaranteed reclaimed bytes.

Check: `cargo test -p loopflow --lib ops::wt::cleanup::tests -- --test-threads=1` (10 passed), `cargo test -p loopflow --test dto_fixtures cleanup_report_keeps_retention_reasons_and_unknown_sizes` (1 passed), and `cargo clippy --all-targets -- -D warnings` passed; full acceptance and installed-schedule proof remain with gate after the remaining implementation.
