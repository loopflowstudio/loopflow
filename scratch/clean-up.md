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
registrations prioritize older retries within each bounded setup window. Receipts retain at most 32 pending registration
keys until attempted; later windows cannot overwrite this continuation. A fixed
lexical endpoint also bounds each registration sweep against later tail arrivals.
Hourly timestamps still bound history-independent cohort membership, not wall time.
Hints grant no deletion authority.

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

Removed authorities: old prune policies/classifiers and targeted APIs;
`prepare_landed_delete`; duplicate lifecycle deletion and CLI-only protection;
fail-open process inspection and preview metadata pruning; orphan
`engine/worktree.rs`; `Listing::pull_requests_known`; `task_open_work`;
`read_nonterminal_task_worktrees`; separate `release_blocker`; and
`worktree_remove_clean_owned`, with their exclusive fixtures. One lease-owned
Git removal serves cleanup (`Clean`) and explicit discard/release (`Force`).

Removed observation/scheduling paths: restarting
`session_events::session_evidence_paths`; per-pass registry snapshots;
`Observations` and its evidence cache; duplicate pre-admission history traversal;
`evidence_roots` and `RegistryObservations::evidence_blocker`; background batch
planning; inline hint/receipt I/O; primary-checkout retry scheduling; and the
unbounded hint loop. Earlier cuts and preservation proofs remain at
`64667a32b:scratch/clean-up.md`, this heading.

Bounded setup now joins registration branches and retry hints into
`CheckoutAttempt` records. Filtering, sorting and admission use that one bounded
candidate list, not repeated path joins or an impossible missing-hint error.
Fresh locked observation remains required; receipt continuation now replaces the
failed-write fairness cursor described in the prior implementation.
Removed the shared candidate fairness lane, background/locked aggregate listing,
`observe_registered` wrapper, candidate-membership sets and stored derived hint paths.
One observation reads each selected registration once; locked application validates
its reciprocal backlink. Saved candidate windows resume without rediscovery.
This pass also removes the repository-wide `Read::Settled` /
`settled_checkout_paths` normalizing inventory and parent-side cleanup lock-file
opens. Candidate-local positive lookups replace discovery; shared admission and
lease discovery now separate file preparation from acquisition.
Removed the nested `Read::Checkout` dispatch and its worker-side JSON round trip;
source observation calls the registration reader directly inside the same isolated
worker. Preview listing now shares the existing Git-read path. `admit_checkout`
names the prepare/open/acquire boundary without changing deadlines or lock ownership.
The setup window owns the 32-candidate cap; application no longer repeats it.
Administrative enumeration now propagates entry errors rather than treating a
partial name set as complete. No known deletion targets remain. Explicit
abandonment and persistent-branch restart retain their separate authority; missing-registration repair never
justifies broad metadata pruning. Admitted destructive removal is never canceled.

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

Git subprocess reads and independent SQLite readers retain their two-second
limits. Setup has a separate count/time admission window from candidate work.
`lf-cleanup-attempt` records discovery/last-attempt time in Git registrations;
malformed hints get oldest priority, never deletion authority. The fixed hourly
cohort cannot be extended by later hint initialization.

The shared `fairness_after` / `fairness_next` lane is removed. The existing cron
receipt now carries `pending_registrations`: at most 32 administrative paths from
one setup window. Successful reads join fresh path, branch and hint facts; candidates
use oldest-first ordering. Each admitted candidate is consumed durably before hint
publication. Failures and interruption therefore yield to the remaining members.
No new window is discovered until that continuation drains; setup resumption no longer
repeats administrative enumeration or common-directory discovery. A fixture reloads
saved candidates with Git unavailable and verifies setup resumes without removal.
This is bounded receipt continuation, not another checkout registry or
source-disposition queue.

A fixed `registration_through` endpoint prevents continuous tail arrivals from
postponing wrap and retry of an interrupted candidate. Names inserted ahead of the
cursor but below that endpoint can still extend a sweep; arbitrary adversarial
arrival rates remain unproved. An admitted setup read is consumed before I/O;
failed reads retry on the next sweep, without pinning the pending window.
The Rust-only receipt fixture includes both continuation and endpoint.

