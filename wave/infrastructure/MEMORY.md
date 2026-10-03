# infrastructure wave memory

Renamed from `systems` in the 2026-07-08 wave/project/task restructure. Owns dependable self-hosting, verified releases, and architecture minimalism. The configured release schedule and accepted proof obligations govern current work; older nightly/weekly notes below are historical.

Release-specific findings live in [release memory](release/MEMORY.md).

The 2026-09-30 [LOO-298 decisions](#data-model-and-performance-decisions-reconciled-2026-09-30)
supersede older Run-owner, historical-import, pinned-development-Home and
demo-before-landing directions for this cutover. Earlier incident observations
remain evidence of their own versions, not instructions to restore those owners.

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

## One main Home (LOO-342, branch evidence 2026-09-30)

Jack Heart approved shipping the one-Home cutover. Ordinary CLI commands, Task
workers, Flow steps and agent tools use the installed CLI and `~/.lf`; explicit
`LF_HOME` experiments initialize once and require a fresh directory after schema
changes. This supersedes the older branch-copy, development-promotion and retained
development-store contracts below. Published installation and main-store migration
recovery remain. Current behavior belongs in [CLI docs](../../docs/lf.md#use-one-home)
and [Homes](../../docs/architecture/homes.md#one-main-home). Approval and branch
verification do not establish release or installed acceptance.

Jack deferred remaining side-store retirement until after release and after their
processes settle. The 2026-09-30 cleanup receipt records 14 installed stores and
415 worktree snapshots removed, with these five paths beneath `~/.lf-dev` retained
because they had live file handles: `installed/local-04115a69e0c34b198bf110976b32f390`,
`installed/local-0912e9bdc1194a4c9774b060dadd428d`,
`installed/local-6bd36934695b4eb98083e81e13aad4e7`,
`installed/local-be852452823d43d7b7fde663651a7590`, and
`worktrees/loopflow-growth-thoughts-1c80b40d4504`. These are dated observations;
reinspect live ownership before cleanup. Older running builds can recreate side
stores until the published cutover. No active database was removed or process killed.

October 2 readback established #1381 merged at
`6c73356074c47a90491c1ed4de8e063d85eb3a30`, with LOO-342 still open at its
saved delivery boundary. Initial installed acceptance used pre-cutover v0.12.29
and therefore could not test the merge; the later v0.12.31 checks below supersede
that attempt. Full dated receipts remain at
`32413d0d90e4eec1c48d7950963cf27837a0fab0:wave/infrastructure/MEMORY.md`.

Seven inactive worktree snapshots, including the retained growth-thoughts checkout,
were individually rechecked and moved intact to
`~/.lf-retired/20261002T191224Z/worktrees/`; `retirement.json` records exact paths.
This retired routing locations, not history or disk usage. The four live stores
above were untouched. Reinspect ownership before retirement; remaining obligations
do not justify recreating a PR or treating a generic next action as completion.

### Release and acceptance recovery (2026-10-02)

Jack Heart authorized publishing the patch, installation, and the remaining
one-Home acceptance. [PR #1406](https://github.com/loopflowstudio/loopflow/pull/1406)
repaired the publisher's installed-CLI command mismatch: staging and finalization
use `lf release publish`. Six publisher tests and hosted checks passed, and
v0.12.30 publication completed through `lf release run patch`.

Installed upgrade refused v0.12.30. Its Task-checkout guard opened the old shared
schema before migration; an isolated preflight also proved the published binary
still embedded the uncut `remove_ask` draft. Main's #1402 removes the unrelated
Task guard. The v0.12.31 release batch includes `remove_ask` and its candidate tree
has no SQL drafts. Publication is not installation acceptance; v0.12.29 remained
selected after both refused install attempts. Do not manually advance the Home
or promote a source build to work around these failures.

The v0.12.31 queued run
[37072469684](https://github.com/loopflowstudio/loopflow/actions/runs/37072469684)
also exposed missing terminal outcome history after capture completion. The
recorder queued terminal observations with best-effort telemetry and drained for
only 250 ms. [PR #1409](https://github.com/loopflowstudio/loopflow/pull/1409), now
merged, persists terminal outcomes synchronously through the existing Session
owner, preserves the original receipt on retry, and leaves stream telemetry
asynchronous. Prepared captures without an admitted Session remain valid. The
regression disables the recorder, completes twice, removes artifacts, then reads
the outcome from SQLite; 28 Session-record tests, the scorecard regression and
all-target Clippy passed, with the regression passing again after sync.

The first v0.12.31 candidate built successfully, but artifact download timed out.
Re-entry selected newer merged fixes under the same version through release
[PR #1411](https://github.com/loopflowstudio/loopflow/pull/1411). An exact candidate
ref creation race also recovered through re-entry, without manually deleting a
ref or generated worktree. [v0.12.31](https://github.com/loopflowstudio/loopflow/releases/tag/v0.12.31)
then published from `a278d6bc1bd4373f78f27027b8ae249100ef14d3` after
[workflow 37077794913](https://github.com/loopflowstudio/loopflow/actions/runs/37077794913)
and signed preparation passed. `lf install` successfully migrated the main Home
from 0.12.29 through 0.12.31; all 33 executable references resolved in preflight.
The CLI reports 0.12.31 and promotion installed the matching macOS app.

Configured checks passed on the installed release:

- Installed and current validation-only source CLIs return the same main Home
  identity, including with a stale `LF_BIN` value.
- A fresh explicit `LF_HOME` remains empty on initial and repeated reads, and
  the source CLI reads that same experiment without importing main's data.
- Installed and source CLIs each complete `sync --plan` as a nested Flow
  operation on both main and experimental Homes. Both experimental child
  success receipts are in the experimental database.
- After an intentional schema change confined to the experiment, the valid
  `lf monitor list --json` command refuses it, explains disposal, and leaves
  the schema unchanged without a backup or repair. An earlier probe used the
  retired `exec` command and was superseded by this valid-command check.
- An agent-issued `lf home id` uses the published executable, succeeds on main,
  and retains this Session's attribution (`via_agent: true`). LOO-342's Task,
  issue, checkout, PR ids and saved Flow invocation match the pre-install read.

Final `lsof` inspection still found 15 processes holding the four retained
development stores. `lf monitor prune --dry-run --json` reported no registered
orphan providers; its dead receipt cleanup cannot retire these live stores.
No legacy process was signaled. The seven archived snapshots remain preserved
at the retirement path above. Installed routing acceptance is now demonstrated;
retirement of the four live stores remains the reason LOO-342 is open.

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

## Task purpose and completion (LOO-367, reconciled 2026-10-02)

Jack Heart accepted Task admission/completion independent of optional Flow and
delivery operations. On 2026-10-02 Jack confirmed that the working conversation
can complete its Task before its own Session or provider turn ends, then report
the result. Completion preserves that conversation and its history; cleanup
retains a checkout while it is in use. Other unfinished work, review boundaries
and unresolved delivery remain blockers. Membership alone grants no exemption,
Flow settlement or process control. This refines the LOO-358 completion rule below;
it does not weaken cleanup or abandonment protections.

Issue-specific confirmation replaces post-mutation whole-Wave refresh; filing
and marker recovery still require their current-Project routing. Failed confirmation
stays explicit, and registered completion retains pending writeback.

October 3 branch implementation admits Tasks with optional placement through public
Session binding and CLI execution; binding preview and generic readers stay
observational. A path/slug pair distinguishes absent delivery from lost placement.
Public bind → first checkout allocation preserves identity/history. After simulated
PR settlement, the requesting conversation completes its Task while retaining the
checkout; a later settled retry cleans it up. Unknown and live independent Execs
still block. Provider-turn and merge evidence are fixtures, not native acceptance.

Managed startup/replacement now avoid implicit allocation. Capture owns execution
cwd; the owning repository supplies managed planning. Continuation and replacement
retain that cwd, while checkpoint and PR recovery apply only to retained placement.
Attributed/taskless Flows share the driver and leave Tasks open. Desktop exposes
independent Flow progress/resume alongside the managed selection. The October 3
follow-up supersedes earlier feedback that these two cuts were unimplemented.
Failed-launch fixtures retain identity and both captures without delivery objects;
they do not prove successful worker launch or Desktop interaction. Those remain
with gate, along with CLI lifecycle and provider/review/retry acceptance.

Completion never ends a Flow or grants process control. The requesting allowance
matches current provider generation and exact driver parent, solely for completion;
cleanup, abandonment, pending reviews and stale callers keep their protections.
Migration freezes retained Flow paths without inventing unknown history. Upstream
#1415 recovery remains separate from completion's unknown-execution refusal.
PR observation locks before reading mutable PR state; absent placement needs no
PR lock. LOO-364 owns broader switching; LOO-366 owns Project availability/resets.

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

## Earlier synced planning and runtime selection (LOO-334, 2026-09-29)

Jack Heart approved one local planning interface, with repository-level Linear
authority when connected and private local plans otherwise. Git shares Wave
goals, memory, Flows, Skills and provider bindings; Linear adds shared planning.
Shared execution between participants belongs to the paid layer. Cross-store
discovery here serves one operator and grants no teammate execution authority.

Accepted contracts below exceed the implemented planning slice:

- Repository files establish Waves; never adopt every Linear Initiative during
  a read. Jack selected one Initiative per Wave/subwave, portable `A/B` names,
  and native parent links only when supported. Use the owning worktree's full
  definition set, including dirty additions/deletions; context-free reads use
  last-fetched configured remote main. Reads neither union views nor reset files.
  No-remote/main-checkout policy and outward definition sync remain unresolved.
- Separate presence, freshness and execution eligibility. Project-less issues
  exist; list omission and failed refresh cannot prove deletion. Acquisition
  timestamps are not provider revisions. Shared entities can be newer than a
  Wave's membership observation, so its sync timestamp cannot date every field.
- Invalid or mismatched planning must stop managed progression while ordinary
  worktree Flows remain available without settling that Task's invocation.
  Jack excluded automatic reconciliation. Cached-Task outage admission remains
  undecided; retaining cached inspection does not select an execution policy.
- Status observes PRs without completing Tasks. Chapter rollover must complete the
  predecessor after unfinished Task transfer and preserves archival/history;
  closure does not establish successful KRs or complete transferred Tasks.
- Jack selected the official runtime at each new worker boundary, independently
  of the execution store, with deliberate visible pins. This supersedes the
  coupled runtime/store target in the LOO-321 branch notes below, while retaining
  source isolation and verified historical pairs. It is not implemented yet.
  On 2026-09-30 Jack required explicit locks to propagate recursively through
  PATH into Flow children and Codex/Claude/OpenCode agent shells. Ordinary
  installation changes must preserve Task/review continuity automatically;
  user-supplied database paths or an owning-store choice do not satisfy acceptance.

The earlier branch's detailed findings and local/simulated proof receipts remain
at `32413d0d90e4eec1c48d7950963cf27837a0fab0:wave/infrastructure/MEMORY.md`.
The landed current-state cutover above supersedes its intermediate implementation.
Retain these constraints and unresolved proof boundaries:

- Partial webhooks invalidate rather than supply complete facts. Unknown revisions
  cannot order steering; an acquired null must not erase newer removal evidence.
- Project revisions do not order Initiative/Team relationships. Contradictory
  ownership remains unresolved outside rejected ingestion; replay is not repair.
  Ordered relationship acquisition and restoration remain unproven.
- Frozen chapter receipts retain predecessor facts. Confirm provider completion
  before archival, recover lost responses by reading provider state, and preserve
  Task/PR identity and KR/metric results. Local stateful proofs did not establish
  configured Linear completion or retroactively validate older receipts.
- Repository alias repair moves Waves and normalized planning atomically; conflicts
  must not merge observations. Populated explicit relocation and local-to-Linear
  connection migration remain unproven.
- Trace interest is process-global. The grouped OAuth regression required an
  isolated process/subscriber, not retries or production logging hooks.

Historical public CLI and Rust/Swift fixtures established planning-only inspection
under acquisition failure. They did not establish execution/action parity, managed
boundary enforcement, configured Desktop continuity or the full command story.
Jack Heart's September 30 bounded LOO-298 contribution confirmed the Task,
AgentSession, FlowSession and Exec ownership split; it was coordination evidence,
not completion or acceptance. Reuse WorkCatalog, TaskExecutionSnapshot, task_run
and the shared skill-command executor. Claim acquisition is not Started; driver
death is not provider death. Current migration policy above supersedes the earlier
intermediate-draft proof obligations.

## Session launch continuity (branch evidence, 2026-09-28)

Jack assigned `jack-heart/session-launch-hardening` after a review child rejected
`--tui` and a later installation selection made Tasks unreachable. No new Task
was filed; LOO-332, LOO-333 and LOO-298 retained separate scope. Full incident
analysis and the working design remain at
`a5d76c576609f6efc80a8ca2198480081066c63c:scratch/jack-heart/session-launch-hardening.md`;
dated memory details remain at
`32413d0d90e4eec1c48d7950963cf27837a0fab0:wave/infrastructure/MEMORY.md`.

Selected-store absence never proves deletion. Preserve Session identity and the
matching executable/data without merging divergent stores by timestamp. Saved
launch arguments establish preparation, not conversation access. The one-Home
cutover supersedes development-store routing, not the obligation to retain failed
launch provenance: attempted executable/digest, owning data, cwd and sanitized
outcome. The offending binary and switch initiator remained unknown; parser,
provider-lookup and store-absence failures had no proven single cause.

The recorded Session suite passed 6/7: successful retry lost failed-launch evidence
because `session open` bypassed the journal wrapper. The 23 installation tests and
Clippy/formatting passes did not resolve that failure. Prepare-under-A → select-B
→ open-same-review and real Ghostty resume remained unproven. Retry must preserve
failed evidence, stored history and pending review without a competing Task Flow.
No shipment, recovery or completion follows from those historical observations.

## Branch data and command ownership (LOO-321, branch evidence 2026-09-28)

The one-Home and LOO-298 decisions above supersede this branch's development-store
isolation contract. Full dated evidence survives at
`ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`.
Jack retained machine placement and remote Mac mini access; a Home remains an
execution destination. No copied snapshot grants authority or merges private writes.
Task continuation preserves captured definitions, failed feedback, checkout and
PR identity. Explicit restart replaces execution; checkout restores retained placement.
Dirty invoking checkouts remain valid; occupied paths and other owners stay protected.

Recorded disposable Linux proofs passed snapshot preservation, inactive-pair recovery,
readiness and fixture chat; they did not prove configured installed workers or Jack's
Desktop acceptance. Remote app chat, redirected-daemon children, local OS death,
seeded demo context and installed Session/new-draft Task writes remained unproven.
Installation recovery preserves verified historical executable/data pairs. Do not
restore removed Wave enablement controls or treat readiness as review acceptance.

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
receipt. Synthetic proofs establish
neither live OAuth nor installed acceptance. State remains Home-local; recorded
replay cannot recover another Home's custom database path. LOO-340 owns shared
authority; this curation authorizes no installed-store repair.

## Account auth consolidation (LOO-320, branch evidence, 2026-09-27)

Jack Heart approved demo scope and delivery through the saved Flow. Cross-account
continuation, native refresh coordination and headroom ranking remained outside
scope. Full evidence and approval context survive at
`ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`;
LOO-339 above refines identity and subscriptions docs own current mechanics.

Remember browser selection only after success; never infer it from the last
window. Native providers own OAuth completion. Cached status must not launch
providers, decrypt/import credentials, contact brokers or create directories.
Inherited leases without identity metadata remain uninspected. Keep dated usage
missingness; resets do not prove capacity. Reject replaced-credential results and
record actual account selection in Session history before native discovery.

Fixture passes did not prove configured login, managed connection, Linear targeting
or real usage. Claude returned invalid_grant. A copied Home retained credential
paths and was not isolation; native locks/Keychain differ from the install lock.
Jack prohibited branch promotion/installed writes. Cached/route disagreement and
copied-Home schema failure remained unresolved; original gate evidence predated
final edits, SSH required intervention, and native checks remained with CI.
No installed acceptance follows. Refresh compare/write races remain unproven.

## Task deletion and command ownership (LOO-305, branch evidence 2026-09-27)

Jack's final LOO-305 scope was provider/local deletion, removal of pm/work and
delivery through the saved Flow; execution settlement stayed deferred. Full branch
proof, configured deletion observations and the installed-database incident survive
at `ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`.
Current operations belong to Task/Wave/repo and doctor --planning; old spellings
are historical. This curation changes no delivery or Task disposition.

Observed issue identity survives refresh, but fresh ownership authorizes deletion;
acknowledgement or trash evidence confirms it. Missing membership never confirms
removal. Confirmation preserves Done outcomes, PRs, Git and terminal times. Planning
and completion have separate writers: provider terminal conflicts block new local
Done, narrow reconciliation preserves newer facts, and retry retains original outcome.
Planning-only creation needs neither agent nor checkout; execution requests preflight
their selected placement and account. Creation-marker recovery preserves identity.
Post-create failure requires recovery rather than compensating issue deletion.

Checkout identity comes from its branch, never upstream tracking. Removal is not
termination: a completed capture can leave live Execs, and native Session locking
cannot establish Task-wide settlement. The configured demo deleted LOO-299–302 and
verified refreshed absence, but also advanced installed Home drafts and broke its
older CLI. Jack forbade subsequent branch promotion/writes; LOO-321 owns recovery.
Linux simulated-provider proof did not prove process disappearance or installed
acceptance. Source tests use disposable Homes with inherited authority removed.

## Task convergence (LOO-319, branch evidence 2026-09-27)

Jack's initial implement/compress/review-slice loop was replaced by
implement → compress → refresh → loop-decide; refresh composes rebase → realign.
Jack accepted refresh's leased push. Captured invocations keep their definitions;
reconciliation does not imply convergence, and human review boundaries still hold.
Queue uses compress → refresh → gate; ship calls gate directly. Historical
update-wave/record-learnings directions are not current skill instructions.
Full approval, failures and proof scope survive at
`ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`.

Task agent precedence is persisted choice → captured skill → checkout configuration.
PM refresh cannot overwrite the dedicated choice. Failed decisions retain exact
Task/invocation/boundary identity and feedback; failed workers cannot regain authority.
Tests locate policy in the captured graph instead of hardcoding catalog indices.
Provider tools need the selected executable and Home; PATH alone failed the live
fixture. Real Codex reached a simulated policy boundary, but configured Claude,
current generic decision policy and Desktop acceptance remained unproven.

Stall observation never grants signal authority. Preserve first PID/start identity;
unknown samples stay unknown, active tools prevent stall, pending review takes
precedence. Free disk is the resource gate; oversized build directories alone are
measurements, and cleanup must not touch another active worktree. Temporary fixtures
did not prove a real five-minute stall. Jack authorized landing through the saved
Flow, withdrawing direct demo landing. No configured acceptance or Task completion
follows from the recorded local passes.

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
- **Bind** is write-once null to Task, allowed on done Tasks, and does not itself reserve work or set Started.
  Actual execution reservation owns Started. Jack selected prospective usage attribution for now on 2026-09-30;
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

**Historical gate and compression evidence, 2026-09-30.** Exact counts,
failures, repairs and production-line estimates remain at
`32413d0d90e4eec1c48d7950963cf27837a0fab0:wave/infrastructure/MEMORY.md`;
the integrated gate artifact is `5dee46ca8:scratch/integrated-gate.md`.
The full materialized Rust receipt remained failed despite focused repairs;
compression also repaired two failed Session cases without a full final-tree gate.
Python, website, formatting and final Clippy passed; Swift repaired an obsolete
fixture. None established configured continuity, installation or conversion.
Hosted CI owned the final candidate; dense cold/warm timing remained outstanding.

Durable findings: retain stable-ID ordering through Session projection; use the
stack transaction and published-parent evidence; resolve saved Skills from captures
before the mutable catalog. Automatic checkpointing must share execution's Work
binding reader (explicit selection, checkout, inherited context), or it can commit
another contribution. A faulty sync fixture launched a real conflict agent; it
reported no push, but native credential effects were not audited. Stub that path.
The pinned 0.12.23 classifier misread quoted scratch as a capability denial; source
repair alone did not update workers. Direct execution supplies retry/account
failover that the old worker path lacked. Claims must identify the worker process,
not its launcher. Owned-child SIGINT and Task cancellation require separate proofs.
Jack's cadence was focused checks plus hosted CI between items, then the full local
Rust matrix without fail-fast at final gate; skipped targets cannot count as passes.

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

## Continuation and recovery lessons (curated 2026-09-25)

Detailed observations and historical check receipts survive at
`ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md` and
`1a691ac6a222b95c46859c9c06d162d6442950a4:.lf/directions/task-continuation.md`.
LOO-295 owns continuation; LOO-296 owns broader restoration. Installed acceptance
and follow-up ownership were unresolved; this curation neither assigns nor closes them.

Capture alternatives and review policy before execution. Retry consumes original
successful evidence under its exact invocation and claim; replaced workers cannot
settle a reused cursor/version. Review feedback does not choose navigation. Preserve
published native identity and history across failed launch recovery; unknown execution
requires evidence before cancellation. A finished Flow never completes its Task.
Driver proofs need multiple turns, retained feedback and resume without replaying review.

The historical live demo required manual binding and native Home correction, so it
never proved automatic propagation. Preserve explicit cutover dispositions for custom,
queued, nested and uncaptured history; arbitrary queue transfer remains unproven.
The recorded broad suite passed separately from a failed default-gate receipt caused
by inherited Home authority. Neither establishes unattended configured continuity.
Fixture executables must be pinned and inherited execution authority cleared.

## Delivery and chapter implementation lessons (2026-09-25)

Curated from the retired `.lf/prs-and-tasks.md` and chapter direction note.
Current mechanics stay beside their code; these are constraints learned from
implementation, not another chapter plan.

- Preserve authored PR title and opening while adding one managed Task block.
  Keep title/body together through copy resolution. A same-head publication must
  display the persisted merge request, read under the mutation lock; publication
  alone does not request settlement. Landing reads copy before clearing scratch;
  publication consumes only `.pr-copy-ref`, `pr-title.txt`, and `pr-body.md` on
  its own path. Independent review evidence survives publication; gate no longer
  requires a duplicate review report. Release re-arming retains
  remote copy. Prove landing and release consumers when changing that shared path.
- Chapter rotation owns one deterministic boundary. Preserve started Task
  identity, expire untouched backlog and freeze predecessor metric targets,
  readings and evaluation time. Activated retries consume frozen evidence; a
  missing instrument must not prevent a later chapter dropping its target.
- Enumerate fresh provider membership before cutover and recover omitted current
  Projects/Tasks by stable identity. An unavailable read cannot be replaced by a
  cached plan. After activation, retries retain the frozen boundary; external
  reassignment remains unresolved, never authority to reclaim work. Settle a
  lost cancellation response against provider state, not portfolio absence.
- Task claims/retirement share SQLite authority; first execution survives Flow
  resets as Task history. Generic Run history is Home-local. Status and roadmap
  share the Task join, including stranded work; do not rebuild an operator tree
  or duplicate routing fields to flatten it again.
- Repo proof commands now live in TESTING.md instead of standalone local skills.
  Installed-updater compatibility lives in docs/lf.md. The env setup helper is
  `scripts/env-setup.sh`; `.lf/` holds executable configuration and chapter history,
  not a catch-all for maintainer notes. Historical chapter records are retained.

### Draft PR readiness (branch evidence, 2026-09-28)

Jack requested draft-by-default `lf pr open`, preserving already-ready PRs.
Publish/submit/arm/land promote only after GitHub succeeds. Browser opening has
no readiness effect. Current mechanics belong in CLI and delivery documentation.
The dated branch checks and unresolved installed acceptance remain at
`528625dc17ba4dd6440d7365c1a3073e1c71d205:wave/infrastructure/MEMORY.md`.

## PR landing recovery (branch evidence, 2026-09-25)

[PR #1287](https://github.com/loopflowstudio/loopflow/pull/1287) follows Jack's
direction: remove landing blockers rather than add receipt systems.

- **Repair needs ordinary delivery authority.** Etude #187's repair stopped at
  `lf rebase` with `Operation not permitted`: landing's Worktree-only override
  defeated its unattended launch request. More rebases cannot fix that boundary.
  The first denied path was not logged; shared Git metadata was the provider's
  explanation, not a measured filesystem failure.
- **Observe outcomes rather than require activity.** The same SHA can become
  pending, passing, or merged. Neither a new commit nor an unused incident claim
  proves progress. Incidents preserve history; one supervisor owns repair.
  Preserve authoritative merge observations instead of requiring another read.
- **Process success is not repair success.** Use the approved `published` or
  `blocked` result in the existing final answer. Surface the exact human action
  after reconciling GitHub; do not relaunch an impossible repair indefinitely.
- **Queue membership needs explicit cancellation.** GitHub CLI 2.101.0 returns
  success from `pr merge --disable-auto` for a queued PR without removing it.
  Distinguish pending auto-merge from queue membership and use
  `dequeuePullRequest` before publishing a replacement head. The regression's
  remote rejects pushes while queued; a mock command exit cannot prove removal.
- **Fence running effects, not only database writes.** Canceling an async waiter
  does not cancel its blocking repair. A replacement must wait for that operation
  to finish. Active joins must retain the checkout whose supervisor lock is held.
- **Local proof has a boundary.** Simulated provider/GitHub tests cover same-head
  recovery, explicit blockers, and canceled-watcher takeover. Live provider
  permissions, hosted recovery/interruption, and orphan-provider cleanup remain
  unproven. Green-but-unmergeable PRs and the original Etude `-c` attribution
  discrepancy remain unresolved; do not promote these tests into those claims.

Current mechanics belong in [delivery documentation](../../docs/architecture/delivery.md).

### Landing fixtures and deleted remote branches (2026-09-25)

- Fresh landing fixtures use `open_ephemeral_store` or
  `SqliteStore::open_ephemeral`. The constructor owns canonical migrations and
  the draft tail without ambient installation authority. Probing for a table
  and replaying its original migration misses later schema changes. Keep the
  forward migration and populated historical preservation tests for installed
  databases; see [TESTING.md](../../TESTING.md).
- Reproduce remote deletion directly in the bare remote so the checkout keeps
  its stale tracking ref. Deleting through the checkout's own push updates
  tracking and misses the failure. An explicit empty lease permits recreation
  while rejecting a branch created after the absence observation. Existing
  branches retain their tracking lease; fetching to bypass rejection could
  authorize overwriting unseen work.
- Fixture and rebase proofs passed: 17 landing unit tests, 22 rebase tests,
  11 CLI safety scenarios, and the authoritative-merge watcher test. Git
  transport uses local bare remotes; provider/GitHub responses are simulated.
  [PR #1289](https://github.com/loopflowstudio/loopflow/pull/1289) also recreated
  the deleted branch on GitHub using `scripts/dev-lf`; the installed 0.12.21
  command still reproduced the stale-lease failure. That publication proves
  branch recovery, not the other live recovery gaps recorded above.
- The installed `update-wave` skill explicitly sent unnamed-Wave learnings to
  `.lf/`, even after source guidance had changed. Resolve the owning Wave from
  repository objectives and refresh exported skills after changing their source.
  The retirement branch replaces that skill with realign, which curates existing
  identified Wave memory and creates no Wave for unbound work. Missing launch
  attribution is not a reason to create another memory location.

## Chapter boundaries and preservation (2026-09-23)

The [accepted chapter](../../.lf/chapters/20260923T000959Z-502f011b/start.md)
keeps Infrastructure responsible for execution mechanics, auth, placement,
recovery, releases, and repository-wide simplification. Product judges external
usefulness; Intelligence owns the non-authoritative evidence. A green scheduling
receipt does not prove publication, and a passing architecture inventory proves
only its declared coverage. Earlier release cadence and backlog notes below do
not override the accepted chapter or current Linear directives.

- **Separate authority from observation.** The summer moved from a remote
  daemon API to complete `lf` execution on each owning Home. Stable Work,
  provider-native continuity, direct child control, and current OS liveness have
  different owners. Run parentage explains causality; it cannot manufacture
  signal authority. The historical reconstruction is a synthesis, not proof of
  one simultaneous fleet deployment.
- **Reduce readers without losing evidence.** CLI and Wave status now share the
  same Work-filtered reader, including its tests. Recent history is bounded;
  exact child discovery is uncapped; activity includes Runs completing inside
  the window even when they started earlier. These answer different questions.
- **Historical archives are outside live architecture discovery.** The checker
  excludes `.lf/chapters/`, as it excludes generated website docs. Fourteen
  retired-term quotations initially failed the check. Preserve those exact
  observations rather than sanitizing history or adding word-specific exceptions.
  The regression still rejects retired vocabulary in active `.lf/skills/` files.
- **PM renames preserve identity but not every historical label.** Chapter
  application found new slugs failing in `lf project status` while stable UUIDs
  resolved the same Work and captured old plan. Use the archived
  `application/project-references.json` identities. Current PM and historical
  execution plans are different facts; never restart a controller or directly
  rewrite storage just to repaint history.
- **Read back shared PM writes from the provider.** During memory curation,
  concurrent branch updates replaced LOO-287's description after a successful
  write. Refresh, merge both evidence sections, and verify the resulting text;
  a successful command or stale local snapshot cannot prove the retained notes.
- **Consolidation does not rebind Task ownership.** LOO-278's files were integrated
  with `lf rebase --manual` into review-chapter, but its registered checkout and
  PR chain still name chapter-planning. Installed `lf task` has no supported
  reassignment; `prepare --name` rejects a different existing workspace and
  `lf work relocate` supports Waves only. Preserve the original tree and exact
  history; a supported adoption path must prove preservation before a new writer
  starts. This is an observed placement gap, not authority to edit the registry.
- **Current PM and historical Work counts differ.** Deduplicate by stable Work
  id before dispositions: the chapter observed 175 rows but 174 unique Works.
  Applied retirement preserved artifacts and history; a PM-complete retired
  issue does not establish shipment or a won KR. Replaying the archive verifier
  proves retained receipts, not fresh PM state or publication.

## Task execution authority (branch evidence, 2026-09-23)

The LOO-298 model above supersedes the Task-worker branch's Run/Playhead and
position-row implementation. Full dated history and proof limits survive at
`ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`.
LOO-286 retains its configured CLI/Home/app continuity proof; LOO-287 owns later
architecture reduction. Neither is completed by this curation.

Retain immutable captured navigation, exact invocation/version/generation fencing,
review evidence and atomic cursor settlement. Helpers, causal parentage and passive
inspection grant no Flow or signal authority. Process death needs exact evidence;
silence, age, tmux and provider history cannot establish it. Failed or interrupted
agent work cannot supply a successful decision. Recover saved boundaries after
catalog changes; ordinary errors release exact claims, unsafe state remains explicit.

Outstanding original proofs include concurrent starts with one worker, actual provider
death and same-cursor replacement, late-result rejection, helper refusal, Session
continuity across app restart, and configured runtime projection. Older migration
fixtures discarded controller-only progress; their green tests never proved lossless
cutover. LOO-298 later discarded historical import explicitly, without upgrading those
old results. The cause of the historical run_liveness removal remains unknown;
automatic idle triggering and governance follow-ups require reselection, not new
Tasks inferred from this archive. Keep the app a projection, never another scheduler.

## Observation must not manufacture idle work (2026-09-23)

The LOO-293 reader repair followed duplicate helper work and a supervisor that
mistook missing observations for absence. Exact observations and source/installed
counts survive at `ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`.
LOO-286 retains installed projection/cutover acceptance; neither Task is closed here.

One read-only WorkCatalog resolves retained stable identities and ancestry, without
requiring full current Project records or granting launch eligibility. Keep minimal
schema coverage and unambiguous selector resolution. TaskExecutionSnapshot shares
one sample of execution/lifecycle evidence across status, conditions and actions;
active work and unresolved review outrank next-launch configuration failure. Pinned
executable names remain legitimate: PID/start identity, not basename, governs receipt
matching. Reused PIDs remain rejected. Source dry-run prune retained live Execs, but
those reads do not prove installed Flow-schema acceptance. Isolate fixture authority
and executable selection; only owned fake provider processes may be cleaned up.

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

## Installation and command scope (branch evidence, 2026-09-24)

Full dated release-bootstrap evidence, failed attempts and command audit survive at
`ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`.
Current mechanics belong in docs/lf.md and TESTING.md. LOO-292 retains installation
and checkout-refresh acceptance; LOO-287 retains command-scope reduction.

Installation downloads verified release artifacts and uses the existing promotion
transaction; checkout integration is separate. First-install recovery cannot require
a prior selection. Matching version labels do not prove exact-store compatibility
or a complete macOS installation. Preserve prior binaries, data and switch receipts;
advancement finishing is distinct from the recorded phase. No redundant receipt
redesign was selected. Scheduled installation retains its launchd identity and
custom install directory, without source checkout/Python prerequisites.

Disposable Ubuntu candidate proofs covered fresh install, repair, failed activation,
retained fallback and authentic 0.12.18 transition. They did not establish published
acceptance or populated historical migration. The real 0.12.20 fresh install failed
promotion; real macOS install remained unproven. Machine installation resolves the
OS home, so HOME/LF_HOME cannot isolate promotion: use disposable accounts/containers.
Wait for packaging copies, verify archive digests, use a signed TLS server leaf,
and retain unrelated runtime tools such as ps when excluding Git from fixtures.

Ordinary folder execution must not synthesize a repository or swallow Git failure.
LOO-287 retains target-first scope resolution and generic execution through provider
completion without implicit Git checkpointing. Repository initialization policy and
populated relocation remain separate unresolved work. The retired Python updater
alias recursed through PATH; do not restore that compatibility shim.

## Shipped

Historical shipped feature descriptions and exact PR references survive at
`ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`.
Current CLI documentation supersedes old pm/work/rebase command spellings.

Retain these lessons: installation syncs exported skills after activation; checkout
placement and branch identity are separate; landing never renames a live checkout;
Linear native Issue/Project/Initiative ownership uses stable IDs. Refresh OAuth before
expiry without rejecting still-valid credentials after a transient refresh failure.
LOO-241 judges cron continuity by each job's latest due interval and exact scheduled
receipt; old raw gaps remain history rather than permanent failure. An operation
receipt is still distinct from successful product publication, as Release memory shows.

## Gotchas

- **`scripts/test.py --all` cannot green the Loopflow UI suite headlessly** (filed). `xcodebuild` runs 304 app/unit tests to a pass, then `LoopflowUITests-Runner` hangs before establishing its connection and Xcode exits 65. Reproduced with a fresh `derivedDataPath`, so it is not a stale-cache artifact. Treat a `--all` UI failure as unproven, not as a regression, until the runner hang is fixed.
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

## Model (design settled)

- Self-hosting is the default. The public repo carries containers, deploy scripts, service units, schedules, and docs; secrets live in Doppler or host-local env, never git.
- Nightly verifies release-grade artifacts with no publish or deploy side effects; weekly publishes only after equivalent verification passes in the same run.
- Loopflow carries the primitives; Cadenza mirrors the cadence and shape until a product-specific difference is deliberate and documented.
- Don't extract a generic multi-product deploy platform before a second or third real deployment proves the shape.
- Release owns the automation spine, not release-content substance: each product owns its own changelog and provider-specific agent credentials (beyond pass-through/secret wiring).
- **One writer per worktree is dispatch discipline, not a general lease**
  (decided 2026-07-10). Worktrees are cheap and placement already exists, so a
  second writer belongs in another worktree. The store contributes visibility;
  mutation-specific coordination may still use a narrow local lock, as exact-head
  PR finalization now does.
- **The database is durable control state, not a message bus.** Radio,
  `bus_messages`, `bus_cursors`, channel identity, bylines, and retention are
  deleted. Authored input is a durable Work Steer. Observation delivery must
  preserve input without requiring a resident Project. Automatic dispatch policy
  remains open; a nudge must not invent a second execution authority.
- **Wave chat connection retains one event-driven Home lifecycle** (reconciled
  2026-09-28). LOO-321 removes the explicit start/stop surface; opening chat
  connects through the local Home's `lf`/`lfd` pair without promoting or
  replacing binaries or moving remote placement. Daemon boot
  publishes one attempt-scoped durable `live | failed` receipt and uses a
  private socket only as the wake edge; `lfd` owns listeners and shares each
  listener's `starting | live | failed` transition with concurrent callers.
  Success drains the durable observation outbox before returning. Failure
  compensates only registry state introduced by that attempt, and one failed
  Wave never terminates successful siblings. The Mac app uses the shared
  connection path. Reconciliation polling remains recovery,
  never startup acknowledgement.
- **Controller evidence is not an agent Run** (learned 2026-07-20; clarified
  by LOO-334). An authorized completion operation persists the Task domain
  transition and completion event in one transaction. A status read observing
  a merged PR must not perform that transition. Never mint a
  synthetic Run to reuse a Run-owned terminal transition. Prove this boundary
  with a zero-agent-boundary fixture and repeated reads that count Runs and
  completion events.
- **Execution authority changed on the Task-worker branch.** The former
  resident session, lifecycle/gate epochs, mutable Flow pins, and parent Turn
  Basis are historical models. Use the Task execution section above for current
  ownership, recorded decisions, failure evidence, and migration constraints.
- **Performance evidence preserves missingness at every boundary** (learned
  2026-07-21). A provider receipt absent, one missing field, and a reported
  zero are distinct facts; the first accepted per-Turn receipt wins and a
  conflicting repeat makes capture partial without rewriting spend. Window a
  scorecard by the owning fact's terminal time, never its parent's start time,
  and publish eligible/measured coverage beside every percentile. An absent
  authority is a named `UNKNOWN`, not permission to infer from observer
  timestamps, trace text, or zero. Budgets judge evidence; they do not change a
  correctness result.

## Planning model (settled, PR #852)

The current Wave/Project/Task contract at the top of this memory supersedes the old
label-based PM representation. Full migration observations remain at
`ec7ce16f9be5347b53a5881e4f18ba458a05a01e:wave/infrastructure/MEMORY.md`.
Linear Project content owns definitions and KRs; local snapshots are read models.
Do not recreate local Project files, a Chapter table or a fourth planning noun.
Stable IDs survive rename; ambiguous ownership stays unresolved. Planning diagnostics
never apply guessed moves. Frontier Projects can remain undated. The original native
hierarchy bootstrap preserved its seed marker and refused ambiguous issue labels;
those observations do not authorize reviving the superseded label model.

## Earlier follow-ups (reselect through the accepted chapter)

- **Reduction leftovers from the `minds` review** (triaged; the `TurnFinished`+`BodyFinished` collapse, the `LoopRun` reuse in `bin/lf.rs`, and the stale `playhead.rs` error hint are applied): factor the shared inbox-interrupt arms and lift the lease-renewal block; merge `interrupt_child`/`interrupt_harness` behind one `begin_interrupt`; finish the endpoint-resolver consolidation; inline `require_loop_flow`. `heartbeat_idle` stays — a real scheduler input, and deleting it to satisfy a lint instinct is reshaping production code around tests in reverse.
- **Live Work/Launches per worktree in `lf status`** — the store already holds
  their cwd. Visibility, not a general lease (see the one-writer rule above):
  typing into an occupied tree should be a choice made with open eyes, not a
  discovery made in history.
- **Concurrent PM reads on status/sync** — `lf pm show` fetches per-project issue lists concurrently, but `pm status` and `pm sync` still read them sequentially. File if sequential reads become a measured bottleneck.
- **Drain current buffer** — keep local `lf`, release scripts, and CI aligned with the latest merged release-infra work.
- **Cadenza release parity** — same nightly/weekly cadence, one-command updater, tests, self-hosted assumptions; document any deliberate divergence.
- **Cron host bootstrap** — bring up the first maintained `lf cron` host (Mac mini default), Doppler configured, with scheduled checks.
- **Release feedback loop** — failed nightly/weekly runs surface as attention items or focused fix PRs, distinguishing verification vs publish vs host vs stale-local drift.
- **Installed-upgrade semantic gate** — preserve saved Task invocations and
  historical stops through migration; validate new selections against the
  candidate catalog without re-resolving active definitions.
- **Project terminal-receipt parity** — make Project failure events and
  Run/Invocation settlement share the atomic receipt boundary now used by
  Tasks, with a fault-injection proof.
- **Replicate intentionally** — apply the skeleton to Manabot/Hootro only when they need it.

- **Deferred: "up/down 5ths"** (Jack, 2026-07-06) — referent unresolved. `lf wt` shipped up/down stack navigation this branch; candidates for the phrase are stack level-jumps ("fifth" = a level), circle-of-fifths name generation instead of random word pairs, or a chord-model transpose. Jack said "keep going" — deferred, not dropped.

The rebase-efficiency follow-ups are resolved by PR #818: config/naming-schema redesign shipped as `WaveId`; `lf wt create` is sibling-only; Task and Project Work own higher-level worktree placement; land rotation and `next`/`advance` are removed.

### How to judge rebase efficiency (dogfood metrics from `.lf/tmp/metrics/ops.jsonl`)

Local-only JSONL, reviewed weekly. Key product metrics: **agent-rebase rate** (% of rebases launching an agent), **avoidable rebase-agent rate** (stale/empty/generated-only branches that still launched one — target 0), median `land`→queued/merged time, post-land repair rate, and command-drift rate (prompt-recommended commands the installed `lf` can't parse). Then flip one default at a time: stack-by-default `wt create`, stale-empty reset before rebase, land/advance split, generated-only reset policy. Synthetic-workload replay harness (50–100 disposable histories, current vs classifier in trace mode) is unbuilt — file if tuning thresholds needs it.

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
