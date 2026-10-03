# Account for scheduled release opportunities and settle product outcomes

LOO-285 · accepted outcome; same-Home persistence revision implemented locally.
Reconciled October 2 against `02d6b3c00` (atomic same-Home coverage) and
`f7a3769dc` (main integration through v0.12.32). The supplied continuation feedback
predates that implementation; the complete Task outcome remains unproved.

## Outcome and accepted decisions

Maintainers need one authoritative connection from every configured due release
opportunity to execution, required verification and actual product outcome.
Published, truthful no-change, deferred with reason and continuation, and failed
with actionable repair remain distinct. Cron process success never establishes
product success, and scheduling continuity never clears failed verification.

Jack Heart accepted one release execution per scheduled wake, covering the
outstanding due set frozen at entry. Every original due survives. An existing
candidate keeps its opportunity owner; otherwise the newest outstanding due owns
catch-up. Collapsed misses refer to that execution and count once, never as extra
successful settlements. Dues arising during execution wait for a later wake.

Completion still requires two adjacent original configured opportunities settled
by two distinct automatic executions, at least one with artifact publication,
all required checks and no manual repair. Delayed automatic starts may qualify;
collapsed or operator-triggered executions cannot manufacture the second success.
The separate fourteen-day availability window is not claimed by this Task.

The existing serial Reliability Task and coherent PR retain implementation and
review ownership. No scheduler replacement, new resident service, generic deploy
platform, process-liveness registry, PM redesign or new Task is needed. Existing
release recovery, calendar calculation, cron executor, generated-worktree
classification and OS locks remain the implementation owners.

Jack's September 28 activation steer records successful v0.12.24 publication,
supported installation, cron preflight and skill-to-Flow sync at the unchanged
10:00 schedule. The misleading original success receipt survived. That is
activation evidence, not proof this later accounting implementation is installed
or either required configured settlement occurred. The authored 09:00 telemetry
and 10:00 release cadence remains unchanged; Release's nested Wave owns the
release schedule. Installed placement/ownership must be observed, not inferred
from authored files.

Jack's October 2 direction retained this checkout and all authored work, required
main integration and closed same-Home continuation, and prohibited production
release, branch installation, schedule changes, Home-authority transfer,
verification waivers or completing Jack's review. The latest feedback reports
main merged and focused recovery, publisher, minor-release and static checks
passing. Source history places the merge at `5e10e9cd1`, CI-repair ownership at
`7c401c1e7`, and the final smoke-command formatting at `fe36937fa`.

## Implemented ownership and behavior

| Owner | Existing implementation | Evidence limit |
| --- | --- | --- |
| `ops/cron/accounting.rs` | Private-Home segment documents retain original dues; one atomic owner-attempt write freezes coverage across same-Home predecessors. Telemetry segments have no release opportunities. | Historical unobserved coverage remains unknown; configured continuation is not yet observed. |
| `ops/cron.rs` | Physical receipts retain process outcome, source and optional runner birth identity. One executor handles ordinary firing and one reserved Recovery prerequisite per wake. Trigger requests retain operator provenance; ordinary kickstart does not force-replace a job. | Shell/seeded-date tests do not prove live launchd timing or trigger races. |
| Flow command executor | `.lf/flows/release-run.yaml` contains `cmd: repo release run patch`. Explicit receipt and descriptor attribution reaches the release operation through current Flow/Exec dispatch. | Old `op:` and Run-era integration plans are superseded; installed propagation remains acceptance work. |
| `ops/release.rs` | One manual/scheduled controller owns target exclusion, exact selection, checks, recovery and settlement. Fetched origin and owned exact-source checkouts preserve caller bytes. | Local preservation cases do not cover every interruption boundary. |
| `settle` | Exact-attempt fencing atomically saves attempt verification and product outcome. Conflicting/late evidence cannot replace accepted success. `finish_process` cannot promote zero exit. | Product proof must still be independently complete. |
| `scripts/publish_release.py` | One ArtifactReceipt retains prepared identity/hashes/stages; public proof records fresh read-back and smoke. Reconcile reuses existing publication operations. | Simulated endpoints/signing/UI are not live acceptance. |
| `ops/cron/history.rs` | Joins opportunities, physical receipts, retained prerequisite failures and dated dispositions; judges original adjacency and intervention eligibility. Closed unsettled owners remain visible outside the display window. | Assignment is neither execution nor settlement. |