Previous count-window, scheduling-model and stalled-setup evidence remains at
`cf571fbe7:scratch/clean-up.md`. The new composed fixture starts with 65 registrations,
stalls publication for the first candidate of each 32-entry group past candidate
admission, interrupts after durable candidate consumption, then introduces three
arrivals per tick beside a one-removal cap. It checks both healthy neighbors collect,
every original candidate appears in the accumulated attempt set (including the
interrupted candidate on a later pass), and continuation stays at most 32 entries.
The set does not count repeat attempts of each failed publisher; it proves coverage,
not a per-candidate retry frequency.

### Preparatory reads

Background discovery no longer runs a repository-wide Git worktree listing or
normalizes all checkout paths first. It enumerates administrative names in an
isolated child and reads each registration's `gitdir`, HEAD and hint in its own
bounded request. A stalled gitdir is now a per-registration failure. Locked rechecks
validate the selected checkout's reciprocal Git registration, including its common
directory, instead of consulting a sibling-wide listing. Exact-head and ownership
checks still run freshly; registration observation alone never permits removal.

Manual previews retain Git's complete listing, now with Git and normalization in
one bounded read-only process group. Settlement discovery now reads only the
selected checkout's stored keys, without normalizing unrelated historical owners.
Neither operation receives admission locks or has a deletion continuation. The
Git child stays in its worker's process group rather than escaping through a nested
timeout wrapper.

The new fixtures cover collection without a working aggregate Git listing, preview
listing timeout, per-registration gitdir FIFO failure beside an observable dirty
neighbor followed by recovery, and apply-time backlink changes. The FIFO fixture
does not prove Git's eventual destructive command can finish while a sibling gitdir
remains stalled; admitted removal is deliberately uncanceled.

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

### Setup I/O isolation

Setup I/O isolation separates registration/hint reads from missing-hint initialization,
attempted-hint writes, receipt reads, receipt publication and receipt pruning.
Each request uses a two-second child deadline before checkout admission, with no
checkout locks, Git lease or source-removal operation. The existing subprocess
owner kills its process group and bounds reaping; an OS-stuck child may remain
awaiting reaping, but owns no checkout admission and cannot continue into deletion.
The internal executable entry bypasses ordinary command/store startup and explicitly
excludes inherited descriptors (including outer Flow/release/Git exclusions). This is
product I/O isolation, not another agent or cleanup owner.

The parent alone advances scheduling and admits removal after acknowledgments.
Atomic scheduling writes may commit before interruption; per-worker hint temporaries
and unique receipt IDs prevent inode reuse. A failed receipt write disables further
updates to that receipt. The next pass resumes from published progress. Scheduling facts
never grant deletion authority. Normal failed hint writes remove their temporary;
killed writes can leave small administrative temporary files. Their reclamation
in retained registrations remains with artifact follow-up, not source deletion.

Real-FIFO fixtures cover stalled hint reads, hint publication interruption,
receipt observation interruption and lost receipt-publication acknowledgments.
They check preservation, available admission after timeout, retry and healthy
neighbor collection. A blocked worker also proves that an inherited outer lock
is released when its parent closes it, without waiting for the worker. A three-second
Git removal fixture checks that setup and admission deadlines never cancel admitted
destructive removal. Receipt fixtures
now use canonical main-repository keys; earlier alias-key fixtures did not
establish resumption by the real maintenance pass. The repaired 41-failure
fixture passes with that shared key.

### Remaining in this PR

Reconciled 2026-10-09 against `6cd3c7594` and `7e1a51086`. The requested
aggregate-settlement discovery repair and preparatory isolation exist. Durable
continuation and fixed sweep endpoints remain intact. Exact-head settlement,
completed-Task delivery and fail-closed history remain mandatory. Item 2 remains
a mechanism-review boundary; neither this isolation cut nor passing tests make
the full PR ready to ship.

