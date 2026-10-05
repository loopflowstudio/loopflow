# infrastructure wave memory

## Release follow-through (2026-10-04)

Jack Heart directed Infrastructure to take responsibility for finishing every
release. Release owns the release-specific execution and evidence; Infrastructure
retains responsibility through verified publication and installed acceptance.
A merged version bump, successful agent turn, or queued build is not completion.
Resume interrupted releases through the supported release operation, preserve
exact publication evidence, and surface any unresolved blocker and next action
without requiring Jack to rediscover the gap. Keep progress updates in the
ongoing conversation while recovery is active.

The v0.13.1 version PR merged while v0.13.0 remained the latest published release.
That gap left the Session filtering in PR #1421 absent from the installed Desktop.
Release-specific recovery evidence belongs in [Release memory](release/MEMORY.md).

## Review replacement (2026-10-04)

Jack Heart authorized LOO-377's autonomous repair; PR #1429 merged as c5dc238b0afb
with hosted CI passing. Restart serializes review launch, fences writers and
retains exact stop evidence for retry. Independent reviews remain protected.
LOO-370's replacement Flow finished after recovery; its Task remains open.
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

The Task then could not complete: an earlier restart had stranded its review
under a replaced Flow, an independent Flow had failed, and their Execs and one
provider turn never settled before a machine restart removed the evidence.
Branch repair (PR 2): an Exec or provider turn that began before the last boot
has exited; a review whose Flow ended awaits no one; `lf flow end ID` retires one
stopped Flow by request, keeping its failure and history and inventing no step
result. Live or unknown execution since boot still blocks. Closing LOO-326 needs
an installed release carrying this, then `lf flow end` and `lf task complete`
from outside any `lf --task LOO-326` run, whose own Exec gates completion.

PR #1435 CI at `6c760f285` exposed a release cleanup race: a terminal repair
Exec receipt can precede closure of inherited checkout descriptors. A delayed
launcher reproduces the retained second checkout; cleanup and repair re-entry now
wait up to five seconds for an independently acquired lease and preserve ownership
if it stays held.
The existing cleanup assertion stays intact. This is branch evidence, not an
installed repair or Task settlement.

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

`a60ac0281` fixes retry timing; `02d6b3c00` atomically covers materialized dues
across same-Home segments. Original ownership/provenance survives; firing segments
own telemetry/retry, and Home changes break continuation. Synthetic tests cover
interruption, late writers and recovery without republishing. `95643bd50` adds
calendar/closure overlap continuation and retains old physical receipts. Save
physical failure before accounting; an accounting error must not erase lock loss.
Repeated overlap adds no settlement. Local interruption proofs at `d60d254ef` show
public reconciliation retains target/checkout locks after controller death;
retry preserves the failed cron receipt, candidate, coverage and caller bytes
without republishing. Cron exit cannot establish orphaned verifier lock release.
Candidate-ref/workflow lost acknowledgements, verifier materialization and
post-arm Task compensation have focused synthetic proofs. Gate, Jack Heart's
review and configured settlements remain open.

Release's September 28 incident proves entry points need operation-level recovery:
an agent reported failure successfully, producing a misleading green cron receipt.
Jack's later steer records v0.12.24 publication/install and skill-to-Flow activation
at unchanged 10:00. That supersedes the child's dated pending-activation evidence,
without proving this accounting branch is installed or either qualifying outcome.
Release-specific detail remains in [release memory](release/MEMORY.md).

All 36 telemetry failures, including the original 35, remain dated counterevidence
from September 24. The missing `agent_turns` diagnosis is historical: integrated
scorecard source consumes SessionHistory. The isolated doctor/scorecard Flow
regression proves Recovery cannot replace natural Scheduled evidence. No installed
pass or accepted Intelligence handoff is established; reproduce current failure
before commissioning duplicate analytics repair. Required UI/public proof and
actual automatic settlement observations remain outstanding. The working plan
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

Renamed from `systems` in the 2026-07-08 wave/project/task restructure. Owns dependable self-hosting, verified releases, and architecture minimalism. The configured release schedule and accepted proof obligations govern current work; older nightly/weekly notes below are historical.

Release-specific findings live in [release memory](release/MEMORY.md).

