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

## Implementation shape

`ops/wt/cleanup.rs` is the shared owner:

- `plan_cleanup(store, repo) -> Vec<CleanupDecision>` supplies the non-mutating
  manual preview. Targeted lifecycle attempts use `plan_selected`.
- `apply_cleanup(store, repo, plan, budget) -> CleanupReport` admits manual and
  lifecycle attempts; `apply_checkout` refreshes evidence under checkout admission
  and the Git lease before removal.
- `run_cleanup_pass(store, repo, budget) -> CleanupReport` takes the nonblocking
  machine lock and resumes bounded cron receipts. `collect_pass` observes and
  applies each candidate together, rather than materializing a background plan.

The existing repository tick runs collection independently of delivery/network
success. Minute passes select settled owners; hourly reconciliation resumes across
passes. First-work activation, upgrade executable repair, explicit disable and
unsupported-host foreground-triggered fallback are implemented; installed behavior
still needs demo. No home crawl, PM fetch, agent or new cleanup queue is involved.
Current local defaults are eight removals, 32 observations and 30 seconds admitting
new candidates; an admitted attempt finishes. Size estimates are background-only
and unknown on timeout. These are implementation choices, not accepted machine-wide
budgets or a whole-pass time guarantee. Oldest-deferred ordering remains an intended
outcome: path cursors currently prove rotation only for a stable candidate set.

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

Removed: old prune policies/classifiers and targeted APIs with their exclusive
fixtures; `prepare_landed_delete`; duplicate lifecycle deletion; CLI-only protection;
fail-open process inspection; preview metadata pruning; the orphan
`engine/worktree.rs`; `Listing::pull_requests_known` and its optional PR map;
`task_open_work`; the release-only `read_nonterminal_task_worktrees` reader and
fixture; and separate `release_blocker` policy. Shared registry observations use
`OpenProcesses`; async lifecycle callers dispatch one complete blocking attempt.

`worktree_remove_clean_owned` is also gone. One lease-owned Git removal function
serves cleanup (`Clean`) and explicit discard/release teardown (`Force`), using the
lease's path rather than a second path argument. Process receipts reuse the existing
PID/start-time classifier. Cleanup derives settlement from its exact-head evidence
instead of maintaining a parallel boolean. The background batch planner/deadline
branch, its exclusive budget fixture and the one-item batch wrapper are removed.
Manual prune and maintenance share one checkout attempt; only their callers own
admission budgets, so an admitted observation finishes its locked application.
They also share observation-error handling and one bounded persistent-branch read
for present or interrupted checkouts. Git filename output is no longer trimmed:
an ignored ` target` directory must not borrow the declaration of `target`.
A regression preserves that unclassified content through plan and apply. Cleanup
receipts retain their output root, not an unused schedule specification. Foreground previews remain non-mutating.
No known deletion targets remain. Explicit abandonment and persistent-branch
restart retain their separate authority; missing-registration repair does not
justify restoring broad metadata pruning.

## Forbidden outcomes

An hourly invocation of today's broad manual prune; age equaling abandonment;
ignored equaling disposable; a successful Flow equaling Task completion;
cleanup requiring an LLM; mandatory foreground recursive disk scans; deleting
another machine's state; moving junk to an unlimited trash directory.

## Current implementation — 2026-10-09

The shared collector in `ops/wt/cleanup.rs` serves manual prune, completed Tasks,
taskless landings and repository reconciliation before network delivery observation.
Minute ticks select settled Task/landing paths; an hourly full reconciliation
continues across passes until its cursor reaches the end. Targeted lifecycle calls
filter before candidate observation. Both registries share checkout, delivery, Process and evidence
retention policy; release facts can veto, never settle experimental source. Removal
refreshes facts under selected/release checkout admission and a Git lease. Contention
defers rather than failing the tick. The pass lock uses the account's production home.

Completed Tasks need a merged active PR and resolved follow-through even if another
landing records the same merged head. Ownership uses Task checkout links, landing
paths or new Git-administrative creation provenance; provenance alone cannot settle
source. Older release schemas use stable checkout links before affected delivery
reads; unavailable matching facts retain the checkout. Unowned legacy trees remain
report-only. Git removes without force after declared artifacts; local refs use
compare-and-delete. Hidden index changes retain checkouts, and cache tags survive
partial artifact deletion until Git removes the checkout.

Installed releases activate repository ticks on first work, resolve executables
through the installation gate, and persist explicit disable. Installation settlement
repairs existing declarations before pruning superseded binaries, preserving ownership,
cadence, activation time and disable; failed repair warns and keeps old binaries.
Without launchd, work-producing commands admit one finite fallback per minute under
the same lock as install/disable. Experiments activate neither service nor fallback.
Reconciliation JSON includes decisions and contention; bounded cron logs retain it.
Its DTOs are Rust CLI-only, with no Swift/Python consumer.

Historical evidence protection covers every captured input's published files and
retained `runs`, manifest and terminal references. The history owner projects reference
fields through existing capture/receipt indexes, without decoding replay/transcript
bodies. Symlinks resolve before classification. Both registries protect provider homes
and referenced files even inside tagged caches. Missing/invalid observations retain
candidates. History SQL has a two-second VM/row deadline and nonblocking admission;
whole-history JSON scanning exceeded that budget against release, so the reader now
selects exact receipt keys. This is not an incremental or whole-pass timing proof.
Background-only allocated-byte estimates use a same-filesystem scan bounded to one
second or the remaining admission budget; partial/failed results stay unknown.

