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
Selected from `lf wave list --json` during the 2026-10-09 investigation, before
implementation authorization. Infrastructure owns this design; no Task binding
is established by these notes.

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

## Baseline before implementation

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
executable on upgrade and remove it through explicit repository-tick disable.
CLI-only installs get the same
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
execution. A completed Task also needs a merged active PR and resolved
follow-through; a separate settled landing cannot bypass that delivery gate.
PR-less completion preserves its checkout. Preserve primary and persistent
checkouts. Preserve uncommitted,
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

Removed: the old prune policies/classifiers and targeted APIs, their exclusive
tests, `prepare_landed_delete`, duplicate lifecycle deletion, CLI-only protection,
fail-open process inspection, preview metadata pruning, and the orphan
`engine/worktree.rs` module (its root resolver also had no callers). Explicit
abandonment and persistent-branch restart remain separate.

The predecessor's unused `Listing::pull_requests_known` flag and optional remote
PR map are gone; listing still reports remote failure and unknown PR state.
Cleanup reuses `OpenProcesses` instead of a second unfinished-process query;
the now-unused `task_open_work` history loader is removed too. No known deletion targets
remain; targeted missing-registration repair is still unimplemented, not a reason
to restore broad metadata pruning. The release-only `read_nonterminal_task_worktrees`
reader and its exclusive fixture are also removed: shared checkout links, delivery
facts and Process evidence now protect release-owned work. The separate
`release_blocker` policy and per-query asynchronous collector dispatch are gone:
both registry snapshots use one synchronous retention policy. Async lifecycle
callers dispatch the complete filesystem attempt to one blocking worker.

## Forbidden outcomes

An hourly invocation of today's broad manual prune; age equaling abandonment;
ignored equaling disposable; a successful Flow equaling Task completion;
cleanup requiring an LLM; mandatory foreground recursive disk scans; deleting
another machine's state; moving junk to an unlimited trash directory.

## Current implementation — reconciled 2026-10-09

The shared collector is in `ops/wt/cleanup.rs`. Manual prune, completed Tasks
and taskless landings use it; repository reconciliation also attempts a local
pass before network delivery observation. Each reconciliation invocation scans
every registered checkout, not just settled owners. The predecessor prune policies,
targeted-prune APIs, prompt-log pruner, automatic remote-branch deletion helper
and orphan engine create/remove helpers are removed. Explicit abandonment keeps
its separate authority. This is an internal checkpoint, not a shipping boundary.

Targeted lifecycle cleanup filters registrations before observing candidates.
Planning reads registrations once and shares Task checkout links, unfinished
Process membership and protected homes once per registry. Selected and release
stores use the same retention policy and their own Process-receipt homes; release
facts can veto removal but cannot settle experimental source. Removal refreshes
those observations under checkout admission and Git leases. Planning and removal
share one admission deadline; an already-started observation or removal finishes.
Hard subprocess/SQL deadlines remain below.

Main `906576f39` separates planning completion from delivery and follow-through.
The synced collector (`237d1a31a`, fixture correction `e1be31600`) preserves
PR-less and unresolved-delivery checkouts even when a matching merged landing
exists. This guard lives in shared observation, so manual and scheduled entry
points cannot bypass it. Task completion itself remains independent of cleanup.

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

### Latest internal slice — reconciled 2026-10-09 at `341615ba8`

Installed releases ensure repository ticks at checkout creation and work-producing
CLI entry points. Tick installation reuses the installation-gate resolver, repairs
changed service definitions and persists explicit disable under the existing cron
owner. Installation, disable and fallback admission share a lock. Without launchd,
an ordinary command starts at most one finite cron runner per minute; its target
and bounded receipts are the same as scheduled checks. Experiments do not install
services. A disabled declaration cannot run even if unloading the OS job failed.

Experimental collection now holds release checkout admission too, checks release
Task delivery and all release Processes, and reads Process receipts from the owning
store's home. The pass lock lives in the account's production home, not the selected
experimental home. Rich Task projections proved incompatible with an older release
schema in the scheduled test; stable checkout links now select only affected Tasks
before reading delivery facts. Unavailable matching facts still retain the checkout.