The obligation identity includes canonical repository, stable Home/Wave, job,
schedule and retained timezone interval; each due key retains its original UTC
instant/local calendar evidence. Target kind is execution provenance, so a
skill-to-Flow cutover preserves the daily obligation. Changed placement, schedule
or timezone closes a segment; successor links preserve closure even if the old
snapshot rewrite is interrupted. Removal preserves the predecessor before unload.
History materializes missing dues read-only, preserving an absent-wake denominator.
The shared calendar uses the first ambiguous occurrence and skips nonexistent
local times. Today's timezone cannot reconstruct unobserved historical changes.

The owner document commits frozen coverage once. Shared reads derive collapse links
across segment documents; new writes omit the redundant persisted links. Existing
documents remain readable, including attempts without execution-segment provenance. Accounting uses a short lock, released before target acquisition/network
work. The target lock covers selection and mutations; the existing job descriptor
authenticates scheduled attribution; checkout leases protect exact source removal.
None grants another owner's authority. Keep schema-1 physical receipt decoding
because those are durable history, not a compatibility shim for an internal DTO.

The current model uses AgentSession/FlowSession/Exec. Cron receipt identity remains
the physical-attempt join; no new generic Run object or SQLite opportunity table
is needed. Main's placement-preflight failure path remains part of accounting.
Taskless and managed execution use the merged command machinery.

### Saved candidates, invalid sources and queue recovery

Saved-candidate inspection is implemented, not the next slice. Under the existing
target lock, the publisher inspects the exact source. A valid candidate resumes
unchanged. An invalid candidate permits one successor selection only after
affirmative unpublished evidence from GitHub, crates.io and versioned R2. Unknown
or partial publication and a moved original tag refuse replacement.

The running attempt retains rejected selection plus inspection/authorization;
original dues, prior failures, manual provenance and frozen coverage survive.
An interruption before successor selection leaves the rejected candidate pinned
and requires fresh authorization on retry. Stale writers cannot authorize/select;
a successor does not inherit its predecessor's workflow id. Another invalid
successor remains saved for a later wake, without a new opportunity or extra
settlement. The recovery decisions reuse run-records' implementation observed at
`6acd2389e878bbc47c1b80bdab8591ebf06a46fe`; caller-preserving selection was retained.
Cached packages undergo successful packaged preflight in a fresh disposable Home.
Published authority alone cannot excuse rejection.

The integrated controller retains current queue-aware `observe_pr_merge` /
`merge_needs_integration`, reobserves merged state after repair errors, and carries
target/checkout ownership through CI repair children. Queued work waits;
AwaitingQueue does not force integration merely because a head is behind. Main's
minor-cycle recovery is integrated; the older note describing it as unported is
historical. A merge and reported focused passes do not establish a full gate.

### Child lifetime and caller preservation

The existing target/checkout descriptors reach tag/publish/inspection children,
shared PR mutations, auto-merge, Git stage/commit/push, hooks, lockfile tools,
notes providers, source fetch/add/reset/remove/delete and Task compensation.
The CI-repair integration additionally carries them into the repair process tree.
The shared PR/Git/Task owners remain authoritative; no parallel release writer
or ambient role inference is introduced.

Cleanup drops the controller's lease and independently reacquires before removal:
a surviving child sharing the old description still excludes it. Unexpected
post-checkout HEAD changes retain both checkout and branch instead of deleting
unpublished work. Notes input lives in the existing prompt store with a unique
name, so launcher exit cannot erase a surviving provider's input. Named source
creation has no main-sync mode or implicit background branch push. Earlier
negative searches missed indirect `sync_main`; the hook regression exposed a
stash replacing the held lock inode. The later removal of that mode supersedes
those overly broad negative claims.

Local tests execute real CLI/Git/bare origins/processes/locks and check resulting
state and caller HEAD, branch, raw index, staged/unstaged/untracked bytes and local
commits. Provider/GitHub/telemetry/publication evidence is simulated where noted.
They do not prove arbitrary vendor descendants retain descriptors or permit
killing production processes to gain access.

