# infrastructure wave memory

Renamed from `systems` in the 2026-07-08 wave/project/task restructure. Owns dependable self-hosting, verified releases, and architecture minimalism. The configured release schedule and accepted proof obligations govern current work; older nightly/weekly notes below are historical.

Release-specific findings live in [release memory](release/MEMORY.md).

Historical sections condensed October 3 retain their complete text at
`12016c6d6dd34a4c1a553c25b5cf2529ee93615b:wave/infrastructure/MEMORY.md`.
This is local Git evidence; no remote availability or new acceptance is implied.

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
October 2 allocation observation is historical. LOO-366 now has
its assigned checkout and Task (`task_7c24c806bfaa464a877352b568384fff`) in the
October 3 kickoff seed; this does not establish LOO-367's current delivery state.
Neither unchecked current Project KR has outcome proof;
the metric portfolio and Project metric targets were empty in this observation.

October 3 source inspection for LOO-366 confirms three independent constraints:
nonempty Flow checks in create/adopt/update/reset, shared predecessor-name
inference in rotation, and rejection of unrecognized em-dash title prefixes.
The kickoff plan proposes explicit Wave Project ensure with shared CLI/Desktop
operations and durable Project-transition evidence. It is a draft mechanism,
not Jack's approval or configured acceptance. Project ensure must remain
independent of primary Session launch and unrelated Waves. Release's child
memory reinforces the same recovery lesson: successful entry-point return is
not proof of the provider outcome; retry through the operation's existing owner.

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

