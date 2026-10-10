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
`action = ValidateCheckout | RemoveCheckout | Retain(reason)`. Preview validation
is not removal authority. Size may be unknown, never silently
zero. A report distinguishes planned, removed, deferred and failed paths.
Evidence records exact-head settlement, not merely a branch's old PR state.
Checkout decisions remain transient projections, not another authority. The single
`session_evidence_projection` draft now adds a rebuildable raw-reference index
owned by Session history; no source-disposition or cleanup Task state is stored.

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
budgets or a whole-pass time guarantee. Last-attempt hints in existing Git
registrations give older retries a priority lane; a receipt sweep prevents failed
hint writes from starving neighbors, and fixed hourly cohorts prevent
arrivals from extending discovery indefinitely. Hints grant no deletion authority.

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
admission budgets. An admitted destructive removal is never canceled.
They also share observation-error handling and one bounded persistent-branch read
for present or interrupted checkouts. Git filename output is no longer trimmed:
an ignored ` target` directory must not borrow the declaration of `target`.
A regression preserves that unclassified content through plan and apply. Cleanup
receipts retain their output root, not an unused schedule specification. Foreground previews remain non-mutating.
The restarting `session_events::session_evidence_paths` payload scan, path-only
retry/full-scan cursors and per-pass registry snapshots are now also removed.
Raw history projection and last-attempt scheduling replace them end to end.
The per-checkout `Observations` wrapper and single-use evidence cache are also gone.
Registration paths are normalized once per snapshot, borrowed during planning and
refreshed under removal admission; retry sorting no longer performs filesystem
reads or clones paths. Cheap checkout protections precede registry reads, and
known dirty/unclassified content retains without a history traversal. Planning now
stops at `ValidateCheckout`; only locked application traverses history. The duplicate
pre-admission history traversal is deleted, including from maintenance. The resolved
`evidence_roots` inventory and `RegistryObservations::evidence_blocker` wrapper are
also removed: history checks stop at a positive protection match, but still require
complete fresh evidence before allowing removal. Duplicate native homes are visited
once; transcript discovery and cleanup share their layout definition. Cleanup fixtures
reuse one exact-head settlement setup instead of replaying landing supervision.
Inline registration-hint reads and missing-hint writes, and direct cleanup receipt
filesystem calls, are replaced by separate read and scheduling subprocess requests.
Primary-checkout retry scheduling and its special worker request path are removed;
manual previews still explain primary retention. Each registration read now feeds
its explicit hint initialization directly, without a second staging inventory.
Worker replies deserialize directly to the requested type. Tests share canonical
receipt setup and Git-delay wrappers while retaining their behavioral assertions.
No known deletion targets remain. Explicit abandonment and persistent-branch
restart retain their separate authority; missing-registration repair does not
justify restoring broad metadata pruning.

## Forbidden outcomes

An hourly invocation of today's broad manual prune; age equaling abandonment;
ignored equaling disposable; a successful Flow equaling Task completion;
cleanup requiring an LLM; mandatory foreground recursive disk scans; deleting
another machine's state; moving junk to an unlimited trash directory.

## Current implementation — 2026-10-09

The shared collector serves manual prune, completed Tasks, taskless landings and
repository reconciliation before network delivery observation. Exact-head settlement,
completed-Task delivery/follow-through, ownership, execution, ignored-content and
Session-evidence checks remain mandatory under checkout admission and the Git lease.
Release facts veto; they never settle experimental source. Checkout removal remains
non-forced, followed by compare-and-delete of the observed local ref. Missing paths
need the collector's exact administrative removal record; no broad metadata prune.

Installed releases activate repository ticks on first work, resolve executables
through installation admission, persist disable and repair schedule declarations
before retiring old binaries. Failed repair keeps old binaries. Unsupported hosts
admit a finite foreground fallback at most once per minute; experiments do not.
Earlier implementation, schedule/interruption evidence and its limits are retained
at `a7b2cc858:scratch/clean-up.md`, including the real-child scheduled-command fixture
and the independent release-registry admission fixture. Neither was a loaded OS
scheduler or second release CLI writer.

### Observation and retry scheduling

Cleanup's registration, layout, ref, status, index and ignored-content subprocess
reads now have two-second limits. Independent read-only SQLite connections give
non-history registry/discovery observations a two-second VM deadline and zero busy
wait without interrupting normal writers. Each candidate receives a fresh reader;
manual previews and locked rechecks use the same owner. The admission window begins
after setup, so a slow successful registration snapshot no longer spends the whole
window before the first attempt. A real delayed Git fixture proves this and a hung
registration command returns an error without deleting anything.

