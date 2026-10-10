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
instead of maintaining a parallel boolean. No known deletion targets remain.
Explicit abandonment and persistent-branch restart retain their separate authority;
missing-registration repair does not justify restoring broad metadata pruning.

## Forbidden outcomes

An hourly invocation of today's broad manual prune; age equaling abandonment;
ignored equaling disposable; a successful Flow equaling Task completion;
cleanup requiring an LLM; mandatory foreground recursive disk scans; deleting
another machine's state; moving junk to an unlimited trash directory.

## Current implementation — 2026-10-09

The shared collector in `ops/wt/cleanup.rs` serves manual prune, completed Tasks,
taskless landings and repository reconciliation before network delivery observation.
Every tick currently scans all registrations. Targeted lifecycle calls filter before
candidate observation. Both registries share checkout, delivery, Process and evidence
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
`f4cfefd0d:scratch/clean-up.md`; the latest implementation slice is `c99c18972`.
This remains an internal checkpoint, not a shipping boundary.

### Remaining in this PR

1. **Session evidence and recovery:** retained capture/manifest/legacy-row payload
   references and provider homes are protected in both registries, including the
   locked recheck. Add native reader/resume proof for historical referenced files
   and symlinked captures; audit any other historical payload owners. The
   interruption fixture covers artifact contents removed while the cache tag and
   checkout registration survive, not interruption during Git removal. Finish
   targeted missing-registration repair without broad metadata pruning. Missing
   paths still retain registration; abandoned PRs without exact-head disposition
   remain retained. No new discard authority is implied.
2. **Whole-pass timing and fairness:** the admission deadline stops starting
   candidate observations/removals, but initial registry/Git snapshots, filesystem
   normalization and started reads can overrun it. `lsof`, history SQL and size
   subprocesses are bounded; other Git/SQL reads are not. Add hourly full
   reconciliation and cheap settled-owner ticks, then oldest-deferred ordering.
   Git registration order still restarts each pass, so slow early candidates can
   starve later ones. Historical evidence scans must also make progress on stores
   larger than one observation budget without treating partial facts as complete.
   Persist scan timestamps/summary fields in bounded cron receipts. Estimated
   bytes now populate eligible background decisions only; no foreground scan.
3. **Composed acceptance:** the disposable-account fixture exercises experimental
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

Check: `cargo test -p loopflow --lib` with filters `cleanup`, `worktree_removal_keeps_dirty_files_unless_explicitly_forced`, `ops::wt::tests`, `ops::read_retry::tests`, and `unfinished_process_retains_identity_across_terminal_failure_interrupt_and_pid_reuse` passed (29 tests); `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed. Full acceptance remains with gate; loaded-scheduler/upgrade experience remains with demo.
