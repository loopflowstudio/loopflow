# infrastructure wave memory

## Machine terminology (LOO-394, 2026-10-07)

Jack Heart authorized the rename, then two changes to PR #1484 and republication
only: discard old Desktop caches and name the installation scope `installation`.
A machine is one OS user and data directory; `LF_HOME` retains its filesystem
meaning. Opaque `home_…` IDs, cron plist keys and historical payloads survive.
Cron reads released JSON keys. Desktop discards version-1 caches; this supersedes
the earlier translation decision. Installation owns artifact updates and skill
exports; short commands stay. Retain `~/.lf-machine/install` and the promotion
lock path because released gates, receipts and jobs pin them; no relocation is
proved. Retained binaries use the shared `install` shorthand. Exact-frontier
fixtures seed released SQL without drafts. Local checks prove neither installed
migration nor remote continuity. Schedule ownership and activation stay intact. Release's
operation-entry lesson also applies to command removal: reject the retired
option beneath a valid current owner; an unknown owner can produce a false pass.
Corrected installation fixtures still require disposable OS-account isolation.

## Task decisions and delivered work (LOO-408, 2026-10-07)

Jack Heart authorized autonomous repair, verification and landing, with verified
merges completing Tasks by default. Explicit remaining work carries its outcome,
evidence condition and next check; overdue calls for a decision, never invented
success. Task state changes must preserve Session turns, reservations, process
outcomes, ancestry and live controls. Historical uncertainty can retain a checkout,
but cannot veto an authorized completion or cancellation. The old per-Exec
acceptance ritual is superseded; its recorded decisions remain history.

Installed 0.13.9 reproduces LOO-353's five pending turns, one reserved input and
two unknown processes despite its merged PR's completion intent. Source fixtures
prove preservation. PR #1483's Process/LFID vocabulary and migration are integrated
without another draft; installed acceptance awaits published repair. Current
Infrastructure reads retain LOO-285's unattended settlement proof, LOO-304's
performance/soak acceptance, LOO-375's installed timing and LOO-390's prevention
measurement. Their merge status alone proves none of those outcomes. Legacy
keep-open requests without explicit remaining work need scope reconciliation.

## Process vocabulary (LOO-400, 2026-10-07)