The 2026-09-30 [LOO-298 decisions](#data-model-and-performance-decisions-reconciled-2026-09-30)
supersede older Run-owner, historical-import, pinned-development-Home and
demo-before-landing directions for this cutover. Earlier incident observations
remain evidence of their own versions, not instructions to restore those owners.

## Installed worker recovery (2026-10-02 PDT)

Jack Heart authorized recovery of blocked workers and publication/installation
of the prepared patch. [PR #1415](https://github.com/loopflowstudio/loopflow/pull/1415)
merged as bfc681ee8; [v0.12.32](https://github.com/loopflowstudio/loopflow/releases/tag/v0.12.32)
published from 8c72e591e78a68227255fd86bfff6a939ded5e8b after release
[workflow 37099938065](https://github.com/loopflowstudio/loopflow/actions/runs/37099938065).
The published installer promoted CLI/app after preflight recognized the existing
Home exactly, with all 33 executable references resolving and no migration.
GitHub's API identified v0.12.32 while its public latest redirect still selected
v0.12.31; the checksum-verified published installer with --version v0.12.32
completed installation. No source build was promoted or runtime data hand-edited.

Installed readback confirms LOO-367 resumed the same saved Flow, passed
loop-decide and entered implementation iteration 1. Historical unowned Execs no
longer block resumption; actual current process/Session/Flow ownership still does.
LOO-285's integrated CI-repair child ownership fix passed focused tests and its
saved Flow resumed beyond sync. Cross-segment continuation implementation and
the two distinct automatic settlements remain, including retained telemetry
failures; this operator-triggered release supplies none of that automatic proof.
LOO-292 retains its demo review. LOO-370 owns retired Run-name cleanup.

LOO-295's installed reconciliation attached existing merged
[PR #1283](https://github.com/loopflowstudio/loopflow/pull/1283) to the original
PR row, recording merge 5bcc40fdff810c39885a31b3fbfd6557f49fde93. The Task stayed
open and all eight unrelated scratch/loc-explorer files were byte-identical.
Its current-runtime review/repeat/final-demo acceptance remains unresolved.

A separate retained-data defect remains: `invalid stored landing placement: home`
from store/sqlite/pr_landings.rs::map_landing fails repository reconciliation,
including after LOO-295's successful association, and degrades the CI watcher.
It also interrupted release settlement after #1417 merged. Supported re-entry
recognized that existing merge and completed the same v0.12.32; no manual
record/ref/worktree deletion occurred. The old placement reader/migration needs
repair with preserved ownership evidence. This is not an empty/healthy backlog.

Jack questioned serial releases. The operator committed to collecting further
recovery defects into one repair batch, with installed-state preflight, instead
of starting another immediate release for each discovery; this is an operating
adjustment, not a claim that Jack approved a new release policy.

Checks: PR and merge-queue CI, candidate workflow and publisher passed; installed
LOO-367 progression and LOO-295 association/file-preservation readback passed.

## Optional chapters and Task workflows (2026-10-02)

The current briefs for [LOO-366](https://linear.app/loopflow/issue/LOO-366/keep-every-wave-ready-for-work-without-requiring-a-chapter)
and [LOO-367](https://linear.app/loopflow/issue/LOO-367/start-and-finish-tasks-without-unrelated-workflow-prerequisites),
read through `lf wave status infrastructure --json` on October 2, record Jack
Heart's accepted direction: ordinary Projects do not require chapters or a
default Flow; Tasks do not require a managed Flow or delivery workflow merely
to exist and record an outcome. Chapters remain optional coordinated resets.
This supersedes older mandatory-chapter/default-Flow assumptions in this memory;
it does not establish implementation or authorize a repository reset.

LOO-366 owns explicit Project ensure/adoption and optional resets. LOO-367 owns
Task admission/completion and preserves unresolved delivery and unfinished work.
Keep that interface distinct: observational reads must not create Projects, and
Task operations must not acquire unrelated coordination prerequisites. The
October 2 Task detail showed LOO-367 starting implementation after Jack's design
review; LOO-366 remained planning-only (`lf task status` found no local Task).
Retain the current serial implementation allocation rather than launch a second
worker during this pass. Neither unchecked current Project KR has outcome proof;
the metric portfolio and Project metric targets were empty in this observation.

## One migration draft per Task (LOO-344, branch evidence 2026-10-01)

Jack Heart, 2026-09-30: only this machine is a client. A Task keeps one draft,
`drafts/<name>.sql`, and edits it in place until landing; unreleased drafts on
main are edited rather than undone by a later draft. Draft ids, `-- name:`/`-- id:`
headers and the `development_migrations` receipt ledger are removed, so the
applied-draft names, IDs and checksums in the LOO-321 diagnostics below no longer
exist. Custom Homes keep exact-schema validation and stay disposable.
`-- depends_on:` still orders drafts across Tasks at the release cut. Mechanics
live in [MIGRATIONS.md](../../rust/loopflow/src/store/MIGRATIONS.md). Branch
evidence only; not shipped.

## One main Home (LOO-342, curated 2026-10-04)

Jack Heart approved the one-Home cutover: ordinary CLI commands, Task workers,
Flow steps and agent tools use the installed CLI and `~/.lf`; an explicit
`LF_HOME` experiment initializes once and needs a fresh directory after schema
changes. [PR #1381](https://github.com/loopflowstudio/loopflow/pull/1381) merged
at `6c73356074c4`; installed routing acceptance passed on v0.12.31 and the Task
reads done (see the 2026-10-04 entry above). Current behavior belongs in
[CLI docs](../../docs/lf.md#use-one-home) and [Homes](../../docs/architecture/homes.md#one-main-home).
The dated cleanup receipts, v0.12.29–v0.12.31 release recovery and the exact
configured checks are in [the pre-curation memory](https://github.com/loopflowstudio/loopflow/blob/cd344891b611914adf44eeb53844193d7a987d2c/wave/infrastructure/MEMORY.md#one-main-home-loo-342-branch-evidence-2026-09-30).

### Legacy retirement completed (2026-10-04)

Jack Heart explicitly authorized clearing all legacy stores and their process
owners. Fresh `lsof` identified 15 owners of the four retained installed stores.
Current `lf monitor prune` had no registered orphan targets; the old `ask cancel`
command failed reading the current installation manifest (`work_dispositions`
missing). Exact executable/start-time checks and process ancestry bounded the
shutdown to those owners, their matching legacy wrappers and descendants.
All 37 processes exited after SIGTERM; no SIGKILL was required. The current
Session and main-Home processes were outside that set.

The four stores moved intact to `~/.lf-retired/20261004T161815Z/installed/`.
The unused root database and two demo databases moved with the remaining legacy
root to `remaining-home/` in the same archive. Its `retirement.json` records
process identities, signals and source/destination paths. The seven earlier
snapshots remain at `~/.lf-retired/20261002T191224Z/worktrees/`. Archives preserve
history and consume disk space; retirement does not mean erasure.

Verification: all 37 recorded process identities exited, legacy handles reached
zero, `~/.lf-dev` no longer exists, and installed `lf home id` still returns
`home_39860354aaca640c2ccb50bf6ca609d8`; prune inspection reports zero errors.
LOO-342 was already marked done in the October 4 status read. This retirement
supersedes the pending-store findings above and closes the remaining acceptance
gap after the October 2 installed routing checks.

## Worktree listing and fenced dispatch (LOO-375, branch evidence 2026-10-04)

Jack Heart reported `lf wt list` at 44 s, 17 s after a deadlocked writer was
killed. Two separate causes, both measured on this branch; neither is shipped.

- **Process count, not Git work, was the listing cost.** About 370 Git processes
  ran mostly one after another at 25–45 ms each to start. Batched ref reads,
  concurrent `status`, one GitHub call and answers remembered per commit pair in
  `.git/lf-commit-facts` leave about 66. The remaining floor is one GitHub round
  trip (about 1 s here), not local work. Numbers and method:
  [report](../../scripts/benchmarks/wt-list/README.md).
- **Never hold the Session fence while waiting on the runtime.** A fenced write
  held the store mutex and SQLite's write lock inside `block_on`; the reader task
  waited for that mutex on a runtime worker, so nothing drove the socket or the
  timeout. The write is now timed on its own thread and store work leaves the
  worker first (`harness/dispatch.rs`). OpenCode's fenced HTTP post still holds
  the fence up to 10 s on its own client; that is bounded, not a cycle.
- **An Exec waits once for a contended store, not once per receipt.** A held
  write lock now costs one 15 s wait (17.4 s measured, was 33.2 s) and a warned,
  unrecorded Exec; the start receipt still precedes the command, because a child
  must find its parent's row. Timing therefore lives beside the store, in
  `<Home>/perf/wt-list.jsonl`: Exec rows lose exactly the slowest samples.
- **Jack Heart's delivery contract:** land after autonomous checks and honest
  benchmarks; on-machine experience is post-merge validation. `lf wt timing`
  reports count, median/p95, failures and version from real invocations. It has
  no ordinary-use samples until a release carrying it is installed; staged
  numbers came from a host at load 30–90. Reading it after install is what
  remains before completion, so landing this PR must leave the Task open: a
  Flow ending in `land -c` contradicts a contract with post-merge evidence.
- **A closed Session with a confirmed-dead provider no longer blocks Task
  admission or completion** when its turn lacks a completion receipt (Jack
  authorized this in the same PR). Closure alone is not enough: live or unknown
  providers still block.

## Environment variables (LOO-341, branch evidence 2026-10-01)

Jack Heart requested an audit of every `LF_*` variable against the policy it
implements. The inventory is [Environment](../../docs/architecture/environment.md).
`LF_HOME` is the only Home selector and its database is always
`$LF_HOME/loopflow.db`; `LF_DB_PATH` and the `LF_CONTROL_*` trio are removed, with
the other names nothing read. One list of Exec-context names drives both the
session shell's `unset` and the tmux client's environment, because a tmux server
copies its first client's environment into every later session. Fixtures open
their store at the Home's fixed path. Not reviewed by Jack; branch evidence only.
`LF_RUN_ID` presence still decides three behaviours without validating the Run.

## Task worktree membership (LOO-358, branch evidence 2026-09-30)

Jack Heart selected the Task's checkout as its general work set: every
AgentSession, FlowSession and Exec there, plus explicit binds. The shared Rust
SQLite reader supplies Task status and Desktop membership; the app does not
reconstruct ownership from paths. Membership is additive, includes descendants
at component boundaries and closed history, and survives a missing checkout.
It changes neither recorded usage attribution nor process/Flow authority.

The managed Flow remains one marked member for worker progression, claims,
Task review settlement and worker delivery authority. Independent unfinished
Flows, pending Ask/review Sessions and live or unresolved Execs preserve work
during completion, recovery and cleanup. General membership cannot settle or
signal them. Current mechanics live in the architecture reference; this branch
entry is not evidence of shipment or configured Desktop acceptance.

## Synced planning integration (LOO-334, 2026-09-30)

Main's landed current-state cutover supersedes the earlier intermediate-schema
bridge below. Planning now migrates from released Session ownership; it does not
restore the historical importer or old execution drafts. Managed validation uses
the single FlowSession driver, after native recovery and completed-Task cleanup.
Task adoption retains branch/worktree/PR identity without inferring Flow progress.

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
mechanics belong to docs/lf.md and docs/architecture/homes.md; superseded command
names and store-selection policies are historical evidence, not instructions.

## Managed account identity (LOO-339, branch evidence 2026-09-30)

Jack Heart selected the identity core for [LOO-339](https://linear.app/loopflow/issue/LOO-339)
delivery and authorized landing and a patch release without review. The earlier
expanded scope is superseded: [LOO-340](https://linear.app/loopflow/issue/LOO-340)
owns shared account state across Homes, current status by default, browser
suppression, Claude cached identity/routing, Flow account bundles and reset credits.
[LOO-338](https://linear.app/loopflow/issue/LOO-338) owns the command rename;
this branch retains `lf auth`. Authorization is not evidence of shipment.

The [design](https://github.com/loopflowstudio/loopflow/blob/8973f689a9e11a83b2dfa467fecddd095940d435/scratch/verify-managed-account-identity-on.md)
and [review and gate evidence](https://github.com/loopflowstudio/loopflow/blob/8973f689a9e11a83b2dfa467fecddd095940d435/scratch/verify-managed-account-identity-on-review.md)
are preserved in local history before scratch clearing; remote availability was
not checked. Current behavior belongs in [subscriptions](../../docs/subscriptions.md).

- **Usage acceptance cannot establish the intended login.** The incident found
  wrong native logins displayed as verified under configured labels. Shared
  validation in `provider_account/identity.rs` compares expected email and
  per-user subject, never shared workspace identity. Codex cached status, routing
  and readiness inspect current credentials; connect/import and explicit
  verification also compare `account/read` email with the file identity using
  file-store mode. Claude connect/import and verification use profile email/UUID,
  never stale `.claude.json`.
  Public auth tests cover disagreement, duplicates and relabel refusal; identity
  tests retain shared-workspace/different-user acceptance.
- **Reconnect must preserve the live login while authorization waits.** The
  existing staged connect path installs only after identity and duplicate checks;
  its paused-browser regression reads the unchanged live credential before
  completion. The provider-directory install lock serializes Loopflow installs,
  not native provider writers. This does not prove refresh coordination or sole
  browser ownership: Codex can still open an extra tab on macOS.
- **An unavailable identity service is not credential rejection.** `poll_codex`
  classifies both account and usage RPC errors before decoding identity. The
  public `account_read_failure_preserves_credentials_unless_revoked` regression
  proves a 500 preserves connected state while a 401 records missing credentials,
  retaining other account facts. A plan is separate from quota: observed Pro
  precedes Plus only among healthy automatic candidates; explicit selection and
  Session affinity remain authoritative. JSON windows retain dated observations;
  expired text windows show unknown, not new capacity.
- **Fixture isolation includes executable selection.** Gate launched real Claude
  because the harness prepended inherited `LF_BIN`'s directory ahead of stubs.
  [TESTING.md](../../TESTING.md#test-without-an-installed-loopflow) now requires
  clearing inherited `LF_*` authority and pinning the compiled source CLI for
  managed-Run gate invocations. An isolated Home alone is insufficient. The
  interrupted run remains failed evidence; accidental native credential reads
  or refresh effects were not audited.

Recorded final isolated materialized Rust checks passed 2,192 tests (13 skipped),
including seven public auth tests; website passed 78 (three skipped). Formatting,
all-target Clippy, architecture, migration history and fresh-Home JSON checks
passed. The original gate receipt remains failed after a screenshot timeout;
separate successful checks do not rewrite it or provide a reusable final-tree
receipt. This curation reruns no behavioral checks. Synthetic proofs establish
neither live OAuth nor installed acceptance. State remains Home-local; recorded
replay cannot recover another Home's custom database path. LOO-340 owns shared
authority; this curation authorizes no installed-store repair.

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
profile targeting and live Claude/Codex windows remain unproved. The configured
Claude probe returned `invalid_grant`. Native directory locks and Keychain writes
remain outside Loopflow's flock/atomic-write proof. The copied-Home demo retained
schema and credential-state discrepancies; copies still reference original
credential homes. No source binary may migrate the installed Home. Synthetic
gate passes, an intervened SSH-fetch fixture and empty-Home checks establish
neither these live outcomes nor installed acceptance; historical receipts are not
reusable for a changed tree. Exported API migration docs do not prove external
consumers migrated.

## Task deletion and command ownership (LOO-305, branch evidence 2026-09-27)

[Task removal and command consolidation · LOO-305](https://linear.app/loopflow/issue/LOO-305)
owns [Task command consolidation · PR #1302](https://github.com/loopflowstudio/loopflow/pull/1302).
Jack's final scope is provider/local deletion, removal of `pm`/`work`, compression
and delivery through the saved Flow. Execution settlement remains deferred.
The accepted command map and detailed proofs survive in local commit
`4a14c0a47dc6e04be9668fb72b737828565a931d`:
[design](https://github.com/loopflowstudio/loopflow/blob/4a14c0a47dc6e04be9668fb72b737828565a931d/scratch/cancel-linear-issues-through-lf.md),
[implementation evidence](https://github.com/loopflowstudio/loopflow/blob/4a14c0a47dc6e04be9668fb72b737828565a931d/scratch/implementation-evidence.md),
[demo](https://github.com/loopflowstudio/loopflow/blob/4a14c0a47dc6e04be9668fb72b737828565a931d/scratch/task-deletion-installed-demo.md),
and [compression/rebase proof](https://github.com/loopflowstudio/loopflow/blob/4a14c0a47dc6e04be9668fb72b737828565a931d/scratch/deletion-namespace-compression.md).
Remote availability was not checked. These are branch results, not shipment.

- **Commands follow their objects.** Task owns create/status/edit/comment/run/
  complete/delete; Wave owns connection, sync and placement; repo owns reteam
  and webhooks; `doctor --planning` owns diagnostics. `pm`/`work` and Task
  abandon/recover have no aliases. Older command spellings below are historical.
  Current mechanics live in [CLI docs](../../docs/lf.md) and
  [planning architecture](../../docs/architecture/planning.md).
- **Identity, confirmation and outcome differ.** Observed issue identity survives
  refresh/chapter replacement but never authorizes a new provider mutation.
  Fresh ownership authorizes deletion; acknowledgement or explicit trash evidence
  confirms it. Missing membership proves neither. Manual confirmation atomically
  retires Ready Tasks and preserves terminal times, Done outcomes, PRs and Git.
  Chapter confirmation shares the insertion while retaining its own classification
  and receipt transaction. Stale snapshots cannot erase positive confirmation;
  old abandonment/applied receipts cannot manufacture it.
- **Prepare before filing.** Planning-only creation allocates no checkout or
  Task row and needs no agent account. Execution creation consumes validated
  placement, pinned base, Flow/auth and selected Task agent. Retry markers survive
  notes edits and reuse persisted identity/title. Post-create allocation failure
  remains recovery work; do not replace preflight with compensating deletion.
- **Completion and planning have separate writers.** Check provider terminal
  conflicts before new local Done, including the confirming refresh. Retain merged
  PR evidence for retries; narrow writes preserve refreshed planning and execution
  facts. Repeated completion preserves its original event/time.
- **Checkout identity is its own branch.** Upstream is tracking information and
  may name main or a stack parent. New branches disable automatic base tracking;
  publication establishes their own origin branch. Historical Run attribution
  resolves retained identity independently of launch eligibility. Blank participant
  overrides fall through to configured/Git names; markerless steers use the
  provider author without rewriting comments.
- **Removal is not termination.** A completed capture and released claim can
  leave a live Exec without an exact Task join. Native per-Run locking cannot
  establish Task-wide admission or landing settlement. The retained reproduction
  and deferred scope are in the design's linked evidence; complete Session/Run/
  activity disappearance and process settlement remain unproven.

The recorded Linux real-binary deletion proof used synthetic Linear and disposable
stores; macOS platform TLS ignored its child-only CA setting. TESTING.md owns
that fixture contract. The source demo deleted LOO-299–302 against configured
Linear, retried successfully and verified refreshed Wave/roadmap absence. It also
advanced installed-Home drafts and broke the older installed CLI. Jack's later
steer forbids branch-binary access to that Home and any branch promotion;
[Installation incident · LOO-321](https://linear.app/loopflow/issue/LOO-321) owns
recovery. The demo's promotion handoff is superseded. All further source proofs
use disposable Homes with inherited LF_* authority removed.

Rebase retained main's shortened Flow and Task-agent semantics. Compression moved
Task scenarios out of OAuth tests and removed duplicated setup/cases, without
counting moved lines as deleted. Recorded focused checks and Clippy passed;
no full gate or installed acceptance follows. This curation changes no chapter,
Task disposition or delivery state.

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

Jack's rule, verbatim: "The main user objects should line up with the main
tables in the DB and when we see stuff like this where a main record is
actually a union over 4 things, we should be suspicious." The trigger was a
2026-09-26 product-first review of Session/Run/Task/Wave: a Session was four read-time
projections over four stores (Run dir, `task_flow_positions`,
`human-sessions/*.json`, `flows/*/position.json`); Run→Task is a `task:`
string in a ranked subject list with a two-value source, mirrored into a
Task event because the Run could not be queried by Task; every post-launch fact
(name, completion, attachment) became a sidecar beside the manifest. Reads
reused the launch resolver, so a bound Session became an orphan when its
Task's PR merged. The store's `runs` table had the Task FK and no writer.

LOO-298 ([PR #1296](https://github.com/loopflowstudio/loopflow/pull/1296)) implements
the three-owner model in this branch. Run no longer exists as a product object
or table. Current contracts belong in [architecture-reference](../../docs/architecture-reference.md)
and [CLI reference](../../docs/lf-reference.md); branch code is not installed acceptance.

Jack Heart authorized autonomous landing on 2026-09-30: “try to do this all
autonomously, no need to review with me.” Land #1296 as one PR after technical
verification, without a demo or review wait. The independent resource-recovery,
publication-continuity and resident-Wave cuts already landed as #1358, #1359 and
#1360; local main history records them. Exec/Chapter extraction would remove
only about 10% and requires manual cutting, so the old decomposition is rejected.
Jack selected merging main, not rebasing; `e3a2c7e2c` integrated #1360. Landing
authorization is not evidence that #1296 merged or that a release migrated data.

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
- **Chapters follow Project status.** Each Wave's one In Progress Linear Project
  shares the chapter name and owns the default Flow; there is no Chapter table
  or plan packet. Preserve started Tasks and expire only proven untouched backlog.
  Synthetic second-Home proofs exercise retries and missing evidence; they do
  not imply a second deployed client or a distributed transaction.

**Integrated gate, 2026-09-30.** [Gate repairs and evidence](https://github.com/loopflowstudio/loopflow/blob/5dee46ca8b8d8282a137b32c5b8d786af7c9cb91/scratch/integrated-gate.md)
are checkpointed locally; publication/merge is not established by this entry.
The release-materialized full Rust run recorded 2,004 passes, seven failures and
17 skips. Focused repair runs cover all seven failures; the original receipt
remains failed. Python passed 310 tests and website 78 (three skipped). Swift's
291-case run had one obsolete import fixture; its eight-case observation repair,
app/runner builds, boundary check and eight distinct fixture captures passed.
Formatting, final all-target Clippy, architecture and immutable migration checks
passed. Required hosted CI still owns the final landing candidate.

The runtime finding was cursor order: stable-ID Session pages must retain ID
order through projection, or renamed titles can repeat/skip records. Stacking
fixtures must use the dedicated transaction and establish a published parent;
generic PR updates intentionally cannot alter parentage. A bad saved-sync fixture
launched a real conflict agent in its disposable repository before that repair;
output reports no push, but native credential effects were not audited. Provider
stubs now contain that failure path. This does not establish configured acceptance.

At `5dee46ca8`, the integrated production-prefix estimate against `12013dae4`
is **+6,202 lines** (+22,792 / −16,590), including SQL and excluding tests/docs;
it is not a net reduction or a parsed statement count. The retained density
measurement still names its earlier candidate. Installed conversion remains
subject to the frozen-snapshot/quiescence obligations above; this gate neither
migrates nor promotes.

Lessons from implementing it (2026-09-29–30):

- Hosted CI stops at the first failure; one round showed 907 of 2,010 tests
  unrun. Jack's 2026-09-30 cadence supersedes per-publication full local runs:
  use focused proofs plus hosted CI between items; run the full local Rust
  matrix without fail-fast before the final gate.
- Typed CLI discovery must preserve saved execution: inventory flags reach the
  SQL reader, selected boundaries resolve captured Skills before the mutable
  catalog, and Ask escapes reserved Skill names. The main integration has 13
  focused passes for these paths; it is not configured-provider acceptance.
- Automatic skill checkpointing must use the same Work binding reader as
  execution: explicit command `--as`, then checkout, then inherited `LF_AS`.
  Checking explicit flags alone can commit another contributor's edits in a
  Task checkout. The repair retains the managed Flow and HEAD; its fixture must
  establish shared skill content before changing branches, without relying on
  an incidental checkpoint from another launch.
- The pinned 0.12.23 worker's output classifier read a quoted sentence in a
  scratch note as a capability denial. The source fix is on the branch; resume
  with the actual cause until workers run a release carrying it.
- Task workers had no transient retry or account failover, so one hitting a
  provider limit stopped instead of switching accounts. Converging on the direct
  path fixes it.
- A worker's claim named the process that launched it, not the worker, so stop
  and liveness targeted the wrong pid (fixed in `5bd311697`).

Compression checkpoint `38d4e6d8a` retains one proof per final behavior. Its local
logs record 44/46 affected passes, then both failed Session cases passing focused
repairs, 17 Chapter passes, two native-ownership passes and build/final Clippy
success. The direct SIGINT proof observes owned-child exit and Exec interruption;
the retained Task-cancellation settlement case remains distinct. These observations
are not a full final-tree gate, configured-provider acceptance or installed
conversion. This realign reruns no behavioral suite. Dense cold/warm CLI timing
and final documentation/gate reconciliation remain with the working plan.

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

Staging gotcha: `install.py local --skip cargo` bundled a stale `lf`, and the
store gate keys on the registered installation path, not the bytes, so a demo
app must route through the installed `lf` (`LoopflowDevControl.json` →
`lf_path`) or be promoted.

## Continuation, delivery and authority lessons (curated 2026-10-02)

The detailed September 23–25 observations, historical schemas, test counts and
counterexamples remain in [the pre-curation memory](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#continuation-and-recovery-lessons-curated-2026-09-25).
The September 30 model/cutover decisions above supersede Run, Ask, Project-cursor
and private-store mechanisms there. Archiving detail does not establish remaining
acceptance or authorize deleting execution history.

- LOO-295 owns continuation; LOO-296 broader restoration. The live demonstration
  required manual Flow/native-Home corrections and did not prove automatic
  propagation. Current Task review → decision → repeat → final-demo acceptance
  remains unproved. OAuth-injection removal fixed one login rejection, not all
  ambient-account/lease questions. Prior simulated suites and isolated reruns
  were not a green configured default gate.
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

The `task-viewer` branch removes the unused prompt-direction feature end to end
and collapses unchecked gather/render wrappers to `PromptComponents` and `String`.
The context-specific contract is in [Intelligence memory](../intelligence/MEMORY.md#prompt-assembly-reduction-branch-evidence-2026-09-24).
Keep reductions tied to actual ownership: `WorkCatalog` preserves historical
identity lookup, `TaskExecutionSnapshot` distinguishes current execution from
durable Work disposition, and `PreparedLaunchPrompt` carries consumed evidence.
None is interchangeable with the deleted wrappers. LOO-287 still owns the broader
architecture pass and its real weekly observations; local deletion and passing
checks do not complete that Task or establish its KR.

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

Observed October 4 with published 0.13.0:

- **Natural login catch-up.** v0.13.0 published at 00:20 PDT while the laptop
  was asleep, then off after a 1% battery shutdown. `com.loopflow.refresh` ran
  at the 12:04 login and promoted 0.12.32 → 0.13.0 with exact-store preflight
  and no migration (refresh.log, switch-49b58550 receipt).
- **Real calendar firing.** The 5min cadence fired at 20:10 and 20:15 through
  launchd itself (runs 1 → 3); weekly was then restored byte-identical and
  idempotent. No Monday 09:00 firing or sleep-coalesced wake run has been
  observed; neither is Loopflow code. Read refresh.log after October 5 09:00.
- **Complete artifacts.** Receipt CLI/app/helper hashes, the entry gate, the
  notarized signature and /Applications 0.13.0 all match. Repeat `lf install`
  changes no receipt, Task, PR, Flow or Wave identity.
- **Fresh published path.** Ubuntu 24.04 container without Git: public installer,
  repeat without download, and missing-entry repair retain the Home identity.
- **Checkout updates.** Sandbox with the installed binary: stale main with an
  unpublished commit plus staged/modified/untracked bytes catches up, repeats
  as a no-op, picks up advanced upstream, bases a sibling on it, and a sibling
  sync refreshes canonical main first. Clean main fast-forwards. The real main
  checkout was already current; its incident path was not replayed there.

Unresolved, not blocking: the currency probe (`--version` plus a 30 s preflight)
returned not-current once at load ~14, starting a redundant download; preflight
measured 6–13 s then, so timeout is a hypothesis. Reloading the schedule killed
that run mid-install and the installation stayed intact. A hand-truncated entry
gate is not healed by reinstall (gate writes are atomic, so only tampering
produces it). Interactive app acceptance was not exercised beyond the running
0.13.0 app.

## Shipped history

Historical installation, rebase/placement, PM, OAuth and cron delivery records
remain in [main's preserved memory](https://github.com/loopflowstudio/loopflow/blob/52ab4a4a5cf1ec3c24b019d5cee3a1c782a30d9b/wave/infrastructure/MEMORY.md#shipped).
Current command, Task ownership and installation contracts above supersede their
old names and execution models. Cron continuity judges each latest due interval
against an exact scheduled receipt; manual receipts do not prove firing, while
failed scheduled targets do. Historical gap days do not keep later telemetry red.

## Gotchas

- **`scripts/test.py --all` cannot green the Loopflow UI suite headlessly** (filed). `xcodebuild` runs 304 app/unit tests to a pass, then `LoopflowUITests-Runner` hangs before establishing its connection and Xcode exits 65. Reproduced with a fresh `derivedDataPath`, so it is not a stale-cache artifact. Treat a `--all` UI failure as unproven, not as a regression, until the runner hang is fixed.
- **Dotted-root vs dotted-ancestry collision — RESOLVED** by the WaveId decoupling: the dir is a flat `.`-chain, the remote branch carries `/`+author, and ancestry is read from the `Run` record, not the string. The old `branch_names.schema` grammar that caused it is gone.
- **Run `cargo test` to completion before trusting a green-looking suite.** A failing lib target makes cargo skip every later target, so lib failures mask bin failures — two `bin/lf.rs` tests naming a deleted command had never run at all.
- **Rust compilation does not validate SQLite column names.** Runtime SQL whose shape depends on a released schema must be shared with a behavior test that prepares and executes it against the materialized migration head. Epoch Work ownership is three exclusive foreign keys (`wave_id`, `project_id`, `task_id`); generic kind/id belongs to explicit routes such as synchronous cross-Work questions, not to Epochs.
- **Source history must reconstruct every applied release frontier** (learned 2026-07-20). One pre-schema-closure local promotion embedded a test-materialized `0.12.4` batch and advanced the shared store while git retained the ten source drafts and omitted the canonical file. Recovery preserved the database, extracted the canonical bytes from the retained immutable binary, matched their checksum to `schema_migrations`, registered the batch, and removed only byte-identical drafts. If a store is ahead by an unknown migration, retain state and old binary bytes; prove the checksum before ratifying history. Since #1123, draft-bearing candidates fail promotion even at an exact frontier, while a schema-complete exact-frontier CLI repair may safely activate with live Runs because it writes no migration.
- **Tests must survive draft migration materialization** (learned 2026-07-21).
  Release-equivalent Rust tests delete ordinal-free drafts and compile the
  generated canonical batch. Test fixtures resolve migration SQL by its draft
  marker through `migration_sql_for_test`; an `include_str!` pointing directly
  at `migrations/drafts/` passes locally and fails the release tree at compile
  time.
- **Ordinary-PR integration tests inherit Task authority inside a worker.** Scrub `LF_RUN_CONTEXT` (plus its lease/invocation companions) when a fixture deliberately represents a non-Task repository. A missing registry while Run context is present is the intended fail-closed behavior, not a commit/push regression.
- **Concurrent editing corrupts a file; concurrent rebasing corrupts history.** Two drivers sharing one worktree shared its `rebase-merge` state dir: conflicts resolved themselves between one command and the next, and `done` advanced 6→22 with no `--continue` from the losing session. Nothing was lost that time. Check for a live agent before working — or rebasing — a wave worktree; the driver that owns the worktree owns its `.git` sequencer.
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

The [pre-curation backlog](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#earlier-follow-ups-reselect-through-the-accepted-chapter)
retains historical reduction, Cadenza parity, host bootstrap, release feedback,
upgrade preservation, instrumentation and replication suggestions. These are
not current authorization or an instruction to revive retired Project/Run
owners. Rebase-efficiency follow-ups were resolved by PR #818. Measure actual
command drift, avoidable agent rebases and post-land repairs before tuning policy;
the proposed synthetic workload harness remains unbuilt. Jack Heart's July 6
“up/down 5ths” referent remains unresolved and deferred, not dropped.

## Direct invocation and large inputs (2026-09-25)

Work selectors give direct skills/flows attribution, context and placement;
`lf task run` owns the managed Task Flow. Direct bound contributions receive
fresh scratch and leave checkpointing to their caller. Bare names prefer skills;
explicit verbs resolve their own kind. Reuse the skill-to-invocation loader,
without one-skill wrappers or name-specific dispatch. Started is written at
explicit interactive/headless CLI dispatch, after capture and before provider
launch. Generic capture stays registry-independent: putting Started there broke
the unavailable-registry regression. Read-only Work resolution and unopened review
preparation never record execution. LOO-298's derivation from Run rows replaces
this write only when that model is implemented.

Recursive scratch exceeded both argv capacity and a provider input limit.
Claude batch input uses text stdin backed by an anonymous file, with system
instructions in the existing context file; captured and streamed output use the
same launch path. Curate scratch instead of silently truncating instructions.
The observed Codex rejected `turn/start` remained waiting; that driver failure
is still unresolved, and Claude's working input path does not establish a fix.