Registered provider homes and native provider homes are protected even inside a
cache-tagged root. Cache removal leaves its tag until Git removes the checkout,
eliminating the tagless-empty-directory interruption window. Reconciliation JSON
now includes the collector report, including deferred reasons and lock contention;
cron logs retain that JSON alongside existing bounded receipt links. The DTO has
no Swift/Python consumer; its Rust fixture is `repository_reconciliation.json`.

The headless installed-declaration test executes the real `lf task reconcile --json`
command through the cron runner: unknown execution retains a checkout, completing
that Process allows the next tick to remove it, a further tick is harmless, and an
unfinished neighboring checkout survives. The fixture seeds a taskless merged
landing and an unknown Process row, then marks the row complete; it does not launch
a live checkout user. Its neighbor is dirty, not an unfinished Task. This proves
scheduled-command retry with simulated execution, not the full Task demo or
login-session/OS scheduler loading. The integration CLI also reads the account's
release registry; `LF_HOME` alone does not isolate that boundary. Cross-store
acceptance belongs in the existing disposable-account installation harness.

The last two slices implement the earlier feedback's automatic activation,
persisted disable, gate-based executable selection, deferred JSON and shared
release retention/admission policy. They do not close the timing, historical
payload or upgrade-repair gaps below. Main remains at the already-integrated
`906576f39`; no newer local upstream change was observed in this reconciliation.

### Remaining in this PR

1. **Session evidence and recovery:** audit referenced payload owners beyond
   `.lf/logs`, `.lf/runs`, `.lf/sessions`, selected/production homes and
   registered/native provider homes. Those roots are protected, but the collector
   does not enumerate historical payload references; a cache tag must not erase
   their sole copy. Preserve or relocate them with reader/resume proof. The
   interruption fixture covers artifact contents removed while the cache tag and
   checkout registration survive, not interruption during Git removal. Finish
   targeted missing-registration repair without broad metadata pruning. Missing
   paths currently retain registration. Published abandoned PRs without exact-head
   disposition stay retained; no new discard authority is implied.
2. **Whole-pass timing and fairness:** the admission deadline stops starting
   candidate observations/removals, but initial registry/Git snapshots and started
   reads can overrun it. Only external `lsof` inspection has a five-second bound.
   Bound individual observations, add hourly full reconciliation and cheap
   settled-owner ticks, then oldest-deferred ordering. Current Git registration
   order restarts each pass; repeated slow early candidates can starve later ones.
   Background size estimates remain null. Add bounded size measurement and cron
   receipt scan timestamps/summary fields; preserve unknown sizes on deadline.
   Deferred reasons already survive in JSON/logs. No foreground recursive scan.
3. **Upgrade repair:** existing gate-based declarations follow promotion, but old
   immutable executable paths are repaired only on later work. Integrate repair
   with installation, preserving explicit disable and original schedule ownership.
   Reuse the installation gate and cron owner, not another scheduler.
4. **Composed acceptance:** cross-store Task/Process/admission fencing exists;
   prove concurrent release admission through an experimental CLI in a disposable
   OS account. The unit proof reads a second registry and tests its lock directly;
   it does not exercise `apply_cleanup` discovering that release registry. Extend
   scheduled-command coverage to a real live Process exiting, a completed Task
   with merged delivery, and an unfinished Task neighbor. Keep generated-service,
   disable, executable-repair and fallback tests. Gate owns the full headless
   matrix; demo owns loaded OS schedule/upgrade and unsupported-host experience.

## Done when

Headless gate: `cargo test -p loopflow cleanup`,
`cargo test -p loopflow worktree`, `cargo test -p loopflow pr_landing`, and
`cargo clippy --all-targets -- -D warnings` pass. New cleanup tests prove the
demo, post-merge edits, ignored user data, missing providers, unavailable process
inspection, concurrent admission, missing paths, interrupted removal and repeated
passes. Scheduler tests use generated service definitions and a test invocation;
no login session or permission dialog required. DTO fixture tests accompany JSON
changes in each consumer language; the current cleanup report is Rust CLI-only
with no Swift or Python consumer. Gate owns execution, not this design pass.

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

Check: `git diff --check` passed (prose only); prior focused cleanup (18), Task-decision (1), scheduled/DTO (3), fmt and Clippy passes retained at `341615ba8`; full acceptance and OS-schedule experience remain with gate/demo.