### Telemetry, dispositions and truthful outcomes

Every attempt freezes historical prerequisite segment/receipt references for its
covered original dues separately from current observations. Missing segments,
unknown timezone history, different Homes, and a new segment before its first due
stay unknown. Canonical repository aliases resolve consistently. Referenced old
failures remain visible outside the requested history window.

Missing/failed current telemetry can reserve one Recovery receipt before launch
through the same installed cron executor. One hour bounds observation, not child
runtime. Deadline/error leaves an unobserved process result unknown; no child is
killed or late exit invented. Live/unknown runner identity defers; proven runner
death still requires job-lock acquisition, which a surviving child excludes.
A successful new check is current recovery, never a rewritten historical pass.
Same-second UUID ordering cannot establish recovery order: Running/Failed evidence
wins conservatively over a tied success.

Doctor continues to judge natural Scheduled firing evidence independently of
result. Recovery cannot fill a missing natural receipt, so the actual telemetry
Flow may still fail continuity during catch-up. The merged Flow runs doctor,
scorecard and weekly usage. `scripts/lifecycle_scorecard.py` now consumes supplied
SessionHistory JSON and current Task PR facts; its old `agent_turns` /
`agent_invocations` SQL is gone. The missing-table diagnosis dates to September 24,
not a fresh failure of this source. All 36 failed receipts (including the original
35) remain counterevidence. No current installed pass or accepted Intelligence
handoff is established; commissioning a duplicate analytics repair is unwarranted
without reproducing a remaining failure against the integrated path.

`lf cron disposition` requires a registered local Task Work id, explicit Wave and
reason, and appends dated ownership. It neither claims remote acceptance nor
rewrites the receipt/opportunity. First-assignment lateness survives reassignment.
Closed unsettled owners expose original Home/candidate/closure and dated repair
outside the window. Closed saved waits are labeled historical. A replacement wake may continue its
uninterrupted same-Home predecessor chain. The candidate retains its original
owner; execution provenance identifies the successor that fired. An already
executing exact attempt may settle after closure; attribution never transfers a Home.

Published requires exact tag/commit, applicable verification, prepared hashes,
all publisher stages and public read-back/smoke. No-change requires a captured
origin tip, exact empty target-scoped range and a completely verified published
baseline plus current prerequisites. An unavailable range is not empty. All
resume/no-change paths share applicable verification; no early-return bypass.
Deferred retains its typed reason and real continuation; unclassified errors,
failed checks, unavailable authority and corrupt evidence remain failures.
External publication followed by smoke failure retains the effect without
qualifying success. A wrapper failure after settlement remains a separate fact.

Required proof includes scheduled telemetry, repository verify hooks, release PR
checks, hosted acceptance, all four native packages and exact-version CLI/daemon
smoke, packaged candidate acceptance, signing/notarization, installer/website and
every configured publisher stage, required UI-host evidence, exact public hashes,
versioned DMG and isolated pinned installer/package smoke. Mutable latest URLs
alone cannot prove the tag. Immutable-subject checks may reuse matching evidence;
current-state prerequisites cannot borrow yesterday's pass. Legacy missing UI
proof requires the exact-source gate. Headless capability belongs to gate/CI;
missing capability never silently waives required acceptance.

## Remaining implementation and proof

1. **Physical overlap reporting.** The cron job-lock loser still records generic
   “next configured firing” text in `ops/cron.rs`. The release controller's
   observation-time continuation already uses the retained calendar, including
   closure and DST; physical overlap receipts need equally truthful continuation.
   `release_history` retains out-of-window telemetry references, but does not
   include release attempt receipt IDs in its linked set. Preserve those physical
   receipts when their retained attempts remain visible, without counting an
   overlap as a settlement. These consumer gaps are separate from atomic coverage.
2. **Integrated interruption/verification proof.** Exercise actual telemetry's
   missing-natural-receipt path against current SessionHistory analytics without
   relabeling Recovery or weakening doctor. Retain truthful blockers and dated
   disposition. Complete unexercised candidate-ref/workflow, public-reconciliation,
   Task compensation beyond pre-push and release materialization boundaries as
   justified by the final call graph. Closed continuation now has the synthetic
   post-publication/pre-settlement recovery proof described below. Review command
   dispatch/descriptor retention through merged Session/Exec/Home/Flow paths.