October 2 readback from `lf task status LOO-342 --json` records
[PR #1381](https://github.com/loopflowstudio/loopflow/pull/1381) merged at
`6c73356074c47a90491c1ed4de8e063d85eb3a30`; its merge observation was checked at
18:14 UTC. The Task remains ready with an idle saved `ship` Flow at `pr land -c`.
This establishes landing, superseding the branch-only delivery evidence above,
but not installed acceptance or retirement of the retained stores. Resuming that
completion step was deferred because those obligations remain unproven. Resolve
their outcome or explicit remaining disposition before treating the Task as done;
do not recreate a PR merely because the generic action suggests a next PR.

Configured acceptance attempted October 2 at 19:12 UTC after Jack Heart requested
the checks and retirement. The machine install receipt selects published v0.12.29
(`61d21f88564783a5fc8f63e385ec035f096fc069`) and `~/.lf/loopflow.db`.
`gh release view` confirmed [v0.12.29](https://github.com/loopflowstudio/loopflow/releases/tag/v0.12.29)
was still the latest published release (September 30, 23:45 UTC). Git ancestry
confirms it excludes the October 1 one-Home merge. Installed `lf home id` returned
the main Home, but an explicit fresh `LF_HOME` through both the public entry gate
and selected artifact still returned main's identity and Waves without creating
the experimental database. The available source CLI was v0.12.27. These are
pre-cutover observations, not failures of the merged implementation. Installed
acceptance requires a published release containing #1381; do not repeat the same
checks against v0.12.29 or treat a source-only proof as installed acceptance.

Fresh `lsof +D` inspection found live database handles in all four retained
`installed/local-*` stores above (15 distinct processes in the initial read).
No process was stopped and those stores remain untouched. Seven inactive
worktree snapshots, including `loopflow-growth-thoughts-1c80b40d4504`, were checked
again individually and moved intact to
`~/.lf-retired/20261002T191224Z/worktrees/`; `retirement.json` there records exact
source and destination paths. This retires their old routing locations while
preserving history; it does not reclaim their disk space. The four live stores
remain pending until their owners settle. After release, repeat default/source,
nested Flow/agent and disposable-Home acceptance, then recheck live ownership
before retiring the remaining stores. LOO-342 remains open.

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

Jack Heart approved repository-level Linear authority when connected, private
local planning otherwise. Files establish Waves; never adopt every Initiative
on a read. The owning checkout's complete definition set, including dirty edits,
is authoritative; context-free reads use last-fetched configured remote main.
No-remote/main-checkout policy, outward definition sync, cached-Task outage
admission and populated repository relocation remain unresolved.

Separate presence, freshness and eligibility. Missing/failed membership is not
deletion. Acquisition time is not provider revision. Project revisions cannot
order Initiative/Team relationships. Unknown-revision webhooks invalidate; a
late null fetch must not erase positive removal evidence. Contradictions remain
unresolved outside rejected ingestion, and replay cannot repair ownership.
Repository alias repair moves planning and Waves atomically without merging
conflicting observations. Keep historical Projects/Tasks visible independently
of current membership. Invalid planning blocks managed execution, not ordinary
worktree Flows or inspection; Jack excluded automatic reconciliation.

LOO-334's September 30 completion-before-archive proof preserves Task/PR identity
and frozen metrics. Retry reads provider facts after a lost response. External
archive/restoration and configured completion remained unproven. Older archive
receipts do not retroactively prove completion. One local reader supplies CLI
and Swift planning; fixture/installation checks were simulated, not live Linear.
Tracing callsite interest is process-global: isolate subscriber tests in their
own process rather than adding retry/hooks.

Jack selected the official runtime at each new worker boundary with explicit
pins propagating recursively into Flow children and agent shells. These runtime
selection intentions were not implemented in that slice; installation changes
must preserve review/Task continuity. LOO-298 coordination at `4f4edff9b` through
`d61295196` supplied the three-owner contract, not completed acceptance. The
later synced-planning integration and one-Home entries above govern current
implementation. Exact earlier proofs remain in the archived memory.

## Session launch continuity (historical branch evidence, 2026-09-28)

Jack assigned `jack-heart/session-launch-hardening` an independent checkout after
a review child rejected `--tui` and installation selection made Tasks unreachable.
No new Task was filed. Evidence: `a5d76c576609f6efc80a8ca2198480081066c63c:scratch/`.
Selected-store absence is not deletion. Saved handoff argv/artifact verification
can recover a launch but do not prove discovery across stores. One-Home decisions
supersede the old pair-selection mechanism, while failure provenance remains:
retain attempted executable/digest, cwd and sanitized outcome before provider
startup and across retry. Original offending binary and switch initiator remained
unknown; parser rejection, native lookup failure and missing-store evidence are
distinct causes.

At `a5d76c576`, Session checks passed 6/7; successful retry still lost failed-launch
evidence because `session open` bypassed journal recording. Installation tests
passed 23 and static checks passed, not recovery. Disposable prepare-under-A /
select-B / open-same-review and real Ghostty continuation remained unproven.
Do not treat a window/native ID as preserved pending review or start a competing
Flow. LOO-332/333/298 retain their separate scope.

## Branch data and command ownership (LOO-321, historical)

Jack Heart's one-Home decision above supersedes branch-store selection, seeding,
and promotion restrictions from LOO-321 / PR #1308. Detailed incident evidence
and synthetic proofs remain in `5f10576bd8e060e20c6aeb9addca8a462bc838b6:scratch/`.
Retain the general boundaries: recover Task checkout/branch/PR identity without
restarting captured work; membership grants no execution authority; failed
migration preserves database and matching executable. Missing recovery evidence
stays unknown. Removed Wave enablement/start/stop controls must not return.
The historical 20-caller chat deadline, remote transport, local OS death evidence,
redirected daemon children and Linux oversized prompts were not proven by the
four successful Linux installation fixtures. Those obligations are historical
unresolved evidence, not new Tasks or a mandate to restore private stores.

## Managed account identity (LOO-339, branch evidence, 2026-09-30)

Jack Heart selected identity core and authorized landing/patch release without
review. LOO-340 owns shared state across Homes, current status, browser suppression,
Claude routing, Flow bundles and reset credits; LOO-338 owns rename. Evidence:
`8973f689a9e11a83b2dfa467fecddd095940d435:scratch/`; authorization is not shipment.

Identity compares expected email and per-user subject, never shared workspace.
Codex verifies `account/read` plus credential file identity; Claude uses profile
email/UUID rather than stale `.claude.json`. Stage reconnect before installation;
failed identity/duplicates preserve live credentials. Loopflow install locks do
not coordinate native refresh; Codex can still open an extra browser tab.
Service 500 preserves credentials, 401 marks missing, and narrow writes preserve
other facts. Healthy automatic choices prefer observed Pro before Plus, while
explicit selection and Session affinity win. Expired windows are unknown.

Gate accidentally launched real Claude because inherited `LF_BIN` outranked
stubs. Clear ambient execution authority and pin the fixture CLI; a disposable
Home alone is insufficient. Native credential effects were not audited.
Recorded isolated checks passed 2,192 Rust (13 skipped), 78 website (3 skipped)
and static checks; the original screenshot-timeout gate receipt remains failed
and is not reusable. Synthetic checks establish neither live OAuth nor installed
acceptance. State remained Home-local; LOO-340 owns shared authority.

## Account auth consolidation (LOO-320, historical branch evidence)

Jack Heart approved PR #1307's scope and delivery through the saved Flow.
Details and contrary proofs remain in
`1891b5ea649c3c21780c264fa83ba5b00452045c:scratch/`. Cross-account continuation,
native credential refresh coordination and headroom ranking were excluded;
follow-up identities were not established. LOO-339 above supersedes identity
validation assumptions, not the unresolved native refresh races.

Cached status must remain read-only through its dependencies. Browser callback
and printed/manual URLs are different evidence; native providers own OAuth.
Retain each usage window's source and age, preserve omitted windows, and never
infer new capacity from reset success. Actual selected account precedes native
Session discovery; retries preserve earlier attribution. Loopflow's flock does
not establish interoperability with native Keychain/refresh locks.

Four live proofs remained unproven: no-paste browser login, first managed
connection, remembered Linear browser targeting, and real Claude/Codex usage
windows. Claude returned `invalid_grant`; decoder fixtures were synthetic.
Copied databases still reference original credential directories. The copied
Home lacked `auth_browser_bindings`; cached status and route inspection disagreed
on missing credentials. Neither limitation was added to approved scope.
Recorded affected gate passed 2,127 materialized Rust, 250 Python and 78 website
tests, plus static checks; its initial fingerprint predates final edits and is
not reusable. One SSH-fetch fixture required intervention. Swift/app, PTY
secrecy, manual fallback and installed acceptance were not demonstrated.

## Task deletion and command ownership (LOO-305, historical branch evidence)

Jack's final PR #1302 scope covered provider/local deletion and removal of
`pm`/`work`; execution settlement remained deferred. Exact design and proofs:
`4a14c0a47dc6e04be9668fb72b737828565a931d:scratch/`.
Deletion requires fresh ownership and positive provider acknowledgement; list
omission never proves deletion. Confirmed deletion preserves terminal outcomes,
PRs, checkout and history. Completion has a separate writer and preserves its
original event/time on retries. Planning-only creation allocates no checkout,
Task execution or agent account. Retry markers survive edits and reuse identity.
Upstream tracking is not checkout identity; publication establishes tracking.

The live deletion demo removed LOO-299–302 but also advanced installed drafts
and broke the older CLI. LOO-321 owns that incident; one-Home decisions above
supersede its recovery routing. Removal did not prove provider/process death or
complete Session/activity disappearance. Synthetic deletion and focused checks
were not full gate or installed acceptance. Blank participant overrides use
configured/Git names; steers preserve known provider authorship.

## Task convergence (LOO-319, historical branch evidence)

Jack accepted convergence before publication/demo and delivery through the Task
Flow (PR #1301). Evidence: `3abb64bf1a25d4109a8353e22f4a55f664a6f4de:scratch/`.
September 28 realign superseded review-slice with implement → compress → refresh
→ loop-decide; refresh owns sync/reconciliation and memory. Captured definitions
survive catalog changes; fixtures must locate captured policy rather than copy
step indexes. LOO-298 supersedes the older Run/Ask ownership mechanism.

Persisted Task agent choice precedes captured skill and checkout configuration;
refresh must not overwrite it. Failure feedback alone cannot supply a verdict.
The real Codex fixture showed policy reassessment after synthetic feedback but
predated generic loop-decide; Claude launch/resume, five-minute stall and
rendered Desktop agreement remained unverified. Direct landing was withdrawn
in favor of the saved Flow. Native credential repair was outside scope.

First process PID/start identity survives missing samples; a replacement sample
cannot authorize signaling. Unknown remains unknown; active tool CPU prevents
stall classification. Free disk is the resource gate, aggregate build bytes only
an observation; 24 GiB triggered local cleanup. Temporary sibling/low-disk
fixtures did not prove configured operation. Historical executable/Home routing
is superseded by the one-Home acceptance above.

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

## Continuation and recovery lessons (curated 2026-09-25)

LOO-295 owns continuation; LOO-296 broader restoration. The retired source remains
at `1a691ac6a222b95c46859c9c06d162d6442950a4:.lf/directions/task-continuation.md`.
LOO-298 supersedes its Run/file-position owners. Retain these obligations:
compile every alternative before execution; recover saved reviews without the
catalog; failed completions lose decision authority; invocation identity fences
late workers even when generations repeat. Feedback does not navigate a Flow.
Persist completion before teardown and serialize all Session input writers.
Silence or an absent listener cannot prove provider death.

The configured demo needed manual bindings and provider-Home correction, so
it did not prove automatic propagation. Forwarded credentials, arbitrary queue
transfer and configured operation/chat/interruption recovery remained unproven.
Recorded broad fixture passes followed a failed default-gate environment pin;
they do not rewrite that receipt. Cutover disposition must retain original IDs,
unknown work and readable history. Pin fixture executables while retaining
system tools. Follow-up ownership and installed acceptance remain unresolved;
do not recreate already-implemented historical Wave deletion work.

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

Jack requested draft-by-default `lf pr open`, preserving an already-ready PR.
Publish/submit/arm/land promote drafts; opening the browser has no readiness
effect. CLI and headless Flow operations share the existing create/update path.
Promotion owns its local readiness update after GitHub succeeds, so a failed
promotion cannot advance Task state or publish ready copy over a draft.

The [preserved realign design](https://github.com/loopflowstudio/loopflow/blob/60ee8daff36b2c97ad5a6f30e1f5daa98a40bca4/scratch/realign.md)
records stateful local GitHub/browser proofs for new drafts, repeated opens,
ready-PR opens, promotion failure and retry, plus focused Task-copy and delivery
checks. The final 27 selected builtin/export/draft tests and all-target Clippy
passed. These are recorded branch checks, not fresh checks from this curation,
real PR mutations, installed acceptance, or proof of Task completion. Historical
links identify local commits; remote availability was not checked.

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

## Task execution authority (historical LOO-286 / LOO-287)

LOO-286 retained `jack-heart/wave-agents`; LOO-287 owns later reduction. Do not
create a competing checkout from a tracking issue. LOO-298 above supersedes the
old `task_flow_positions`, Run and Wave Playhead architecture and historical
migration requirements. Earlier counterexamples and checkpoints remain in the
archived memory identified below; neither Task was completed by curation.

Keep execution independent of optional Wave chat. Captured definitions survive
catalog deletion. Invocation identity, driver generation and provider generation
fence distinct races. Task membership cannot grant cursor or signal authority;
exact native process evidence owns signals. UI is projection and trigger, never
another scheduler. Deleted Session/body leases (`a7044e2b5`) and the Invocation/
Epoch/Basis/liveness stack (`5f7f66833`) must not return. The operational cause
of the old liveness deletion remains unknown.

Historical LOO-286 proof gaps: real finite Project operation; concurrent Task
starts converging; actual provider death and same-cursor recovery; late-result
rejection; helper isolation; exact conversation continuity after app reopening
with optional services stopped. Seeded stores and killed sleep processes prove
only their boundaries. Earlier migration tests discarded controller-only and
Project progress; green tests did not prove lossless cutover. Jack later dropped
historical import, but current Work/conversation retention remains binding under
LOO-298. Worker lifetime, automatic idle triggers, remaining governance and
installed projection consistency require reconciliation with their owners,
not a new Task for every historical remainder.

## Observation must not manufacture idle work (2026-09-23)

The `task-viewer` repair (`7a41fc0b2`, `d7e221fda`, `aabc03966`) followed two
duplicate helper implementations of LOO-293. Separate readers had reconstructed
identity and liveness from display conventions, and a supervisor treated missing
observations as absence. This branch repairs those readers; it neither adds a
launch lock nor establishes a deployed execution cutover.

- **One catalog resolves Work for Runs, Usage, and Activity.** Read stable
  identity/ancestry columns through a read-only query, not full Project records:
  the latter failed on the installed store's missing `iteration` column. Keep
  the minimal-schema regression. Public issue identifiers, internal Work IDs,
  and external IDs select the same Work. Resolve an unambiguous most-specific
  Work before checking historical ancestor labels; use ancestry to disambiguate
  shared names. Current user filters still apply to today's hierarchy. Preserve
  original manifests and record full ancestry on future bound Runs.
  The catalog is an ephemeral index, not a second durable owner; its selector
  aliases preserve historical Run discovery and are not obsolete compatibility.
- **Receipt ownership survives executable renaming.** Pinned `lf-<hash>` workers
  are legitimate. Visibility and prune share PID/start-identity matching, without
  a basename precondition. Retain reused-PID rejection and keep that fixture's
  PID distinct from the absent OpenCode owner. A completed native launcher says
  nothing definitive about the remaining client or unresolved Session.
- **Sample execution and lifecycle once per Task detail.** TaskExecutionSnapshot
  projects the existing FlowPosition/claim/process evidence for status,
  conditions, actions, and roadmap. Current non-idle evidence wins over dirty
  progress and next-launch configuration failure. An unresolved review boundary
  still yields a waiting condition on terminal Work. Keep lifecycle, worker
  evidence, and UI condition distinct; remove parallel derived Session booleans.
  Run attribution does not acquire advancement or process-control authority.
- **Source proof and promotion are separate.** The incident read found all
  three Runs via public/internal selectors and Usage; source `ps` saw 20 live
  nodes versus installed 0.12.19's two, and dry-run prune excluded all ten live
  Exec PIDs. Nothing was reaped or migrated. Current-schema status has isolated
  test evidence only. Installed projection/cutover proof remains in LOO-286's
  existing obligations; this does not settle it or LOO-293.
  The 2026-09-24 read repeated the three-Run result and excluded all six live
  Execs from dry-run prune (12 live nodes at that sample). Source builds default
  to a development Home: an empty result there says nothing about installed
  history. Use the owning executable/database pair for installed incident reads;
  a branch executable may redirect an explicit installed directory to private data
  under LOO-321's isolation contract above. For isolated tests, clear
  `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`, `LF_HOME`, and `LF_DB_PATH`.
  Neither sample proves the older installed Flow schema works.
- **Isolate launch tests from ambient authority.** A research fixture inherited
  the live control database, and a Session fixture failed to publish its fake
  client during the initial broad suite. Isolated reruns passed after clearing
  execution authority. Fixture cleanup may stop
  only its identified fake provider child. A passing DTO round-trip alone cannot
  prove execution precedence.

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

## Installation and command scope (historical 2026-09-24)

LOO-292 retains installation/integration acceptance and LOO-287 command reduction.
Evidence: `1f2d2c051:scratch/`; one-Home installed proof above supersedes the older
unproven routing acceptance. Machine installation selects published artifacts
and uses the promotion transaction, without Git/source upkeep. Checkout sync
owns integration. First install has no prior fallback; candidate handoff must
remain recoverable without requiring a prior selection. Exact-store preflight,
complete macOS artifacts and bounded subprocess inspection matter more than
matching version strings. Do not restore Python refresh recursion.

The disposable Ubuntu candidate passed fresh install without Git, repair,
dirty/ref/migration preservation and authentic 0.12.18 transition; that fixture
had no populated historical Home. Public 0.12.20 fresh-install failure remains
contrary historical evidence. Populated historical migration and login/wake
schedule behavior were not proved there. Debian bookworm's GLIBC incompatibility,
archive copy race/segfault and incorrect TLS leaf were fixture boundaries.

Installation resolves the OS account home. Environment overrides cannot isolate
promotion: use disposable OS accounts/containers without real installation.
Preserve system tools when excluding Git/lf. Default schedule was login plus
Monday 09:00, with positional weekly/daily/hourly/5min alternatives; launchctl
fixtures did not prove wake behavior. Promotion phase alone cannot replace
artifact/store/fallback evidence.

LOO-287 retains target-first scope resolution and ordinary-folder execution
through completion without implicit Git checkpoints. Do not swallow genuine Git
errors or add fake repositories/capability registries. New-repository setup,
receipt redesign and remote discovery policy were unselected; no generic
platform is authorized by these historical notes.

## Earlier shipped history

Detailed release notes for skill sync, worktree placement, Wave identity, Linear
hierarchy/OAuth, deterministic rebase and cron continuity remain in the archived
memory below. Current command names and mechanics belong in docs, not historical
examples. Retain the unresolved efficiency experiment: no synthetic 50–100-history
rebase comparison harness exists. Any classifier tuning needs real evidence.

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

## Planning model (historical PR #852)

Wave → Linear Initiative, Project → Linear Project, Task → Issue superseded
label-based planning. Stable UUIDs own identity across renames. Seeding was
restart-safe and only retired old bindings after all legacy issues were assigned;
ambiguous issues remained explicit. Current synced-planning and optional-chapter
decisions above supersede the earlier Linear-only and required-default assumptions.
Projects need no target date. `doctor --planning` diagnoses without applying
moves. Native subwave links and local planning follow the newer ownership
contract. Detailed rollout history remains in the archived memory.

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