Path-only rotation is removed. `lf-cleanup-attempt` in each existing Git registration
records discovery/last-attempt time, atomically replaced before observation. Malformed
hints receive oldest priority and are replaced; hints never establish ownership.
Retries interleave oldest unattempted/last-deferred priority with a receipt-owned
lexical sweep (`fairness_after`). The sweep is persisted before hint I/O and prevents
failed writes from pinning every observation slot. It does not establish ownership
or replace the timestamp lane that protects old deferrals against arrivals. The hourly cohort
has a fixed start timestamp; settled retries remain eligible while discovery is in
progress. Tests introduce three eligible arrivals per tick with a one-removal cap
and prove previously deferred checkouts go first. Receipts retain constant-size
cohort coverage, sweep position and counts, with an updated Rust-only DTO fixture.
A composed fixture interrupts after durable admission, then introduces three arrivals
per tick beside 41 persistently unwritable hints. Healthy collection progresses,
every failed candidate is retried, and repairing one hint allows its collection.
This proves failures beyond the 32-observation window without exhausting the time
window; stalled I/O and arbitrary arrival rates are not covered. Directory-entry and
type errors now isolate the affected registration rather than aborting discovery.
Previews do not write scheduling hints.

### History observation contract

The history owner replaces the restarting payload scan with one draft migration:
`session_evidence_source` defines the raw projection; `session_evidence` stores it;
`session_evidence_backfill` records transactional coverage of a fixed historical
high-water mark. Maintenance projects at most 256 historical rows per page under a
two-second SQL deadline. Source insert/update/delete triggers maintain appended and
changed references atomically, including observations of older captures. Arrivals
cannot extend the backfill cohort. No negative deletion decision or canonicalized
destination is stored. An interrupted page rolls back with its cursor; incomplete
coverage or malformed references never authorize removal. The index can be rebuilt
from Session events. Previews neither read the complete projection nor advance it.

Only complete coverage is read, in one SQLite snapshot. Raw destinations are resolved
again in the locked removal observation, so earlier symlink/ancestor changes cannot
hide behind a page cursor. The released-frontier migration fixture exercises multiple
pages, heavier arrivals, an aborted page, an earlier-reference update, appended
terminal evidence and malformed data. The composed collector now crosses three
pages with 600 arrivals per tick and an aborted second page, verifies rollback to
256 rows, preserves a retargeted historical symlink during apply and eventually
removes an unrelated settled checkout. This proves progress beyond the row-page budget, not
arbitrarily large final reference sets or an end-to-end wall-time ceiling.

Historical-owner inspection covered capture manifests (context/runtime files), legacy
`runs` paths, terminal `result_ref`, published capture payloads and provider identity
receipts. `provider-session.json` is now included among published payload paths.
Client/attempt/conversation observations retain their values in Session history;
their source/diff path strings do not themselves make source disposable. Native
account homes required an additional fix: a transcript symlink below an external
home can reach into a declared cache. The native-history owner now observes those
symlinks in the supported Codex/Claude layouts with a cooperative deadline. Protection
also covers checkouts nested inside protected roots. A new fixture preserves native
Codex rollout bytes through cleanup, then calls `provider_conversation::admit` with
an unrecorded ID, forcing actual transcript discovery and cwd reading. No provider is
launched; this is not a provider-resume proof.

### Remaining in this PR

Reconciled 2026-10-09 against `65018af2e` and `c62a28b02`: failed-hint
fairness, cheap previews and composed backfill coverage now exist. They do not
resolve the final-observation counterexample below. The locally available
`origin/main` has no commits absent from this branch; no remote refresh is claimed.

1. **Finish bounded observation and failure isolation.** Independent setup I/O
   isolation now separates registration/hint reads from missing-hint initialization,
   attempted-hint writes, receipt reads, receipt publication and receipt pruning.
   Each request runs in a two-second subprocess before checkout admission, with no
   checkout locks, Git lease or source-removal operation. The existing subprocess
   owner kills its process group and bounds reaping; an OS-stuck child may remain
   awaiting reaping, but owns no checkout admission and cannot continue into deletion.
   The internal executable entry bypasses ordinary command/store startup and explicitly
   excludes inherited descriptors (including outer Flow/release/Git exclusions). This is
   product I/O isolation, not another agent or cleanup owner.

   The parent alone advances scheduling and admits removal after acknowledgments.
   Atomic scheduling writes may commit before interruption; per-worker hint temporaries
   and unique receipt IDs prevent inode reuse, and a failed receipt write disables further updates
   to that receipt. The next pass resumes from published progress. Scheduling facts
   never grant deletion authority. Normal failed hint writes remove their temporary;
   killed writes can leave small administrative temporary files. Their reclamation
   in retained registrations remains with artifact follow-up, not source deletion.

   Real-FIFO fixtures cover stalled hint reads, hint publication interruption,
   receipt observation interruption and lost receipt-publication acknowledgments.
   They check preservation, available admission after timeout, retry and healthy
   neighbor collection. A blocked worker also proves that an inherited outer lock
   is released when its parent closes it, without waiting for the worker. A three-second Git removal fixture checks that setup and
   admission deadlines never cancel admitted destructive removal. Receipt fixtures
   now use canonical main-repository keys; earlier alias-key fixtures did not
   establish resumption by the real maintenance pass. The repaired 41-failure
   fixture passes with that shared key.

   **Remaining:** global lock-file creation, initial registration normalization,
   registry filesystem opens/normalization, checkout/Git lease discovery and aggregate
   locked observation still run inline. Setup now bounds individual hint/receipt
   requests, not their cardinality or total duration; directory enumeration failure
   can defer the repository. Kernel-uninterruptible filesystem calls are not simulated
   by the FIFO tests. The 41-failure fairness proof still does not establish a whole-pass
   time bound. No canceled worker receives locks or a continuation to apply cleanup.
   Further lock preparation must preserve that separation; destructive removal itself
   must never be wrapped in the cancellable worker. Item 2 remains the review boundary
   for any dependent final-history mechanism.
