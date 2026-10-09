# Automatic worktree cleanup

Draft — 2026-10-09. Jack Heart requested investigation and a product design;
implementation, deletion and retention defaults are not approved.

> “lf is aggressive about cleaning up worktrees so that we are not causing a bunch of waste on our customers computers”

## What to build

Make the installed product automatically reclaim finished, disposable checkouts,
retry deferred cleanup, and explain retained bytes without discarding work.

## Placement

**Infrastructure**: machine resource stewardship and reliable lifecycle cleanup.
Selected from `lf wave list --json` on 2026-10-09. No Task filed or Flow launched.

## Shape and boundary

Additive series. The keystone is one cleanup owner plus automatic worktree
collection. It replaces today's overlapping deletion decisions in one PR.
Independent follow-ups cover caches in retained checkouts and other storage;
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

## Internal slices

1. **This slice:** unify eligibility and remove unsafe/unused predecessor paths;
   migrate lifecycle and manual prune consumers with protection regressions.
2. Extend the repository tick and creation/upgrade schedule wiring with bounded,
   retryable collection and truthful dry-run/reporting.
3. Prove skipped-then-idle, crash recovery, idempotence and install lifecycle;
   update worktree docs and installation references.

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

1. **Disposable caches in retained checkouts:** product-level declaration of
   regenerable roots, idle/age and machine-wide byte budgets; preserve source and
   Task identity while evicting cold artifacts. Replace the Python build-cleanup
   policy with this product owner; retain repository-specific test admission.
   Include persistent checkouts and interrupted-removal residue with proven
   ownership. Do not infer disposability from `target`, `.venv` or `.gitignore`.
2. **Other lf-owned storage:** measure and bound temporary artifacts, installation
   generations and logs at their writers. Durable transcripts and database
   history need a separately accepted retention contract. Preserve existing
   two-generation migration-backup policy rather than adding a rival pruner.

## Measure

On laptop and mini: registered/retained/eligible checkout counts; estimated
allocated bytes by category; observed free-space delta after collection; oldest
eligible retention age. APFS sharing, hardlinks and concurrent writers mean
directory sums are estimates, not guaranteed reclaimed bytes.

Check: source/call-site inspection only; no product tests or deletion run.