3. **Affected-suite gate and configured acceptance.** Focused passes are not the
   final affected-suite gate or hosted matrix. After code proof and authorized
   delivery, supported installed acceptance still needs telemetry, UI-host/public
   proof and the actual two-execution pair. No branch binary may repair the main
   Home, and no release/schedule/operator intervention may manufacture that pair.

Same-Home continuation and observation-time retry are implemented in
`02d6b3c00` and `a60ac0281`. Source inspection confirms the materialize-before-owner
write, shared derived coverage, candidate preservation, late-writer fencing and
Home-change boundary. The existing synthetic unit and CLI tests cover those
contracts, including reconciliation after publication but before settlement.
They do not close the integrated or configured proof above.

The `f7a3769dc` merge adds v0.12.32 version/lock metadata and release notes for
Task-worker recovery and PR rediscovery. It adds no accounting implementation or
configured acceptance evidence. Those recovery changes do not grant release
settlement authority or supersede this Task's production restrictions.

No new product choice is needed for these accepted local contracts. Transferring
an old Home's authority, accepting a new remote repair handoff, changing cadence,
weakening proof or changing the coherent-PR boundary would require Jack's decision.
Those changes are not selected. Concrete operational blockers belong beside their
observations; historical startup errors do not prove a current blocker. The old
0.12.29 foreign-Team restart and pre-merge missing-Daemon fixture failures remain
historical after Jack's reported 0.12.31 recovery and main integration.

## Same-Home persistence revision (October 2 implementation)

The earlier review correctly rejected segment-local continuation: replacement
wakes span predecessor and successor documents. Dropping closed checks or recovering
only old dues would violate Jack Heart's frozen-all-outstanding decision.

The implementation materializes original dues first, then atomically appends the
new attempt on its candidate's original opportunity. That attempt's `covered` set
is authoritative. The shared reader derives collapse ownership across retained
coverage, rejecting conflicting owners, cycles and cross-Home claims. Resolved
owners are cached within the read so repeated failed catch-up does not cause
exponential traversal. New documents omit persisted `coalesced_into`; history
still exposes the derived field. Retained documents and earlier attempts lacking
`execution_obligation` remain readable as original-segment executions.

A wake follows only its uninterrupted same-Home predecessor chain. Home changes
break the chain even if later placement returns to the original Home. Existing
candidate identity remains on its original due. Multiple retained candidates
remain a named repair failure instead of silently discarding one. Re-entry with
the same receipt does not expand its frozen coverage. Historical interventions
remain on their original dues; qualification reads the complete ownership set
instead of copying intervention records onto each new owner.

`execution_obligation` records the segment that fired. Current telemetry and retry
use that segment; each original telemetry association resolves through its own
original segment. Process completion records newly due waits on the firing segment.
Settlement, selection and telemetry writers use the same derived owner and exact
latest-attempt fence. History resolves owners outside its display window.
Snapshot readers hold the existing short accounting lock while reading documents,
preventing a mixed view of materialization and the owner commit.

Local tests reconstruct every durable materialization prefix and both sides of
the owner write, exercise interrupted-before-settlement candidate retention,
reject late writers, retain legacy documents without rewriting them, and exclude
cross-Home continuation. A real CLI/Flow regression with simulated external services
exercises closed-candidate recovery, successor coverage and CLI display. Restoring
the pre-settlement record while retaining the simulated publication proves the
next wake reconciles it without another publication or settlement. Manual
intervention on a predecessor still disqualifies a later pair. These are synthetic
proofs, not production publication or the two automatic configured settlements.
Final check results appear once below.

### Delete — do not maintain

- Removed segment-local validation and owner/telemetry/history joins.
- Removed persisted collapse-link writes and copied intervention provenance;
  shared coverage projection owns both relationships.
- Removed the cron unit fixture's obsolete command-argument assertion; durable
  receipt/outcome assertions remain, and scheduled integration exercises dispatch.
- Preserve original dues, candidate/failed proof, exact-attempt fencing, independent
  physical outcomes and the two-distinct-automatic-execution qualification rule.

### Compression review (`02d6b3c00`, after same-Home persistence revision)