2. **Revise final evidence observation before dependent implementation.** The
   opt-in `cleanup_evidence_cost_probe` measures already-complete raw projection,
   fresh path resolution and native traversal separately—no backfill is timed.
   At 1,024 rows: 8,192 paths, 23 ms raw read and 67 ms resolution; at 8,192 rows:
   65,536 paths, 228 ms and 519 ms. At 65,536 rows the raw read hit its two-second
   deadline. Native traversal of 1,024 / 8,192 / 65,536 ordinary files took roughly
   0.5 / 3.6 / 33 ms. These synthetic debug-build samples overlapped compilation;
   they show a real final-read failure, not a universal cardinality threshold.
   Native symlink-heavy layouts and stalled filesystems remain unmeasured.

   **Architectural counterexample:** after complete backfill, an unchanged reference
   set that cannot finish within the final-read deadline restarts and retains on
   every tick. Paging raw history does not resolve this. Persisting partial negative
   filesystem observations would violate fresh validation: an earlier symlink may
   retarget before removal. Raising a fixed timeout only moves the counterexample.
   Dependent evidence progress implementation stops at this mechanism-review boundary.
   The replacement needs either a workload-sized, bounded observation contract with isolated I/O,
   or a history-owner mechanism that supplies a complete fresh view without a full
   traversal. No cached negative result, relaxed preservation rule or increased
   timeout has been adopted as a substitute.

   `c62a28b02` removes duplicate home traversal and returns on a positive protection
   match. It does not stream the raw projection: `session_evidence_paths` still
   constructs the complete deduplicated path set before checkout overlap checks.
   Thus the measured raw-read failure still prevents unrelated collection. The
   native deadline is cooperative per home, not a bound on stalled filesystem calls
   or aggregate observation across homes and registries.

   **Unresolved mechanism choice, not a new retention decision:** workload-sized
   isolated observation must specify how a healthy finite set eventually completes
   while a stalled path does not monopolize maintenance. A history-owner fresh view
   must also cover external native writers and symlink changes; the SQL projection
   alone does not. Either design needs a composed proof with already-complete
   history exceeding today's final budget, fresh retargeting/appended-reference
   vetoes, interrupted observation, and eventual unrelated collection. Symlink-heavy
   native layouts need their own measurement/proof. Raising a constant timeout or
   accumulating negative path results does not meet this requirement.

   The independent preview conflict is resolved: `validate_checkout` reports exact
   source settlement with deferred history validation. Planning performs no recursive
   native-history traversal and does not read the full reference set. Application
   repeats cheap facts and validates history freshly under admission; unreadable or
   incomplete history retains. Native symlink and unreadable-layout fixtures cover
   preview-to-apply preservation. Eventual collection beyond final observation
   budgets remains unproved and is still required before shipping.
3. **Composed acceptance.** Native provider launch/resume, a second release CLI
   writer, full headless acceptance and regression matrix remain with gate. The
   current fixture reads a native transcript but does not launch its provider.
   Appended/updated references and interrupted pages have projection-level proofs;
   the collector's multi-page fixture now composes heavier arrivals, transactional
   interruption, symlink retargeting and eventual unrelated collection. These do
   not establish progress beyond the final-read deadline.
   Loaded OS schedule, promotion recovery, unsupported-host experience and a full
   published-upgrade exercise remain with demo. Abandoned PRs without exact-head
   disposition remain retained; no new discard authority is implied.

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

Check: `cargo test -p loopflow --lib cleanup_setup_` — 7 passed; focused failed-hint fairness, oldest-deferral, slow-candidate, slow-registration and receipt tests — 5 passed; real CLI I/O protocol smoke, `cargo build -p loopflow --bin lf`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` — passed; full acceptance/provider resume: gate; installed scheduler/upgrade: demo.