The scheduled-command fixture proves retry after a real shell child exits, leaving
its seeded Process row unfinished. It seeds completed merged Task delivery with
resolved follow-through and an unfinished neighboring Task. `lsof` is substituted;
this is neither a native provider launch nor a loaded OS scheduler. The passing
installation-harness proof exercises experimental CLI discovery of an independent
release registry while directly holding its admission file, then releasing and
retrying. It does not launch a second release CLI writer. The interruption fixture
covers artifact deletion before Git removal, with the tag and registration surviving.

Earlier implementation details and evidence limits are preserved at
`f4cfefd0d:scratch/clean-up.md`. `c99c18972` adds historical evidence, schedule
repair and composed retry coverage; `b998379c7` consolidates lease-owned removal
and Process identity checks. The earlier iteration feedback's simulated-execution
limit is superseded by the real-child fixture, not by a native provider proof.
This remains an internal checkpoint, not a shipping boundary.

Collection now applies each admitted candidate before observing another, so a slow
observation or size estimate cannot consume the application budget of a whole
planned batch. Constant-size cursors and attempt/removal/deferred/failure counts
use the existing bounded CronReceipt writer and retention policy. The cursor is
saved before observation, including failed/slow observations. Cheap retries rotate
past the last attempted path; full scans resume in registration-path order. Each
pass admits at most 32 observations and eight removals. Filesystem-sensitive Git
status, index, ignored-content and persistent-config reads have two-second subprocess
limits. Other layout/ref reads remain unbounded. History is read lazily only after ownership,
settlement and execution checks; incomplete history still retains affected settled
candidates without replacing unrelated ownership/primary-checkout explanations.

Removal writes its exact decision inside Git's administrative registration before
touching declared artifacts. An absent checkout can be unregistered only when that
record, administrative HEAD and local ref still agree, followed by fresh delivery,
execution and evidence checks under both admissions and the Git lease. There is no
broad metadata prune. Tests cover missing unmarked neighbors, new commits after
the removal record, non-mutating preview and absent-checkout recovery through Git's
non-forced removal. The interruption is simulated between checkout deletion and
unregistration, not a killed Git child at an injected syscall boundary.

Historical coverage now follows a symlinked old capture payload, reads its retained
provider reference and resolves the native resume identifier to the original
Session after a newer capture. It does not launch a provider or prove a provider
reads the resumed transcript. Repeated-pass coverage uses a real delayed Git
subprocess, a dirty first candidate and four eligible checkouts with a one-removal
pass cap; one eligible observation also outlasts admission and still completes.
Manual batch coverage preserves the original plan while deferring checkouts beyond
the removal cap. It also proves hourly discovery, cheap-tick filtering, bounded receipts,
zero-budget deferral and rejection of partially readable history.

### Remaining in this PR — reconciled 2026-10-09

`fae554e2a` and `000811423` supersede the iteration feedback's batch-planning and
registration-order starvation findings for the tested stable set. They also supply
cheap/hourly scans, bounded receipts and narrowly marked absent-registration repair.
Those mechanisms do not need reimplementation. Safe large-history progress and
bounded observation remain the main unfinished behavior, not gate-only checks.

1. **Bound observation and preserve fairness.** Initial Git registration snapshots,
   filesystem normalization, receipt reads and non-history SQL can exhaust admission
   before a cursor advances. Layout/ref reads and the aggregate locked recheck also
   lack a bound. Bound read work without canceling an admitted destructive removal
   or skipping its fresh evidence. Prove progress with slow initial reads as well as
   slow candidates. Preserve the intended oldest-deferred behavior under arrivals;
   current path rotation supplies neither retained-age ordering nor that proof.
2. **Safe progress through large history.** The two-second reader restarts the
   whole reference projection each time; timeout retains every otherwise eligible
   candidate. It does not paginate. Settle the history owner's observation contract
   before adding incremental state: preserve raw references, identify complete
   coverage, include appended observations and revalidate current filesystem
   destinations under removal admission. A saved negative result plus SQL cursor
   is unsafe when an earlier symlink or ancestor changes between pages. A cache
   must remain a rebuildable projection, not another deletion authority. No cache
   or schema migration has been introduced. Prove eventual collection beyond one
   read budget and retention after symlink retargeting, new references, malformed
   pages and interruption; partial pages must never authorize removal.
3. **Historical evidence and composed acceptance.** Audit other historical payload
   owners and exercise the native transcript reader/provider-resume path. The old
   capture fixture preserves bytes and resolves a native identifier; its payload is
   fixture text, not a native transcript, and it launches no provider. Abandoned PRs
   without exact-head disposition remain retained; no new discard authority is implied.
   The disposable-account fixture exercises experimental
   CLI discovery of release admission, but directly holds the admission file; it
   does not launch a second release CLI writer. The scheduled fixture exercises a
   real child exiting, seeded Process identity, completed merged Task delivery and
   an unfinished Task neighbor. Native provider launch, full headless acceptance
   and regression matrix remain with gate; loaded OS schedule, promotion recovery
   and unsupported-host experience remain with demo. Installation repair has a
   headless owner test; a full published-upgrade exercise is still needed.

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

Check: `git diff --check` — passed (prose-only realign); prior 24 cleanup tests/fmt/Clippy recorded in the supplied pre-realign notes, not rerun; full acceptance: gate; loaded-scheduler/upgrade experience: demo.