No further production reduction selected. Effective model: retained obligation
segment → original due → attempts. Each attempt freezes coverage, candidate,
verification and outcome; its execution segment identifies the wake. Physical
cron receipts and dated repair dispositions remain independent facts. This pass
leaves that model and all public APIs unchanged.

Reviewed accounting persistence, predecessor traversal, coverage derivation and
exact-attempt writers through telemetry, release-history qualification and CLI
text/JSON. Checked the release-history DTO fixture, interruption/legacy/Home-boundary
unit cases, scheduled replacement regression, publisher receipt definitions and
public read-back, plus release README and cron-host documentation. No additional
Swift or Python mirror of ReleaseHistory/ReleaseOpportunity was found.

- The previous duplication is already removed: `covered` owns the relationship;
  `coalesced_into` is derived and omitted from new persisted documents. Deleting
  the public projection would push owner resolution into each consumer. Adding
  separate storage DTOs solely to avoid the save-time omission adds vocabulary.
- `execution_obligation` and the original owner differ on replacement wakes.
  Combining them loses current telemetry/retry context or original candidate
  ownership. Missing execution provenance in retained documents deliberately
  means the original segment; deleting that read path discards durable history.
- `next_due_at` proves original adjacency; the observation-time calendar owns
  retry timing. Historical telemetry associations and current recovery similarly
  answer different questions and cannot share a mutable observation.
- Receipt exit, attempt settlement and publisher stages cannot share a status.
  Prepared artifact hashes and fresh public read-back have separate proof roles.
  PublicReleaseReceipt already inherits ArtifactReceipt; Rust's private proof
  reader validates its subset while retaining the complete JSON evidence.
- ReleaseHistory joins obligations, receipts and dispositions with a computed
  summary. It does not wrap one object. Coverage validation, late-writer fencing
  and intervention qualification are distinct obligations of that join.

Simulated review found no additional coherent deletion on this model path.
The interruption cases retain original failure/candidate evidence and reject
late writers; their synthetic scope remains unchanged. No code changed and no
behavioral tests were rerun. The focused results below remain recorded evidence;
full gate, Jack Heart's review and configured automatic settlements remain open.

## Acceptance counterexamples

### Focused behavioral proof

Required behavioral cases belong in existing cron, doctor, release integration,
Flow and publisher tests; recorded passes retain their original scope. Use real temporary repositories/files/OS locks and simulated
external services. Assert durable outcomes and preserved bytes, not mock calls.

| Counterexample | Required observation |
| --- | --- |
| One on-time firing, unchanged sync, repeated same-minute firing | One due key, preserved activation, one accepted terminal settlement |
| Wake after three due times; release lasts across another due time | Three original due times link to one execution and at most one settlement; the fourth waits for the next wake |
| Crash while recording collapsed misses; retry resumes an incomplete tag | Saved covered keys repair missing links; original candidate owner survives; no second publisher or duplicated settlement |
| Manual run or `cron trigger` near a natural firing | Manual/intervention provenance retained; no fabricated opportunity or qualifying autonomous pair |
| Overlapping scheduled/manual release selection | One target mutation owner; loser is deferred to a real continuation |
| Parent dies while publisher child remains active | No second publisher or cleanup of the active stage |
| Crash before launch, after tag, after public release, before settlement | Unsettled opportunity remains visible; exact same candidate/tag resumes or reconciles once |
| Old attempt returns after retry or terminal settlement | No terminal regression; rejected evidence retained diagnostically |
| Agent exits zero without release evidence | No publication/no-change settlement; mechanical path does not depend on agent prose |
| Successful continuity, failed telemetry | Independent red verification and dated owning disposition; no qualifying success |
| Failed/missing exact-candidate check on no-change or resume | Failure/deferral with evidence, never a bypass |
| Draft release, wrong commit/hash, absent asset, smoke failure | No complete publication settlement; any external effects remain recorded |
| Corrupt JSON or interrupted atomic replacement | Fail with path/cause, preserve last valid evidence; never turn a missing row into no-change |
| Schedule change/removal, timezone/DST, changed Home | No invented/dropped due times; uncertainty explicit where history is absent |
| Staged/unstaged/untracked caller edits and local commits, including overlapping upstream paths | Exact caller HEAD/branch/index/working bytes unchanged after every failure, retry, and success |
| Independent target/repository | No opportunity collision or unintended shared publication lock |