Jack Heart approved Process, LFID/PID and PR #1483 landing
(comment `32c00054-4c60-4b96-bdf4-4d5f142ab881`).
`process.lfid` is durable identity; optional `pid` is a reusable Unix PID.
References use `process_lfid`/`parent_process_lfid`. One draft preserves IDs,
parents, outcomes and unknown PIDs; LOO-397 owns command placement.
SQL renames preserve JSON, captures, receipts and provider environments
(boot-witness loss; Release's landing incident). Sequencer receipts retain
`process_id` for Rust's `process_lfid`; round-trip tested.
Fixtures prove neither installation nor control authority.

## Terminal conversation recovery (LOO-409, 2026-10-07)

Jack Heart authorized autonomous repair through landing. Installed v0.13.9
revision `6448e3c9e7` recorded reservation and spawn-request receipts for LOO-386's
first input `ca3b3cf9be254e118f314fef934cd2e9`; the later two admission errors
were retries. Its TUI returned successfully without native history. The terminal
path discarded its temporary client receipt without retaining a provider process.
This contradicts the proposed initial-generation mismatch.

Record native provider identity at spawn and observed exit at wait; failed spawn
is positive non-start evidence, not an engine exit. Commit reservation with the
driver claim. Native fallback must use the same ownership checks as first launch;
remote clients cannot stand in for their surviving engine. Fast terminal exit
must preserve successfully published native history between opener probes.
LOO-408 owns completion, LOO-324 native/account discovery, LOO-400 terminology.
CLI/PTY proofs pass; installed reconnect still fails. No migration. The legacy
Session lacks exact process evidence; driver success proves no provider exit.

## Storage footprint (LOO-390, 2026-10-06)

Jack Heart authorized autonomous investigation, cleanup and delivery. Findings,
measurements and prioritized proposals: [storage footprint review](../../docs/reviews/storage-footprint.md).
`session_events` was 92% of a 2.6 GiB store growing about 0.5 GiB a day since the
October 2 capture cutover: every streamed token was two rows, and the same
events also sit in `runs/*/events.jsonl`. Source now coalesces increments before
SQLite, validates an exact-frontier store in place during install preflight
(full copies killed by the 30 s currency probe stranded 11.7 GiB in `$TMPDIR`;
#1465's pinned snapshot remains for pending migrations),
limits the WAL to 64 MiB, and keeps two fingerprinted migration backups.
27.9 GiB was reclaimed: stranded preflight copies and Cargo output of two merged
idle worktrees. Installed effect awaits a release and is unobserved. Existing
rows are not rewritten. Jack's 78.7 GiB recording and 81-copy leads were gone
before measurement. Undecided, and left intact: which of SQLite or `events.jsonl`
is the system of record, capture retention, binary/artifact pruning, and about
23 GiB of legacy `traces`, `backups`, `lfd.db*` and `logs` with no reader.
Unknown ownership is not permission to delete. Test `session_record` with `LF_*` cleared.

## Retained capture storage and autonomous cleanup (2026-10-05)

Jack Heart delegated LOO-370's autonomous source delivery. Infrastructure selected
one opaque `~/.lf/runs` root, superseding physical relocation; the Task brief and
comment `3829b49e-2ade-4bee-8850-2ae2297399a8` record that decision. PR #1450
(`1af81fe03`) is integrated. Session identity controls conversations; capture keys
select history. Preservation constraints and contrary evidence remain under
Capture cutover below. This decision authorizes neither installed conversion nor
live interruption. Earlier detail: `6448e3c9e:wave/infrastructure/MEMORY.md`.

## Project configuration and review direction (2026-10-05)

Jack Heart resolved LOO-366's shared local Project binding and one-time cached-name
cutover on October 5; the accepted constraints and evidence remain under
[Optional chapters and Task workflows](#optional-chapters-and-task-workflows-2026-10-02).
Both decisions were delivered to LOO-366, and its saved Flow resumed. Source
implementation and configured acceptance remain unfinished; neither approval
establishes readiness. Earlier detail remains at
`c4373492cfc0c77fc27a7887045f74c48b381e72:wave/infrastructure/MEMORY.md`.

The Infrastructure Project recommends `code`; KRs/targets and reviews remain.
Accepted review and v0.13.3 installation: `57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:wave/infrastructure/MEMORY.md`.

LOO-326 completed on October 6 through installed v0.13.5's exact historical-Exec
acceptance; its unknown outcome and checkout remain intact. Exact evidence is
retained under Transient recovery. LOO-370 also completed under the opaque-root
decision, without a physical capture migration.
LOO-367's installed retry recorded a recovery boot witness (Session event 819671)
but stopped again on the same boot. A later authorized restart can establish
old-provider death; no restart or successful Flow continuation is claimed.
Preserve its conversation and saved Flow rather than replaying unchanged evidence.

## Release follow-through (reconciled 2026-10-05)

Jack Heart directed Infrastructure to take responsibility for finishing every
release. Release owns the release-specific execution and evidence; Infrastructure
retains responsibility through verified publication and installed acceptance.
A merged bump or queued build is not completion.
Resume interrupted releases through the supported release operation, preserve
exact publication evidence, and surface any unresolved blocker and next action
without requiring Jack to rediscover the gap. Keep progress updates in the
ongoing conversation while recovery is active.

Release memory records v0.13.5 publication and installation on October 6,
including historical-Exec acceptance, boot-witness recovery and PR-base repair.
The normal installed release controller completed publication, then installation
recognized the existing database without migration. Manual release success
establishes neither a scheduled receipt nor two unattended settlements. Exact
release evidence belongs in [Release memory](release/MEMORY.md).

## Capture cutover (LOO-370, reconciled 2026-10-05)

Jack Heart authorized autonomous source delivery, not installed conversion or
interruption. Infrastructure selected one opaque `runs/` physical capture root
under that delegation; the updated brief and delivered steers explicitly accept
it. This changes the prior layout-replacement requirement. No relocation,
parallel layout, compatibility alias or privileged offline migration is required.

Capture keys select history; Session IDs select mutations; Session/Exec provenance
supplies authority. Missing payload cannot erase resumable SQLite identity.
Keep historical paths, payloads, feedback, native identity and usage unchanged.
Checksum-pinned v0.13.3 capture preservation passed with stub providers, including
native resume, independent replay, review feedback and nested Exec ancestry.
Those fixtures prove neither installation nor conversion; #1450's source merge
does not establish either installed outcome.

The abandoned permission probes demonstrated writable hard-link aliases and a
recovery check/write race that altered unrelated replacement metadata. Those
counterexamples and the unresolved Mac boot-custody investigation remain at
`6fcdbe9da0b47ef95f1f92ebdb259401a327cd46:scratch/finish-removing-the-retired-run.md`.
Deleting the probes does not establish exclusion or migration. Installed acceptance
remains separate from source fixtures and delivery.

## Review replacement (2026-10-04)

Jack Heart authorized LOO-377's autonomous repair; PR #1429 merged as c5dc238b0afb
with hosted CI passing. Restart serializes review launch, fences writers and
retains exact stop evidence for retry. Independent reviews remain protected.
LOO-370's replacement Flow finished after recovery; its Task completed October 6
under the opaque-root decision above.
Installed acceptance remains unproved;
LOO-373 owns the retained landing-placement reconciliation error.

A review's service, Session driver and provider are distinct process owners.
Record the service's exact Exec before child launch. Acquire the existing launch
lock before the driver fence: reservation/spawn must settle before collecting
owners, and reobserve the same Flow afterward. Retirement must fence review
writers and retain captures/native history without recording successful review.
Save exact process identities before signaling: native stop removes client receipts,
so an interruption before replacement otherwise destroys retry evidence. The
public restart fixture reproduces that boundary with no Session provider PID;
retry uses saved identities. Unknown ownership and independent reviews still block.

## Transient recovery (LOO-326, 2026-10-04)

Jack Heart approved existing Tasks using valid cached planning regardless of age;
known invalidation/removal/terminal state/ownership mismatch still blocks. Apply
that policy at restart, continuation and each managed worker boundary. New advice
still requires successful Linear publication before worker replacement. Preserve
observation age; never rewrite a stale response as freshly acquired evidence.

Read retries belong only around the failing provider read. Artifact attempts use
separate temporary directories and keep candidate identity fixed. Bound pipe
collection as well as process exit: a descendant can retain stdout after its
parent exits. Never retry publisher writes or turn missing checks into success.
The Swift cleanup finding stays in release memory (shipped in v0.12.24).
PR #1413 merged these as acd6654f9 after hosted CI passed, including Swift under
combined headless and external-network restrictions. Merge is not installation.

Jack Heart accepted the historical uncertainty of read-only Exec
`5f239ead-89f9-49c4-92c4-4c2f8b97ca94` rather than blocking delivered work.
PRs #1413/#1435/#1445/#1455 merged and shipped through v0.13.5. On October 6,
`lf task complete LOO-326 --accept-unknown-exec` with that exact ID succeeded;
fresh Task status confirmed `done` and current planning writeback. The command
retained the unknown outcome and checkout. Acceptance grants neither process
control nor cleanup authority. Missing receipts and unattributed terminal trace
events still cannot establish exit; Session closure does not prove child exit.

Branch prevention now keys process receipts by Exec ID and removes them only
after a successful terminal write. Interrupt cleanup follows the same rule;
pruning requires exact death plus a matching persisted terminal record. Failed
writes, PID reuse and pruning must preserve unfinished identity without inventing
an outcome. These loss paths are not proved causes of the original incident.
Release's October 4 retained-landing evidence remains the counterexample: its
receipt proved death while separate leases established re-entry authority.
The receipt-preservation repair cannot reconstruct the missing identity.
Hosted CI passed before delivery; completion uses explicit accepted uncertainty,
not a fabricated terminal receipt. Earlier interrupted local gate evidence remains
in git history at `fc60c17c2a931f121d6672fea5c037352cc0e9b1`.

Release's October 5 recovery shows retries cannot fix a deadline that kills
healthy transfers: 76 MB near 200 KiB/s exceeded five minutes but finished within
fifteen. Jack Heart also selected preserving the pending release version during
recovery, superseding automatic patch successors; published tags, artifacts and
migration bytes stay immutable. Details and acceptance limits belong in Release memory.

PR #1435 CI at `6c760f285` exposed a release cleanup race: a terminal repair
Exec receipt can precede closure of inherited checkout descriptors. A delayed
launcher reproduces the retained second checkout; cleanup and repair re-entry now
wait up to five seconds for an independently acquired lease and preserve ownership
if it stays held.
The existing cleanup assertion stays intact; this repair shipped with #1435.

Syncing a Task's own remote branch must not make its remote tip the PR's base.
That mistake stranded PR #1456 despite passing CI. PR #1459, installed in
v0.13.5, repaired the retained base using exact remote-tracking reflog evidence;
PR #1456 merged October 6 at 13:53 UTC without rewriting feature history.
Foreign sibling ancestry and missing evidence still refuse.

## Scheduled release accounting (LOO-285, source reconciliation October 2)

Jack Heart retained one execution per wake for frozen missed dues. Completion
still requires two adjacent original configured dues, two automatic executions,
at least one artifact publication, required verification and no manual repair.
Collapsed misses supply accounting coverage, never additional settlements.

At `fe36937fa`, main's Session/Exec/Home/Flow and release recovery are integrated.
Exact saved-candidate inspection permits replacement only with affirmative
unpublished evidence, preserving rejected candidate/proof and original due owner.
Unknown or partial publication blocks replacement. CI-repair children now retain
release target and checkout ownership. Reported focused passes and merged source
are not a full affected gate, configured publication or Task completion.

Separate durable facts: physical cron exit, product settlement, publisher stages,
public artifact proof and dated repair ownership. Assigning a closed opportunity
an owner does not resume it. Same-Home continuation is implemented locally;
old-Home authority is never transferred by attribution. Child-held mutation locks
and exact checkout leases protect different scopes; cleanup must independently
reacquire after dropping the parent's shared handle. Parent death and elapsed
wait grant neither mutation nor deletion authority. Main-reset/stash helpers can
replace a held lock inode; explicit source selection avoids that failure class.

Same-Home coverage, overlap, retry timing and interruption proofs remain at
`c418953634bd101f51878d2be2b40fb3facafabd:wave/infrastructure/MEMORY.md`
and its referenced commits `a60ac0281`, `02d6b3c00`, `95643bd50`, `d60d254ef`.
They retain original ownership, physical failure, frozen coverage, candidate,
caller bytes and child-held locks without republishing or double settlement.
Synthetic proofs do not establish configured automatic settlements.

PR #1457 merged installer isolation: candidate preflight and public installer
smoke use disposable Linux containers, checking selected CLI bytes instead of
the entry gate. Native macOS smoke stays separate. Prior simulated checks prove
neither container/public acceptance nor automatic settlement.

October 6 scheduled receipt cron_a4b8b11b2a534bf99d183e677f2a6871 failed after
recovery cron_5d930c31c7ae4a118f6b93774496991c passed continuity but the scorecard
emitted retired product/task-loop-trust. Accepted chapter commit 42451654e removed
that contract; remove its hardcoded producer output, preserving strict metric
validation and lifecycle rows. Installed CLI with the repaired checkout reports
35 rows; twelve focused tests pass. Original failed receipts and repair ownership
remain unchanged. Jack Heart authorized publication; the operator owns subsequent
release recovery after LOO-382. This manual proof supplies no automatic settlement.

Release's September 28 incident proves entry points need operation-level recovery:
an agent reported failure successfully, producing a misleading green cron receipt.
Jack's later steer records v0.12.24 publication/install and skill-to-Flow activation
at unchanged 10:00. That supersedes the child's dated pending-activation evidence,
without proving this accounting branch is installed or either qualifying outcome.
Release-specific detail remains in [release memory](release/MEMORY.md).
October 5 child evidence records repaired installed jobs using the machine gate
at unchanged 09:00/10:00 schedules. Ownership remains Infrastructure; the child
is not registered. History exposed three unresolved opportunities with unknown
timezone provenance, zero executions and no qualifying pair. Schedule repair
cannot backfill that evidence or count as an unattended settlement.

All 36 telemetry failures, including the original 35, remain dated counterevidence
from September 24. The missing `agent_turns` diagnosis is historical: integrated
scorecard source consumes SessionHistory. The isolated doctor/scorecard Flow
regression proves Recovery cannot replace natural Scheduled evidence. No installed
pass or accepted Intelligence handoff is established; reproduce current failure
before commissioning duplicate analytics repair. Required headless Desktop/public proof and
actual automatic settlement observations remain outstanding. LOO-357 retired
the UI-host prerequisite; PR #1441 removed its scheduled validator requirement. The working plan
owns remaining implementation; no production release, install, schedule change,
Home transfer or review completion is authorized by local reconciliation.


On October 4, Jack Heart waived the interactive demo and authorized landing
PR #1419. This supersedes the earlier review requirement for this delivery;
installed acceptance and the two automatic settlement observations remain
unproved. The branch’s prior memory curation remains in Git history at
`e2eccdd257f0f3e2cf34db7583f7309eb0084394:wave/infrastructure/MEMORY.md`.

## Operator acceptance and account direction (2026-10-04)

Jack Heart directed the Wave to finish LOO-295 and LOO-342 using actual machine
usage rather than another staged acceptance exercise. Both now read `done`
through `lf task status`. LOO-367's retained feature invocation completed design
review, traversed four loop returns and reached demo; LOO-285 independently
reached demo after four returns. Together with LOO-295's reconciled merged
PR #1283, this closes its remaining continuation proof. Its unrelated viewer and
earlier assessment survive at `~/.lf-retired/20261004-loo295/scratch`.

LOO-342's installed routing checks and ongoing main-Home usage establish its
outcome. Jack explicitly declined more work on retired Ask tooling. Four old
stores still had legacy process handles at that read; their later authorized
retirement is recorded below. Cleanup no longer blocks this Task's completion.

LOO-324 now follows stop-bundling's shared/isolated model. Jack clarified that
cross-binary history means discovering native Session records across folders,
not duplicating transcripts or reconstructing new conversations from replayed
context. Reuse native history and retain Loopflow Session identity. A usage
threshold such as 90% redirects future launches only; actual exhaustion triggers
automatic shared failover. Isolated agents retain their own account/fallback.
The updated Linear brief owns the current scope; stop-bundling's branch evidence
does not yet establish shipment.

Renamed from `systems` on 2026-07-08; current schedules supersede historical ones.

The 2026-09-30 [LOO-298 decisions](#data-model-and-performance-decisions-reconciled-2026-09-30)
supersede older Run-owner, historical-import, pinned-development-Home and
demo-before-landing directions for this cutover. Earlier incident observations
remain evidence of their own versions, not instructions to restore those owners.

## Installed worker recovery (curated 2026-10-07)

October 2 recovery evidence remains at
`6448e3c9e7e519585378feaffe04606bc1b55d3e:wave/infrastructure/MEMORY.md`
under this heading. Published v0.12.32 preserved the Home without migration and
resumed LOO-367. LOO-295 and LOO-292's later acceptance above supersedes their
then-open review obligations. LOO-373 owns the retained landing-placement defect;
re-entry preserved records and checkouts. Manual releases prove no automatic settlements. The operator's batching proposal was not Jack Heart's
approval of a new release policy; source, installation and acceptance stay distinct.

## Optional chapters and Task workflows (2026-10-02)

Jack Heart's October 2 direction separates ordinary work from optional chapters:
LOO-366 owns Project ensure and coordinated resets; LOO-367 owns Task admission
and completion. Projects need no chapter or default Flow; Tasks need no managed
Flow or delivery workflow merely to exist and record an outcome. Preserve
unfinished work and unresolved delivery. Reads never provision Projects. This
supersedes mandatory-chapter assumptions, without proving either KR or authorizing
a repository reset.

Jack Heart's October 4 reviewed direction selects an explicit Wave Project
configuration field and exact-ID ensure; no name matching or candidate selection.
Chapter creation requires planned KRs, with new Task admission afterward.
Unreviewed backlog survives rotation until explicit disposition. These decisions
supersede the earlier status-selection and automatic-expiration proposals.

Jack Heart selected one shared local binding across checkouts on October 5
(`f092d63a-a152-4920-af81-d676a576f694`). October 6's SQLite decision below
supersedes its file layout; stale files and settled receipts never select a Project.
The earlier review remains at
`e04c83513573cc09883fb2b92ebdb63e06a22c95:scratch/keep-every-wave-ready-for.md`.

October 5 source permits empty Flow and preserves names. Jack Heart's comment
`5419b87c-bfec-4f42-8914-021483249895` authorizes one historical name-only correction,
retaining original evidence and strict subsequent conflicts. Atomic ingestion and
projection retain entity ages; accepted Initiative ownership rejects stale
full/partial Wave association. Cold detail resolves configured ownership, with
both durable-identity shortcuts deleted. Shared SQL ownership and focused tests
preserve foreign/unmapped plans and legitimate same-Wave Task/PR identity.
Replay/generic writers are deleted; restart retains accepted facts. Rotation now
accepts confirmed readbacks. Exact source/evidence remains at `d4d77d8e22f4244ad83027ba9c85bb8644a622e6`.
Reteam accepts full issue and exact Team readbacks, preserving Initiative ownership
and independently newer facts. Identifier-only writers are deleted.

October 5: queued SQLite workers own Wave guards through commit, including after
caller cancellation; one shared writer preserves that lifetime. Multi-Wave
operations lock in stable ID order and reuse held guards. Cold detail re-reads
ownership under the lock; delayed absence compares the queried revision/age.
Inspection follows UUID across renames. Cancellation proofs exclude competing
reteam preview until commit; separate apply fixtures cover cached recovery and
expansion/move/narrowing response loss. These supersede the unrepaired-reteam
finding; the registration and rotation proofs below establish later boundaries,
not installed acceptance.

Exact-ID rotation supersedes the name-selector failure retained at
`42e6c2706b1:scratch/keep-every-wave-ready-for.md`; cross-process recovery remains
unproved. Desktop activation
must preserve #1447's cached plans and independent Session reads, outside polling.
#1446's ongoing procedures retain started-Task follow-through; reads never provision.

Input replacement exposed two admission gaps on October 5: generic `cwd` updates
removed unbound Task membership, and a Session elsewhere could replace input while
its bound Task checkout was excluded. Source retains the stored workspace and locks
its Flow workspace and bound Task root too. Exclusion/retry regressions pass. CI
repair acquires its explicit Task root at its outer boundary, without reacquiring
inside reservation; its operation-entry proof remains open. Task-bound Flow cwd
already derives from the Task, so claims and review reservation need no second
path owner. The later rotation proof below establishes exclusion and failed-reset
retry; configured readiness remains unproved.

Collection preserves issue-reported ownership; listing Project IDs cannot override
it. October 5’s failing loopback regression prompted removal; installed acceptance
remains unproved.

Jack Heart's October 5 comment `e4dafef5-2a87-4359-818a-3770356ba850` requests
autonomous Intelligence repair without another review Session. The reported
Backlog/empty-Flow Project is `999bdbdd-c045-41a6-8ffc-a97c4a40b0b3`; preserve its
KRs, Tasks and identity, with no competing creation or repository rotation.
This explicit ID supplies bootstrap selection, not a candidate heuristic.
The active-Project adoption test proves no Backlog activation or installed repair.

Registration now takes the Wave guard before Project resolution and retains it
through either SQLite registration commit, including caller cancellation, then
releases it before checkout setup and execution. Admission uses exclusive canonical
workspace/Task roots and shared ancestors, covering future missing roots without
a population scan while siblings progress independently. Both APIs preserve unbound
conversations, Task/PR atomicity and unset Started. The rejected Home-wide guard
remains at `2d96630b3:wave/infrastructure/MEMORY.md`; both missing-root failures
remain at `be36cde18:wave/infrastructure/MEMORY.md`. Git leases remain separate.

Both registration APIs share one insertion transaction and return its committed
Task for publication, checkout setup and execution. Accepted issue facts retain
age and reserved placement; uncached initial facts remain usable. One store and
resolved Wave serve selection and insertion.

Rotation now gathers exact Task roots under ordered Wave locks and acquires one
checkout lock set. Queued planning writers retain both scopes after cancellation;
a caller-owned lock alone would release exclusion before a queued commit. Automatic
backlog cancellation and its standalone retirement writer are deleted. Unreviewed
backlog stays with its Project; unresolved abandonment keeps its separate settlement.

The October 5 file-binding implementation is superseded by the October 6
SQLite decision below. Its exact preservation evidence and unresolved output-handle
leak remain at `42e6c2706b1b35b2852e438ff94a49d060faccda:scratch/keep-every-wave-ready-for.md`.

Ensure’s initial operation fixtures seed post-binding recovery, not a crash. Evidence:
`57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:scratch/keep-every-wave-ready-for.md`.

October 5 rotation retains exact-ID input, selected issue membership and settled
history. Whole-input preflight includes accepted facts and legacy conversion;
all pairs reserve before provider writes. Creation intent survives retry; unfinished KRs stay editable.
Before switching, reclassify new work; afterward reconcile only saved selections,
preserving later starts and external moves. Planning updates preserve names,
summaries and unrelated text. Queued membership/conversion writers retain guards.
Operation fixtures cover partial settlement and both sides of the switch;
CLI crashes, Desktop and configured Intelligence acceptance remain open.
Earlier counterexamples: `fc6df439424bd341ec3cd8182c19b13ed45cffd7:wave/infrastructure/MEMORY.md`.

October 6: Jack Heart authorized SQLite selection and LOO-382's stream, deleting
the YAML selector and `ProjectPreparation`. Dependency `e887a21c1` entered through
`66a3daa86`; its dirty work stayed separate. Explicit mutations import YAML once,
retaining bytes; later edits select nothing. Creation selection/settlement commit
together. Accepted facts retain age and partial inventory. Readiness uses those
facts, transitions and the actual activation Exec; unfinished is unknown. Both
surfaces share activation; absent Flow hides its template. Watchers never activate.
Committed frames replace refresh callbacks. Only the live per-Wave command shows
“Preparing Project…”; cached plans survive it. Current transport errors precede
retained errors; Retry clears transport feedback. Public CLI/crash, mounted and
installed Intelligence acceptance remain unproved; source grants no installation
authority. Release's operation-entry lesson still applies. Earlier detail:
`c99baa3baa23d324b013a03ea02cdcbc8953c2aa:wave/infrastructure/MEMORY.md`.

Jack Heart selected Loopflow Desktop and Work-named observation on October 6,
then accepted the reviewed source/naming direction with “lg2m.” Wire/cache bytes,
selection keys and identities stay unchanged; Work remains Wave/Project/Task.
Workspace can be non-Git; TaskWorktreeSnapshot means Git placement. Loading is
WorkReadingStatus, distinct from WorkStatus. Review and the two-reader proof at `457e65d6b` do not establish public CLI
crash recovery or configured Intelligence repair. October 6 gate stopped below
the 32 GiB disk reserve after supported recovery; static checks passed, product
suites did not run. The CLI/crash suite remains absent. Release's entry-point
lesson applies: operation fixtures cannot prove dispatch/recovery. Desktop's
required boundary is headless app/view checks; mounted judgment is optional.


## One migration draft per Task (LOO-344, 2026-10-01)

Jack Heart selected one editable draft per Task and immutable released SQL.
Mechanics: [MIGRATIONS.md](../../rust/loopflow/src/store/MIGRATIONS.md).
Unshipped branch evidence: `986be7988:wave/infrastructure/MEMORY.md`.

## One main Home (LOO-342, curated 2026-10-04)

Jack Heart approved the one-Home cutover: ordinary CLI commands, Task workers,
Flow steps and agent tools use the installed CLI and `~/.lf`; an explicit
`LF_HOME` experiment initializes once and needs a fresh directory after schema
changes. [PR #1381](https://github.com/loopflowstudio/loopflow/pull/1381) merged
at `6c73356074c4`; installed routing acceptance passed on v0.12.31 and the Task
reads done (see the 2026-10-04 entry above). Current behavior belongs in
[CLI docs](../../docs/lf.md#use-one-machine) and [Homes](../../docs/architecture/machines.md#one-main-machine).
The dated cleanup receipts, v0.12.29–v0.12.31 release recovery and the exact
configured checks are in [the pre-curation memory](https://github.com/loopflowstudio/loopflow/blob/cd344891b611914adf44eeb53844193d7a987d2c/wave/infrastructure/MEMORY.md#one-main-home-loo-342-branch-evidence-2026-09-30).

### Legacy retirement completed (2026-10-04)

Jack Heart explicitly authorized clearing all legacy stores and their process
owners. All 37 identified processes exited after SIGTERM, none needing SIGKILL.
The four installed stores, the unused root database and two demo databases are
intact under `~/.lf-retired/20261004T161815Z/` (`retirement.json` records
identities, signals and paths); seven earlier snapshots remain at
`~/.lf-retired/20261002T191224Z/worktrees/`. `~/.lf-dev` no longer exists and the main Home
identity is unchanged. [Method and verification](https://github.com/loopflowstudio/loopflow/blob/c418953634bd101f51878d2be2b40fb3facafabd/wave/infrastructure/MEMORY.md#legacy-retirement-completed-2026-10-04).

## Worktree listing and fenced dispatch (LOO-375, 2026-10-05)

Jack Heart reported `lf wt list` at 44 s, 17 s after a deadlocked writer was
killed. PR 1 shipped in v0.13.3; PR #1456 merged October 6, after v0.13.5,
and shipped in v0.13.6 on October 6. Installed validation on 54 worktrees returned
20/20 successful samples per surface: external text median/p95 1.465/1.511 s,
JSON 1.387/1.443 s. Both miss the one-second aim. Production timing is installed;
local Git is the larger phase, with no measured hard lower bound. Numbers and method:
[report](../../scripts/benchmarks/wt-list/README.md).

- **Process count, not Git work, was the listing cost** (PR 1): about 370 serial
  Git processes became about 70 through batched ref reads, concurrent `status`,
  one GitHub call and per-commit-pair answers in `.git/lf-commit-facts`.
- Historical GitHub batching and retained-read mutex measurements remain at
  `6448e3c9e7e519585378feaffe04606bc1b55d3e:wave/infrastructure/MEMORY.md`
  under this heading; these branch observations establish no latency floor.
- Store opens validate ledger/schema, leaving full integrity scans to migration,
  doctor and install. Keep the Session fence outside runtime waits; OpenCode's
  fenced post remains bounded at 10 s. Exec observation waits 15 s, then warns
  and runs unrecorded. Exact measurements and counterexamples remain in the
  same `6448e3c9e7` archive above; current timings live under `<Home>/perf/`.
- **Jack Heart's delivery contract:** land after autonomous checks and honest
  benchmarks; installed timing is post-merge validation, so the Task stays
  open. Installed 0.13.6: 1.47 s text, 1.39 s JSON median over 20 pairs; ≤1 s
  warm p95 online is unmet.
- **Install preflight/promote read the OS account's Home whatever `LF_HOME`
  says.** Tests running them are container-only installation proofs.
- **A closed Session with a confirmed-dead provider no longer blocks Task
  admission or completion** without a completion receipt (Jack authorized).
  Live or unknown providers still block.

## Environment variables (LOO-341, reconciled 2026-10-07)

Jack Heart's audit lives in [Environment](../../docs/architecture/environment.md).
`LF_HOME` selects the machine's data directory and `loopflow.db`; shared Exec
names drive shell/tmux clearing. Earlier branch evidence remains at
`c31279995a4ea0eec09c053e39c2f71a81d26034:wave/infrastructure/MEMORY.md`.
LOO-370's October 6 completion above supersedes this section's pending-delivery
claim; it establishes no physical capture conversion.

## Task worktree membership (LOO-358, branch evidence 2026-09-30)

Jack Heart selected the Task's checkout as its general work set: every
AgentSession, FlowSession and Exec there, plus explicit binds. The shared Rust
SQLite reader supplies Task status and Desktop membership; the app does not
reconstruct ownership from paths. Membership is additive, includes descendants
at component boundaries and closed history, and survives a missing checkout.
It changes neither recorded usage attribution nor process/Flow authority.

Every Flow naming the Task is equally its work; the earlier marked-worker model
is superseded. Membership grants no process or settlement authority. LOO-408
separates Task decisions from execution and retains conservative checkout cleanup.
Earlier branch details remain at
`6448e3c9e7e519585378feaffe04606bc1b55d3e:wave/infrastructure/MEMORY.md`.

## Synced planning integration (LOO-334, 2026-09-30)

Main's landed cutover supersedes the intermediate-schema bridge. Retained
integration detail: `57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:wave/infrastructure/MEMORY.md`.
Planning preserves Task/PR identity without inferring execution progress.

## Planning and launch preservation (curated 2026-10-02)

The September 28–29 planning, launch-hardening and branch-isolation details are
preserved in [the pre-curation memory](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#earlier-synced-planning-and-runtime-selection-loo-334-2026-09-29).
The current one-Home and Session cutovers above supersede their development-store
routing, intermediate migration and Run-owner implementations, not unresolved
acceptance. Historical test counts remain evidence of those revisions only.

Jack Heart selected one local planning interface: repository Linear authority
when connected, private plans otherwise; Git shares definitions and bindings.
Cross-store discovery never grants another participant execution authority.
Repository definitions establish Waves, not arbitrary Linear Initiatives.
Jack selected one Initiative per Wave/subwave and portable A/B names; native
parent relationships depend on provider support. Owning-checkout definitions
include dirty additions/deletions; context-free reads use fetched configured
main. No-remote policy and outward definition sync remained unresolved.

LOO-334's durable constraints survive: presence, freshness and execution eligibility
are separate; omission is not deletion; acquisition time is not provider revision.
Partial/unknown-revision webhooks invalidate rather than invent complete facts.
Preserve removal evidence across lookup races. Project revisions cannot order
independent Initiative/Team relations; contradictions require explicit repair.
Repository-alias repair moves normalized planning atomically, without combining
conflicting observations. Explicit relocation and configured Linear completion
remain unproved. Historical chapter membership requires frozen evidence, not
reconstruction from current normalized entities. Lost completion responses require
provider readback; archive acknowledgement never proves KRs or transferred Tasks done.
Cached-Task outage admission belongs to LOO-326's current review, not these notes.

The launch-hardening incident/design remains at
[a5d76c576](https://github.com/loopflowstudio/loopflow/blob/a5d76c576609f6efc80a8ca2198480081066c63c/scratch/jack-heart/session-launch-hardening.md).
Selected-store absence did not establish deletion. Preserve original launch
failure, executable, data ownership, native identity and pending decision across
retry; no timestamp-based private-store merging. The original offending binary
and installation switch initiator remain unknown. Its retry test failed 6/7;
prepared JSON/window/provider identity did not prove conversation access.
Configured review continuity across installation and real Ghostty resume remained
unproved there; later results must name the actual boundary they establish.

LOO-321 / PR #1308's historical isolated-store design and local proofs remain in
that archive. Source-only fixtures never established Jack's installed worker,
app acceptance or remote app transport. Home placement is distinct from a data
directory, and database isolation does not isolate provider or checkout effects.
Restore saved Task/PR/checkout identity; explicit replacement owns new definitions.
Authored command removals must cover runtime, app and DTO consumers. Current
mechanics belong to docs/lf.md and docs/architecture/machines.md; superseded command
names and store-selection policies are historical evidence, not instructions.

## Managed account identity (LOO-339, branch evidence 2026-09-30)

Jack Heart selected the identity core for [LOO-339](https://linear.app/loopflow/issue/LOO-339)
delivery and authorized landing and a patch release without review. The earlier
expanded scope is superseded: [LOO-340](https://linear.app/loopflow/issue/LOO-340)
owns shared account state across Homes, current status by default, browser
suppression, Claude cached identity/routing, Flow account bundles and reset credits.
[LOO-338](https://linear.app/loopflow/issue/LOO-338) owns the command rename;
this branch retains `lf auth`. Authorization is not evidence of shipment.

Design, review and gate evidence remain at `8973f689a9e1:scratch/`; current
behavior belongs in [subscriptions](../../docs/subscriptions.md).

Usage acceptance cannot establish the intended login: validation compares
expected email and per-user subject, never shared workspace identity. Reconnect
stages and installs only after identity and duplicate checks, without proving
native refresh coordination or sole browser ownership. An unavailable identity
service is not credential rejection; a plan is separate from quota. Fixture
isolation includes executable selection: clear inherited `LF_*` authority and pin
the compiled source CLI. The four detailed findings remain at
`abd039b2a818669c43e7c189f6a37382335639f2:wave/infrastructure/MEMORY.md`.

Synthetic passes prove no live OAuth or installed acceptance. State remains
Home-local; LOO-340 owns shared authority. No installed repair is authorized.

## Account auth consolidation (LOO-320, curated 2026-10-04)

Jack Heart approved LOO-320's scope and delivery; cross-account continuation,
native refresh coordination and headroom ranking were excluded. Detailed branch
proofs and retained failures remain in
[the October 4 source memory](https://github.com/loopflowstudio/loopflow/blob/49f8385f0f41194434ce208a2b63ea10b570e723/wave/infrastructure/MEMORY.md#account-auth-consolidation-loo-320-branch-evidence-2026-09-27).
Current mechanics belong in [subscriptions](../../docs/subscriptions.md).

Native providers own OAuth and callback completion; a printed OSC URL or cached
login cannot prove the current attempt succeeded. Cached inspection must avoid
launch, decryption/import, broker contact and directory creation. Persist usage
windows with their original time/owner; missing windows and reset success never
prove new capacity. Record selected account before native Session discovery.

Browser login without pasted code, first-time connection, remembered Linear
profile targeting and live Claude/Codex windows remain unproved; the configured
Claude probe returned `invalid_grant`. No source binary may migrate the installed
Home. Synthetic passes establish no live or installed outcome. Remaining caveats:
`abd039b2a818669c43e7c189f6a37382335639f2:wave/infrastructure/MEMORY.md`.

## Task deletion and command ownership (LOO-305, curated 2026-10-05)

Jack Heart selected provider/local deletion, command consolidation and delivery
through the saved Flow; execution settlement remained deferred. Detailed branch
proofs, command mappings and the source-demo incident remain at
`c4373492cfc0c77fc27a7887045f74c48b381e72:wave/infrastructure/MEMORY.md`
under this heading, with original artifacts at `4a14c0a47dc6e04be9668fb72b737828565a931d`.
[CLI docs](../../docs/lf.md) and [planning architecture](../../docs/architecture/planning.md)
own current mechanics; historical command spellings are not aliases.

Fresh ownership authorizes deletion; acknowledgement or explicit trash evidence
confirms it. Missing membership proves neither. Preserve terminal times, Done
outcomes, PRs and Git; stale snapshots cannot erase positive confirmation.
Completion and planning have separate writers. Retain merged-PR evidence and
original completion time on retry. Planning-only creation needs neither checkout
nor agent; post-create allocation failure retains identity for recovery rather
than compensating deletion. Upstream tracking never defines checkout identity.

The configured deletion demo removed LOO-299–302 but advanced installed-Home
drafts and broke its older CLI; Jack then forbade branch-binary access and
promotion. Source proofs use disposable Homes without inherited authority.
Removal is not termination. Detail: `abd039b2a818669c43e7c189f6a37382335639f2:wave/infrastructure/MEMORY.md`.

## Task convergence (LOO-319, curated 2026-10-02)

[LOO-319](https://linear.app/loopflow/issue/LOO-319) and
[PR #1301](https://github.com/loopflowstudio/loopflow/pull/1301) retain ownership.
[Prior memory](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#task-convergence-loo-319-branch-evidence-2026-09-27)
preserves exact slice/demo evidence and superseded Flow compositions.

Jack Heart selected reconciliation before decision/publication and retained human
demo and authored delivery. Captured invocations keep their definitions; tests
must locate captured policy rather than copy current catalog indices. Feedback
never supplies a verdict, and failed decisions cannot regain authority. Task
agent choice survives PM refresh and reloads at each launch, including review.
A PATH-only provider repair failed: executable and Home must agree. The real
Codex fixture used synthetic feedback and predates generic loop-decide; configured
Claude launch/resume, real five-minute stall and rendered Desktop agreement
remain unverified. Local simulated liveness is not signal authority. Preserve
first PID/birth identity across missing samples; active tool work prevents a
false stall. TESTING.md owns fixture isolation and disk-resource policy. Source
verification uses disposable stores; no old branch binary may migrate main.

## Data model and performance decisions (reconciled 2026-09-30)

Jack Heart selected public objects with one SQLite owner, direct skill/operation
Execs and compiled Flow graphs. The four-store Session counterexample, LOO-298's
autonomous single-PR landing authorization, rejected decomposition and merge-not-rebase
decision remain at
`08191d19269af96e15d79918689a04b4fa44dc30:wave/infrastructure/MEMORY.md`
under this heading. Current contracts belong in the architecture and CLI references;
source history does not establish installed conversion or configured acceptance.

**One client, one main Home.** Jack reports only this machine is a client; the
pinned dev Home is gone and its active Tasks were moved by hand to `~/.lf`.
Do not recreate that Home or make fleet compatibility a cutover requirement.
The reported transfer is not a verified conversion. Branch verification stays
in disposable Homes with inherited authority removed; no branch binary writes
the installed Home. Historical import, old-format compatibility and intermediate
draft preservation are discarded. Keep current Work/links, account routes and
resumable conversations. Exactly three direct drafts remain: `record_execs`,
`project_status_chapters` and `session_ownership`; released SQL remains immutable.

**Merge and conversion have different proof.** LOO-298 owns its merge checklist;
this branch retains the earlier copy at `ab901f1f1:scratch/remaining-work.md`
in Git history. Configured provider and
Desktop continuity remain unproven; removing Jack's attendance requirement does
not turn fixtures into acceptance. Before release conversion, quiesce old writers
and new launches, preserve a consistent SQLite/filesystem backup and matching
executable, rehearse current-state retention against the exact candidate, then
verify the converted state before reopening writers. The private-copy converter
currently reads live sidecars; its database backup plus those reads is not an
atomic snapshot. Preserve native IDs, pending reviews, selected captures and
the manually transferred Tasks, without importing old turns or driver authority.

- **Three owners.** Exec is one actual lf process. AgentSession is one
  conversation, interactive or headless, surviving driver and engine
  replacement. FlowSession is one started Flow. History is subordinate to its
  owner and has no lifecycle of its own.
- **Every Flow step is an Exec** (2026-09-29): "why not have lf flows actually
  launch skill execs?" A skill step runs the same `lf skill` command a person
  would run; an op runs its own command. Flow running skills without an Exec was
  "a big leak" that reimplemented skill machinery; close it fully. Where direct
  and Task-step behavior differ, "run directly seems like it wins there always."
- **Parents are processes.** An agent-issued lf command's parent is the lf
  process driving that agent ("to be clear i still want being called by an
  agent process to give you the right parent-lf process"). The word parent means
  Exec to Exec only; loop and template relations need other words.
  Implementation matches Session/provider generation and origin to the current driver;
  a replaced provider retains its proven historical parent. Causal ancestry
  grants neither Task attribution nor Flow settlement authority. The final
  schema omits the unused caller-turn token and its intermediate archive;
  selected native events identify Flow completions.
- **Definitions compile; runs are skills and ops.** A Flow definition may
  reference other Flows; starting it compiles them into one graph (Jack's word).
  A subflow is "more of a lens than an operational entity". Loop passes are not
  child FlowSessions either (2026-09-30, reversing the earlier "runtime nesting
  creates parents" rule): a pass is a node and iteration position. The direct
  ownership migration creates this final shape. Earlier child-pass archive and
  intermediate-schema conversions are deleted under Jack's compression decision;
  their older proof is not proof of the final three-draft conversion. Retry keeps
  its pass, Iterate advances return counters, and one FlowSession owns progression.
- **An ID names an object.** A captured input is not an object: it is an event
  in AgentSession history that names its Exec. `RunId` and its side table were
  deleted. Prefer the word Exec over launch or run where the thing is one agent
  start under one lf process.
- **Flow decisions are typed results** of the selected successful turn,
  modelled on PydanticAI and Jev: the step declares its output schema. The
  in-turn decide and route commands are removed; Jack said the command "felt
  wrong". On 2026-09-30 Jack selected Blocked with a required reason in that
  structured result; the blocked command is removed. A keyed Ask returns
  feedback to another turn of the same conversation without moving the cursor.
- **Bind** is write-once null to Task, allowed on done Tasks, and sets Started
  once. Jack selected prospective usage attribution for now on 2026-09-30;
  earlier usage retains its recorded owner. Keep this one read-time choice for
  Intelligence to re-evaluate; do not invent a mid-turn token split.
- Every denormalization has a validator or is deleted. Method for any model
  review: derive the user's objects and APIs from the product first, then check
  the infrastructure for hops.
- **Naming remains a choice.** Jack kept `session_events` → `agent_events` and
  the earlier `exec_events` rename proposals open. `run_events` is now deleted;
  Exec results live on `execs`, so there is no current journal table to rename.
  This observation neither closes the proposal nor authorizes a new event owner.
- **Optional chapters, explicit selection.** Jack Heart's October 4–5 direction
  supersedes this cutover's status-based selection and backlog expiration: one
  shared local binding selects each Wave's Project; chapters and Flow are optional.
  Chapter creation requires KRs, and backlog survives until explicit disposition.
  No Chapter table, second deployed client or distributed transaction is implied.

The integrated gate, implementation lessons and compression receipts remain at
`c418953634bd101f51878d2be2b40fb3facafabd:wave/infrastructure/MEMORY.md`
under this heading. The original full gate had seven failures; focused repairs
passed, but did not establish a full final-tree pass, configured acceptance or
installed conversion. The production diff was +6,202 lines, not a reduction.
Preserve ID ordering through Session projection, use dedicated PR stacking writes,
and resolve automatic checkpoints through the same Work binding as execution.
Provider stubs must contain conflict-agent launches: one bad fixture launched
real credentials whose effects were not audited. Dense CLI timing and configured
continuity remained unfinished in that dated evidence.

Performance (instrumentation implemented in LOO-291; LOO-300 continues): `os_signpost`
intervals under `studio.loopflow`/`perf` for cold start, navigation, Wave/Task/
Session paint, every `lf` read, Markdown parse and terminal key-to-draw;
`scripts/benchmarks/desktop-performance/record_live.py` records local usage
without telemetry. The retained [90-second idle recording](../../scripts/benchmarks/desktop-performance/20260926-demo-app/report.md)
measured `session list` at p50 809 ms and `roadmap --all` at 3.49 s; `ps --json`
was 274 ms, so not every read exceeded the proposed 300 ms budget. It recorded
zero hitches but one 1.85 s potential hang and nearly flat RSS. The earlier
installed build's six-second probe measured 51 ms/s hitches; these different
windows/builds do not prove a causal improvement. Republishing identical readings
was found in source and removed; remaining hang causes need profiling.

LOO-300 owns Session streaming, projection caching and the density harness after
the data-model work. The handoff records passes only for cold-start-to-outline
and terminal-key-to-echo. `PerformanceCatalogueTests` also retains a filter test
that can skip when SwiftUI exposes no NSTextField; six other tests were removed
after mounted paint hooks failed to fire. Missing results remain proof gaps.
Key-to-next-draw and PTY echo are proxies, not glyph presentation. Click ≤100 ms,
`lf` read ≤300 ms off the main actor and idle ≤5 ms/s hitches remain proposed
targets until comparable measurements support published budgets.

The earlier S5 active-PR resolver left landed branches unbound. That is historical
failure evidence, not binding policy: current Session rows own attribution and
write-once bind permits done/landed Tasks. Preserve current ownership through the
one-machine conversion; discarded historical attribution needs no importer.

The superseded source-install staging incident remains at
`1452f58d3:wave/infrastructure/MEMORY.md` under “Data model and performance
decisions”; current published-installation policy governs verification.

## Continuation, delivery and authority lessons (curated 2026-10-02)

The detailed September 23–25 observations, historical schemas, test counts and
counterexamples remain in [the pre-curation memory](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#continuation-and-recovery-lessons-curated-2026-09-25).
The September 30 model/cutover decisions above supersede Run, Ask, Project-cursor
and private-store mechanisms there. Archiving detail does not establish remaining
acceptance or authorize deleting execution history.

- LOO-295's October 4 operator acceptance above supersedes this section's
  unresolved continuation claim. LOO-296 retains broader restoration; historical
  manual Flow/native-Home corrections and ambient-account/lease gaps remain in
  the pre-curation memory, not proof of automatic recovery.
- Capture all alternatives and human boundaries before execution; recover from
  saved definitions after sources disappear. Invocation identity fences late
  results independently of reusable versions/generations. Consume actual
  successful completion evidence under the exact claim. Cursor settlement does
  not prove external effects happened once. Serialize feedback/reopen writers,
  persist completion before teardown and preserve published native identity.
  A finished Flow does not complete its Task. Prove driver and native handoff,
  not only reducers, including another executable first on PATH.
- Unknown custom/queued/nested history stays unresolved with bytes/order intact.
  Inspection does not recompile or launch it. Causal ancestry, age, a missing
  listener, finished launcher or absent display cannot grant process authority
  or prove provider death. Preserve original dispositions and exact process
  receipts, including hashed executable names and PID-reuse rejection.
- One Task execution snapshot feeds status/actions/roadmap. Current execution
  evidence wins over dirtiness or next-launch configuration errors. Missing
  observation must not become idle capacity or a duplicate worker; the earlier
  LOO-293 duplication came from split readers and that inference. Catalog joins
  use stable identities and minimal schemas; current hierarchy filters do not
  rewrite historical attribution. Fixtures isolate all ambient authority and
  stop only their own identified provider children.
- Preserve authored PR copy plus the managed Task block; publication alone does
  not request settlement. Draft opening preserves readiness; promotion records
  state only after provider success. Exact copy must survive release re-arming.
  Same-SHA pending/passing/merged changes are progress without another commit.
  Failed repair must surface its real blocker rather than loop indefinitely.
- Queue cancellation is distinct from disabling auto-merge: gh 2.101.0 could
  return success without dequeueing. Reobserve authoritative queue state before
  replacing a head. Canceling an async waiter does not stop its blocking child;
  keep the actual checkout lock and effect ownership until descendants finish.
  Local landing tests did not establish hosted interruption/permissions or
  orphan cleanup; green-unmergeable PRs and the old Etude -c discrepancy remained
  unresolved in those records. PR #1289 proved deleted remote-branch recreation
  only. An empty lease must still reject a branch created after absence was read.
- Rotation preserves started identities and freezes the boundary's membership,
  metrics and evaluation time. Failed fresh reads cannot justify retirement;
  retries consume the frozen boundary and verify uncertain writes at the provider.
  Deduplicate stable Work IDs; renamed PM labels do not rewrite captured history.
  LOO-278's integration did not transfer its registered checkout/PR ownership.
  Historical archives remain outside live architecture discovery.

LOO-286/LOO-287's broader configured continuity obligations are retained where
not superseded by the accepted cutover: concurrent starts, real provider death
and same-cursor replacement, late-result rejection, helper non-authority, and
exact Desktop close/reopen continuity. Their seeded stores, missing executables,
sleeping children and isolated tests did not prove those outcomes. Old migration
counterexamples dropped controller-only Task/Project positions; the newer
current-state cutover's explicitly accepted scope governs disposition, not a
retroactive claim that those proofs passed. Finite Project Runs and generic
Wave/Project execution symmetry are superseded, not future implementation mandates.

The deleted lease/liveness stacks (a7044e2b5, 5f7f66833) and repeated prompt-fallback
add/delete/restore cycle show why failure must be repaired in the operation.
The full operational cause of run_liveness removal remains unknown. UI projects
shared evidence and triggers operations; it is not another scheduler or synthetic
controller. Historical automatic-trigger/default-wait proposals need current
selection, not automatic resurrection. Current delivery mechanics live in
[delivery documentation](../../docs/architecture/delivery.md); TESTING.md owns
populated migration and clean-PATH proofs. Refresh exported skills after changes;
Wave learning stays with its identified owner, never miscellaneous .lf notes.

## Prompt reduction boundary (2026-09-24)

`719226ef4:wave/infrastructure/MEMORY.md` retains the source references.
Historical identity, execution eligibility and consumed launch evidence remain
separate. LOO-287's architecture pass and weekly observations remain unproved;
local deletion and checks establish no KR. Intelligence owns prompt assembly.

## Installation and command scope (curated 2026-10-02)

LOO-292's installation acceptance closed below; LOO-287 retains command-scope
reduction. [Prior memory](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#installation-and-command-scope-branch-evidence-2026-09-24)
retains September 24 fixtures, failed public v0.12.20 clean-home promotion and
exact branch evidence.

Installation verifies a pinned published installer; the promotion transaction
alone activates artifacts/store. Checkout integration is separate. No Git,
Homebrew, uv, Python or source maintenance belongs in installation. First install
has no previous selection: preflight must precede ordinary startup authorization,
and the pinned candidate owns recovery after handoff. Matching version strings
are insufficient without exact-store preflight and a complete matching macOS app.
Bound inspection of broken binaries. Do not restore the old Python refresh alias:
delegation through PATH recursed into the old source updater.

Preserve launchd label/logs and custom install directory, without source
WorkingDirectory. Simulated launchctl proves no real timing. Promotion resolves the OS
account home: HOME/LF_HOME and PATH mocks cannot isolate it. Use disposable OS
accounts/containers without the real installation; verify failed first activation
and retained-candidate recovery. Artifact-copy completion and member hashes matter;
one asynchronous fixture copy produced mismatched bytes and a segfault. Ubuntu
24.04 reached promotion where Debian bookworm lacked the release's GLIBC versions.

Ordinary-folder absence differs from a genuine Git error; never invent an empty
repository or silently mutate a default route. Machine commands need no repo
capture. LOO-287's ordinary-folder/provider-completion and target-first resolution
remain unproved across every consumer. New-repository creation/configuration/PM
setup remains unselected. No receipt redesign was selected: switch phase alone
cannot substitute for durable advancement evidence. Current mechanics and proof
commands belong in docs/lf.md and TESTING.md.

## Installation and checkout closure (LOO-292, 2026-10-04)

Jack Heart requested closing LOO-292 on actual machine evidence. Install updates
published machine artifacts only; `lf task sync` (which replaced `lf rebase` in
#1367) owns checkout updates. The schedule is opt-in login plus weekly, Monday
09:00 local, with positional daily/hourly/5min. The separate daemon is retired.

Published 0.13.0's October 4 installation and checkout acceptance, exact hashes,
cadence receipts and retained limitations remain at
`6448e3c9e:wave/infrastructure/MEMORY.md`
under this heading. Installation preserved the Home, repaired a missing entry in
an isolated Ubuntu container and preserved caller bytes during checkout sync.
No Monday firing, sleep-coalesced wake or interactive app acceptance was proved.
Unresolved: a currency probe triggered a redundant download under load; timeout
is only a hypothesis. Reload killed that download without damaging installation.
A hand-truncated entry gate is not healed by reinstall.

## Shipped history

Historical installation, rebase/placement, PM, OAuth and cron delivery records
remain in [main's preserved memory](https://github.com/loopflowstudio/loopflow/blob/52ab4a4a5cf1ec3c24b019d5cee3a1c782a30d9b/wave/infrastructure/MEMORY.md#shipped).
Current command, Task ownership and installation contracts above supersede their
old names and execution models. Cron continuity judges each latest due interval
against an exact scheduled receipt; manual receipts do not prove firing, while
failed scheduled targets do. Historical gap days do not keep later telemetry red.

## Gotchas

- **`scripts/test.py --all` cannot green the Loopflow UI suite headlessly**. `xcodebuild` runs 304 app/unit tests to a pass, then `LoopflowUITests-Runner` hangs before establishing its connection and Xcode exits 65. Reproduced with a fresh `derivedDataPath`, so it is not a stale-cache artifact. Treat a `--all` UI failure as unproven, not as a regression, until the runner hang is fixed.
- **Run `cargo test` to completion before trusting a green-looking suite.** A failing lib target makes cargo skip every later target, so lib failures mask bin failures.
- **Rust compilation does not validate SQLite column names.** Runtime SQL whose shape depends on a released schema must be shared with a behavior test that prepares and executes it against the materialized migration head. Epoch Work ownership is three exclusive foreign keys (`wave_id`, `project_id`, `task_id`); generic kind/id belongs to explicit routes such as synchronous cross-Work questions, not to Epochs.
- **Source history must reconstruct every applied release frontier** (learned 2026-07-20). One pre-schema-closure local promotion embedded a test-materialized `0.12.4` batch and advanced the shared store while git retained the ten source drafts and omitted the canonical file. Recovery preserved the database, extracted the canonical bytes from the retained immutable binary, matched their checksum to `schema_migrations`, registered the batch, and removed only byte-identical drafts. If a store is ahead by an unknown migration, retain state and old binary bytes; prove the checksum before ratifying history. Since #1123, draft-bearing candidates fail promotion even at an exact frontier, while a schema-complete exact-frontier CLI repair may safely activate with live Runs because it writes no migration.
- **Tests must survive draft migration materialization** (learned 2026-07-21).
  Release-equivalent Rust tests delete ordinal-free drafts and compile the
  generated canonical batch. Test fixtures resolve migration SQL by its draft
  marker through `migration_sql_for_test`; an `include_str!` pointing directly
  at `migrations/drafts/` passes locally and fails the release tree at compile
  time.
- **Installation tests need OS-account isolation.** Jack Heart’s October 5 steer forbids the three LOO-370 host checks named in TESTING.md: getpwuid bypasses HOME/LF_HOME. PR #1444 supplies disposable-account proofs; until integrated, isolated CI owns them. Ordinary fixtures also scrub inherited LF_* authority.
- **Concurrent editing corrupts a file; concurrent rebasing corrupts history.** Two drivers sharing one worktree shared its `rebase-merge` state dir: conflicts resolved themselves between commands and `done` advanced 6→22 with no `--continue` from the losing session. Check for a live agent before working — or rebasing — a wave worktree; the driver that owns the worktree owns its `.git` sequencer.
- **Linear Project UUIDs survive renames; derived slugs do not.** Project content
  lives in Linear and the local SQLite snapshot, with no `projects/*.md` cache.
  Use stable IDs when reconciling current names with captured historical plans.
- **Environment configures a process; it must never decide what the process is.** An earlier runtime chose between booting a listener and being a resident from inherited environment, so a promoted wave could attach to its parent's listener with the parent's token. The current `lf wave` surface keeps that role explicit.
- **Current PM truth and durable Work history have different lifetimes** (learned 2026-07-21). A terminal Project omitted from the current PM snapshot can still own non-terminal historical Task Work. Wave reads must render the current PM hierarchy and classify the stranded Project/Task separately as Wave-owned degraded evidence; they must not fail the whole join, delete history, or synthesize a PM Project. Preserve stable identity when the historical checkout is absent. LOO-305 removes the old local-only `work abandon` recovery command; inspect retained facts with explicit `lf task status <task-id>`. Missing Project evidence alone never authorizes provider deletion.
- **Terminal Task state and current PM routing are authorization boundaries**
  (learned 2026-07-21). An open Linear issue, inherited direction, or sibling
  completion is evidence, never permission to reopen `Done` or `Abandoned`
  Work. Recovery from abandonment requires explicit User authority. When
  Linear moves an issue, historical Task Runs retain their evidence but lose
  automated PR and completion authority; fail closed before side effects and
  preserve the full Work, Run, Steer, and PR history for remediation.
- **Historical continuity currently short-circuits daily telemetry** (observed
  2026-08-23). `telemetry-daily` stops in `doctor` on the same eight 2026-08-04
  through 2026-08-11 gap days before its scorecard runs. LOO-241 owns making
  continuity obligation-aware. Fresh receipts are new evidence for that Task,
  not grounds for duplicate daily Tasks; retry its Work only from a Turn with
  valid Run execution context.
- **Release orchestration and product publication are separate evidence**
  (observed 2026-08-23). A cron receipt proves only the scheduled target's
  terminal state. `lf release status` remained at tag `v0.12.14` with a
  successful hosted workflow and gate-safe notes but no GitHub Release after
  both successful and failed `release-run` receipts. Judge the release KR by
  the product state and keep same-tag recovery singular; LOO-261 owns the known
  clean-host candidate-validation boundary.
- **Incomplete release synchronization still consumes caller edits**
  (reproduced 2026-08-23). The scheduled retry left `main` clean after removing
  two pre-run Infrastructure memory edits. LOO-266 owns preserving the caller
  branch, index, and working bytes across every release exit; do not file a
  second repair Task for later instances of the same failure.

## Durable model lessons (curated 2026-10-02)

[Earlier model and planning records](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#model-design-settled)
preserve their exact decisions and implementation history. Current objective,
accepted optional-workflow direction and authored release schedule supersede old
nightly/weekly cadence, daemon-chat and mandatory Project assumptions.

Self-hosting remains the default; credentials follow the repository Doppler
policy. Products own release substance; avoid a generic deploy platform before
real consumers require it. One writer per checkout is dispatch discipline;
mutation-specific locks do not create a general execution authority. Durable
state is not a message bus, and attributed input cannot manufacture another
controller. Task completion owns its transaction/event; a merged-PR status read
must not complete it or mint a synthetic agent execution.

Performance evidence distinguishes missing receipts/fields from measured zero.
Preserve first accepted usage and conflicting-repeat partiality. Window by the
owning fact's terminal time; publish measured coverage beside percentiles.
Unknown authority cannot be inferred from observer time or trace prose.

Wave/Project/Task map to provider-native planning objects; goals/memory/definitions
remain in Git and plans in their authorized owner. Historical label migration
and old command names are archived, not current instructions. KRs are explicit
judgments, never inferred completion. Preserve unrelated provider associations;
ambiguous moves remain diagnoses. Standing frontier plans need not acquire an
invented completion date. No fourth user-facing planning noun was selected.

## Earlier follow-ups (reselect through the accepted chapter)

Historical suggestions remain at
`d281370191844294ce0ad877752f2aa6a6402282:wave/infrastructure/MEMORY.md#earlier-follow-ups-reselect-through-the-accepted-chapter`;
they authorize no current work or retired owners. PR #818 resolved rebase-efficiency
follow-ups. Measure command drift, avoidable rebases and post-land repairs before
tuning policy; the synthetic harness remains unbuilt. Jack Heart's July 6
“up/down 5ths” referent remains unresolved and deferred.

## Direct invocation and large inputs (curated 2026-10-05)

`44fe36620:wave/infrastructure/MEMORY.md` retains evidence. Preserve caller-owned
checkpointing, common invocation loading and Claude's file-backed stdin. Rejected
Codex `turn/start` remaining waiting is unresolved; Claude success proves no repair.

LOO-420 (2026-10-07): Jack Heart selected native same-harness invocation,
translated ports, inlined builtins and `--agent`/`-a`. Codex receipts prove source,
separate context and same-thread resume, not recall without resupply. Claude hook
context survives deletion; rendering metadata cannot establish model authority.
Exclude answer leakage. Installed 0.13.9 app launches recorded no native identity;
both reconnects failed. Jack then directed removal of `--ide` and its app-launch
path. Keep terminal/headless execution and historical records. Launcher success
proves no engine exit. Native dispatch, ports and fidelity remain unproved.
[Probes and limits](../../scripts/benchmarks/skill-invocation/README.md).