1. **Preparatory isolation implemented; source progress still has a review gap.**
   Minute selection now asks a candidate-local positive SQL question using stored
   checkout keys. It neither loads nor normalizes every settled owner's path.
   A lookup failure consumes that candidate's continuation slot and reports failure;
   the next registration sweep retries it. It cannot abort healthy neighbors or
   certify incomplete discovery. Hourly scans do not require the lookup. Aliased historical
   keys can miss the minute fast lane; the complete registration sweep still
   observes them. These queries never authorize removal.

   Source observation (including registry opens, Task/Process path normalization,
   cache-tag reads, reciprocal registration validation and missing-checkout lookup)
   now runs in a read-only worker. External-cwd normalization stays inside its
   existing five-second inspection budget. Source workers have a two-second
   request deadline and keep Git descendants in their own process group. The
   parent repeats source observation under admission, then performs the unchanged
   final evidence checks. A worker returns at most `validate_checkout`, never
   deletion authority. Review also found the common missing-tail resolver used
   `exists()`, swallowing access/symlink errors as absence. It now propagates those
   errors with `try_exists()`; source normalization no longer falls back to raw
   paths. Unknown resolution retains instead of becoming a negative overlap. History projection pages and receipt-context reads also
   use isolated requests; projection transactions retain their existing owner.

   Admission and Git lease owners expose discovery separately from acquisition.
   Cleanup workers prepare directories and open files, transfer **unlocked** file
   descriptors over a private Unix socket, and exit before the parent acquires
   anything. Cleanup opens every admission/lease file before taking any checkout
   lock. Machine-lock file preparation uses the same path. Lost acknowledgment
   drops queued descriptors; an interrupted worker owns no checkout lock and has
   no deletion continuation. Existing writers use the same lock paths and modes.
   Lock acquisition remains parent-owned and nonblocking; admitted artifact/Git
   removal is never canceled. Cleanup does not rewrite the lease's diagnostic
   owner text, avoiding new parent-side file writes during admission.

   New composed fixtures cover minute and hourly healthy collection beside a
   settled candidate stalled in a real FIFO-backed Git read, with an unresolvable
   unrelated historical settled path in the registry. A separate interrupted
   candidate settlement lookup proves failure-local retry. They also cover lock-file
   preparation interruption before open and after descriptor transfer, available
   admission afterward, neighbor collection and retry. Evidence is finite-workload
   isolation, not an arbitrary-arrival-rate or whole-pass timing guarantee.

   Remaining limits: administrative-name enumeration is repository-wide, although
   cancellable; worker descriptor enumeration and process launch precede the child
   deadline; reaping can add one second. Parent-owned nonblocking lock syscalls
   cannot promise an OS/filesystem wall-time bound. Large source-observation sets
   may exhaust the worker budget and retain; no partial negative observation is
   reused. In particular, `RegistryObservations::read` still resolves every Task
   checkout, and `blocker` may resolve unrelated open-Process cwd paths. An
   unresolved/stalled Task or Process alias can therefore retain every candidate
   even though the worker's failure is reported per candidate. `read_running_paths`
   also normalizes the complete external cwd set; a single resolution failure
   makes external execution unknown for every candidate. The new healthy
   collection proof covers unrelated **landing** paths and candidate-local reads,
   not that broader source-veto set. Discarding an unresolved alias would weaken
   preservation; locality needs an ownership/path contract that proves disjointness,
   not an incomplete scan labeled complete. No such replacement is selected here.
   This source-derived progress gap remains for mechanism review alongside item 2.
   The unresolved choice is how the source owner can prove candidate disjointness
   despite an unreadable alias, or supply a complete fresh view without restarting
   every unrelated lookup. Any replacement needs composed healthy-collection proofs
   beside unknown Task, registered Process and external cwd paths, while retaining
   aliases that can reach the candidate and preserving fresh retargeting vetoes.
   The current fixtures do not establish these cases; skipping unknown paths or
   increasing a fixed timeout is not a selected solution.
   Aggregate final history, its connection opens/path resolution, and native
   traversal remain item 2. Git's admitted removal may inspect siblings;
   setup isolation does not prove deletion through a permanently stalled sibling.
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

Check: recorded at `7e1a51086`: `cargo test -p loopflow --lib` with filters `cleanup_setup_` and `cleanup_apply_` — 19 passed; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — passed (unchanged code, not rerun); prose reconciliation: `git diff --check` — passed; full acceptance/provider resume: gate; installed scheduler/upgrade: demo.