### Configured-path acceptance

- Capture exact installed obligations, placement, executable/source digest,
  timezone, release configuration, required-check inventory and unresolved
  historical failures. Keep pre-observation coverage explicitly incomplete.
- Use the supported release/install/sync path when authorized, preserving the
  09:00/10:00 cadence and existing receipts. The September 28 activation is
  retained evidence, not a substitute for observing the final implementation.
- Observe two adjacent original real dues through two distinct automatic
  executions, at least one published, with all verification and no manual repair.
  No forced trigger, backdated due, collapsed duplicate or nonadjacent green pair.
- Retain exact attempts, command/config digests and immutable subjects, hosted
  workflow/check references, tag/commit, publisher manifests, asset hashes,
  platform smoke and intervention provenance. An absent required check remains
  absent; cross-platform execution cannot be inferred from one host.
- Installed history/doctor must expose every due as accounted, missing or unknown;
  unresolved work needs truthful continuation and failures need dated owners.
  A late assignment remains late. A pending configured firing is a bounded
  declared wait at its observed next due, not an agent sleeping for a day.

Measure due/accounted/unknown/unresolved counts, original timing and delays,
collapsed dues, distinct executions and settlements, failure-disposition age,
and eligible adjacent pairs. Collapsed dues contribute to accounting only.
No retrospective release-health gate enters doctor: release already depends on
telemetry containing doctor, and such a gate would create a dependency cycle.

## Retained evidence and review

All prior designs, slice/review/compression notes and raw evidence remain at
`fe36937faf75151a2f5d7195b32ff708ce335e86:scratch/`. They were clean committed bytes
before curation. In particular, `release-saved-candidate-recovery.md` retains the
four-case Docker recovery proof, inspector survival, queue cases and 30 publisher
passes; `shepherd-assessment-20261002.md` retains the read-only main/run-records
comparison. Its statements that main integration and saved-candidate recovery
were still future work are superseded by the source and steers above. Historical
review verdicts and their narrower source searches do not navigate the current Flow.

`evidence/baseline-summary.json` retains the September 24 00:31 UTC observation:
70 receipts (36 failed telemetry, 33 successful release processes, one failed
release process), published v0.12.19 and missing `agent_turns` probe. Full receipts,
logs, publisher receipt and read projections are available in the same git tree.
The baseline cannot establish a settlement percentage or the two-opportunity KR.
The independent invalid v0.12.30 incident source `8cbd0c5b1c99a12151908c59f923b0128706a34f`
contained canonical release SQL plus `drafts/remove_ask.sql`; the retained
run-records assessment names packaged rejection. Infrastructure memory now
records v0.12.31 publication and supported installation,
plus one-Home acceptance. This supersedes the earlier pending-install observation;
it does not establish this branch's installation or scheduled settlement. No fresh
provider read was made during this reconciliation.

Latest supplied feedback reports focused recovery, publisher, minor-release and
static passes after main merge and CI-repair ownership repair. This reconciliation
reuses those results within their scope; it does not invent a final-tree gate,
hosted outcome, current installed check or configured settlement. The prior Docker
proof used seeded dues and simulated external services with real Git/CLI/locks.

The earlier simulated review identified stale completion claims: assignment
had been conflated with continuation, dated telemetry diagnosis with current
source, and repetitive completed slices with remaining work. The reconciled plan
separates these boundaries and retains the full acceptance criteria. That earlier reconciliation changed prose only; the current implementation and
its focused checks are described above.

Recorded implementation checks: isolated `cargo test -p loopflow --lib ops::cron::` passed 45 tests (changed intervention regression rechecked); `cargo test -p loopflow --test scheduled_release_tests` passed eight cases and the repaired/extended replacement regression passed separately; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` and `lf context --skill implement` passed. Full affected gate, hosted/UI/public proof and configured two-execution acceptance remain outstanding.

Check (October 2 realign): `git diff --check` and `lf context --skill realign` passed; prose-only reconciliation reused recorded focused results; full affected gate and configured acceptance remain deferred.
