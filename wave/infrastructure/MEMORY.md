# infrastructure wave memory

Renamed from `systems` in the 2026-07-08 wave/project/task restructure. Owns dependable self-hosting, verified releases, and architecture minimalism. The configured release schedule and accepted proof obligations govern current work; older nightly/weekly notes below are historical.

Historical detail retired during the LOO-326 curation remains in `b95976881da61e0fa63fc06b5058d9e4c7dc22f4:wave/infrastructure/MEMORY.md`.
This curation preserves unresolved obligations without restoring superseded owners.

Release-specific findings live in [release memory](release/MEMORY.md).

The 2026-09-30 [LOO-298 decisions](#data-model-and-performance-decisions-reconciled-2026-09-30)
supersede older Run-owner, historical-import, pinned-development-Home and
demo-before-landing directions for this cutover. Earlier incident observations
remain evidence of their own versions, not instructions to restore those owners.

## Transient recovery (LOO-326, branch evidence 2026-10-04)

Jack Heart approved existing Tasks using valid cached planning regardless of age;
known invalidation/removal/terminal state/ownership mismatch still blocks. Apply
that policy at restart, continuation and each managed worker boundary. New advice
still requires successful Linear publication before worker replacement. Preserve
observation age; never rewrite a stale response as freshly acquired evidence.

Read retries belong only around the failing provider read. Artifact attempts use
separate temporary directories and keep candidate identity fixed. Bound pipe
collection as well as process exit: a descendant can retain stdout after its
parent exits. Never retry publisher writes or turn missing checks into success.
The release child's Swift cleanup finding remains owned by release memory; Jack's
supplied steers establish its v0.12.24 shipment. The other changes here are branch
evidence until delivery, with no live release or installation replacement.

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

Branch `01f7814ef` implements normalized connected planning, revision-ordered
issue and Project facts, webhook receipts and planning-only status. Inspection
retains invalid/removed facts and their age even when hard-stale or forced
acquisition fails. Public CLI proof now covers unavailable planning without
execution; Rust/Swift fixtures cover the planning envelope, all with null
execution. Managed readers still exclude invalid/removed records. Full
execution/action parity, local lifecycle, contextual definitions and runtime
selection remain work; inspection does not prove managed boundary enforcement.

Durable lessons from this slice:

- Partial webhook payloads invalidate; they never supply complete entity facts.
  Keep revision/removal receipts even before caching or execution exists. Unknown
  revisions cannot authorize ordered steering. Realign reproduced a missing
  `updatedAt` content edit failing before invalidation, then routed it through
  the existing unknown-revision invalidation path.
  After acquisition, preserve newly arrived removal evidence: a null response
  must not downgrade a confirmed removal to absence. A barrier-controlled lookup
  reproduced that race; inspection now retains removal and dated facts while
  managed reads refuse.
- Project facts have their own revisions, independent of the enclosing issue.
  Project `updatedAt` cannot order separate Initiative/Team relationships.
  Contradictions persist as unresolved ownership outside the rejected ingestion
  transaction; replay cannot repair them. Ordered relationship acquisition and
  repair remain necessary. List omission alone cannot retire a Project.
- Current normalized membership cannot reconstruct a predecessor chapter. The
  chapter receipt owns frozen history. Failed assertions expecting stale snapshot
  resurrection or never-acquired entities did not establish lost history.
  Confirmed chapter archive acknowledgements exclude predecessors from current
  reads, including after migration or delayed lists. The September 30 slice now
  completes predecessors before archival, using the current status's team or
  workspace scope. Refused or unconfirmed completion cannot archive; retry reads
  provider state after lost responses. Its stateful local proof preserves Task/PR
  identity and frozen KR/metric results. Existing historical receipts are not
  retroactive proof of provider completion. External archive acquisition,
  restoration and configured Linear completion remain unproven.
- Repository alias repair must move Waves and normalized planning atomically.
  Moving only the Wave stranded its planning in the old scope. Conflicts fail
  without merging observations; this canonicalization is not cross-repository
  relocation or local-to-Linear connection migration. Populated planning across
  explicit repository relocation remains unproven.
- Tracing callsite interest is process-global. The grouped OAuth log-capture
  failure reproduced and was resolved with an isolated process/subscriber;
  its recorded grouped run passed 15 active tests. Do not add retries or production
  tracing hooks to satisfy a concurrent test's subscriber.

Recorded lookup, mutation, receiver, migration, Rust/Swift planning and five
installation-container proofs are local/simulated evidence, not live Linear or
the full command story. The working design retains exact receipts and limits.
Jack supplied successful LOO-298 coordination on 2026-09-30 from a bounded read-only
contribution at `4f4edff9b` through `d61295196` (documentation-only difference).
Task retains identity/Project/Wave/checkout/PR evidence; AgentSession/session_events
owns conversation/native execution/usage; FlowSession/flow_events owns graph,
cursor, claims and review; Exec owns process evidence. Runtime Run storage is
removed there. Runtime child FlowSession removal and preserving migration remain
outstanding: discovery and locks must not depend on those children. Reuse the
narrow WorkCatalog reader, TaskExecutionSnapshot, task_run and shared skill-command
executor. Claim acquisition is not Started; driver death is not provider death.
Prove released and draft-Session migration frontiers separately without rewriting
applied checksums. This contract supersedes the unregistered coordination blocker,
but LOO-298 remains unfinished and unaccepted; its execution model is not integrated
into this branch. Neither coordination nor the chapter proof establishes the full
continuity acceptance, Task completion or Flow navigation.

## Session launch continuity (branch evidence, 2026-09-28)

Jack assigned `jack-heart/session-launch-hardening` an independent main-based
checkout after a review child rejected `--tui` and a later installation selection
made existing Tasks unreachable. Infrastructure owns this execution-continuity
repair. The [incident analysis and working design](https://github.com/loopflowstudio/loopflow/blob/a5d76c576609f6efc80a8ca2198480081066c63c/scratch/jack-heart/session-launch-hardening.md)
preserves observations, unresolved causes and acceptance criteria in local
history; remote availability was not checked. No new Task was filed. Scheduled
delivery (LOO-332), recursive Wave operation (LOO-333), and the Session/Run/Exec
model (LOO-298) retain their separate scope; this prevention does not require
LOO-298's unintegrated model changes.

- **Selected-store absence does not establish deletion.** The switch receipt
  and retained records locate the missing Tasks and failed Run in the prior
  development store. Preserve installation isolation while resolving an existing
  review's executable and data together. Reuse installation receipts and Session
  identity; do not merge private stores or choose between divergent copies by
  timestamp. A restoration preview does not establish applied recovery.
- **Preparation carries context; it does not prove conversation access.** The
  branch's saved `open_argv` carries the executable, Home and database, and its
  startup resolver recognizes matching retained artifacts in settled receipts.
  This addresses a saved handoff after selection changes. It does not establish
  ordinary discovery of an existing Session from another selected store. Current
  preparation semantics belong in [CLI docs](../../docs/lf.md).
- **Failure provenance must survive before provider startup and after retry.**
  Record the attempted executable/digest, owning data, cwd and sanitized outcome
  through existing Run evidence, excluding prompts and credentials. A later
  provider record or successful help check cannot identify an earlier rejected
  child. The original offending binary and the switch's initiator remain unknown;
  parser rejection, provider conversation lookup failure and selected-store absence
  are distinct observations, with no established single causal chain.

At local checkpoint `a5d76c576`, diagnostics name the child and store, but the
recorded Session suite passed only 6/7 tests: the focused retry proof still fails
with “successful retry lost the failed launch evidence.” Source inspection shows
`session open` bypasses the journal wrapper; richer wrapper error formatting alone
does not persist that failure. Recorded installation unit tests passed 23 tests,
and Clippy, formatting and diff checks passed. This curation reruns no behavioral
checks and claims neither shipment nor recovery. The scratch note's uncommitted
status is historical; those edits are now in the checkpoint.

The disposable prepare-under-A → select-B → open-same-review proof and real
Ghostty open/resume remain outstanding. Saved JSON, a window or provider ID alone
cannot satisfy them. Retry must preserve the failed evidence, both stores and
the pending decision without starting a competing Task Flow. Scratch remains the
working design until these obligations are resolved; no Task completion or
chapter change follows from this curation.

## Branch data and command ownership (LOO-321, branch evidence 2026-09-28)

LOO-321 / PR #1308's branch-specific executable/store isolation is superseded by
Jack Heart's one-main-Home decision above. Detailed design, proofs and failures
remain at `5f10576bd8e060e20c6aeb9addca8a462bc838b6/scratch/` in local history.
Installation receipts, retained published fallback and migration recovery remain
required. Never infer successful migration from matching executable versions.

Still-relevant boundaries: preserve Task/worktree/PR identity when continuing;
restart explicitly replaces captured progression. Private fixture databases do
not isolate external provider mutations. Missing checkout recovery retains the
pinned base and branch. Installation promotion must remain separate from Task
execution and passive reads. Four disposable Linux proofs passed; configured
installed-worker execution, real Session/new-draft writes and remote app transport
were not established. Jack had not accepted the app behavior. Deferred gaps were
seeded demo context, private-draft repair, daemon-child routing, OS death evidence
and demo write-back; reselect against current one-Home code before pursuing them.
The oversized Linux prompt defect was outside this scope.


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

## Account auth consolidation (LOO-320, branch evidence 2026-09-27)

LOO-320 / PR #1307 retained six auth leaves, staged browser login, dated usage
observations and explicit account attribution. Current behavior belongs in
[subscriptions](../../docs/subscriptions.md). Jack Heart approved delivery after
demo; cross-account continuation and native refresh coordination remained out of
scope. Exact evidence is retained at `1891b5ea649c3c21780c264fa83ba5b00452045c/scratch/`.

Key lessons: OSC manual URLs and native browser callbacks are distinct; cached
login cannot prove a new authorization. Read-only status must avoid provider
launch/decryption/network/directory creation. Usage omission is unknown, not
zero; expired windows do not reset capacity. Selected account identity precedes
native Session discovery. Credential-file replacement does not coordinate with
native provider refresh locks or close compare/write races.

Live browser-without-paste, first managed connection, remembered Linear profile
and real usage windows remained unproven. Configured Claude returned invalid_grant;
its decoder fixture was synthetic. Copied Homes still referenced live credential
directories and did not isolate refresh. The failed screenshot gate and its
initial-tree fingerprint cannot become a reusable final-tree pass, although
separate Rust/Python/website/static suites passed. Swift/UI remained with CI.
The fixture once launched real Claude because inherited LF_BIN preceded stubs;
clear inherited execution authority and pin source CLI in disposable Homes.
LOO-339/340 supersede parts of this account model, not its historical failures.


## Task deletion and command ownership (LOO-305, branch evidence 2026-09-27)

LOO-305 / PR #1302 implements deletion and command consolidation. Jack Heart
selected provider/local deletion and removal of pm/work; execution settlement
remained deferred. Exact design/demo/incident evidence survives at
`4a14c0a47dc6e04be9668fb72b737828565a931d:scratch/`.

Fresh ownership authorizes provider deletion; explicit acknowledgement confirms
it. Missing membership never proves removal. Confirmed deletion preserves Done
timestamps, historical identity, PRs and Git. Status cannot complete a Task.
Creation validates execution dependencies before allocating; post-create failure
must recover the same identity, not compensate by deleting it. Branch identity
is independent of upstream tracking. Removal does not terminate execution.

A configured demo deleted LOO-299–302 but also advanced installed drafts and broke
the installed CLI. Jack forbade further branch writes/promotion there; LOO-321
owns that recovery. Source verification uses disposable Homes with authority
cleared. Synthetic Linux TLS proofs and focused static tests were not full gate
or configured process-settlement acceptance. macOS TLS ignored child-only CA
settings; the test contract lives in TESTING.md.


## Task convergence (LOO-319, branch evidence 2026-09-27)

LOO-319 / PR #1301's accepted Flow evolved to implement → compress → refresh
(rebase → realign) → loop-decide, with ship calling gate. Jack Heart accepted
refresh's leased push. Captured invocations retain their own definitions;
assert policy from their graph, never copied catalog indices. Realign owns plan
and selected Wave memory. Historical update-wave/record-learnings steps are
superseded, not new work. Evidence survives at
`3abb64bf1a25d4109a8353e22f4a55f664a6f4de/scratch/`.

Persisted Task agent choice precedes captured skill and checkout config.
Exact Task/invocation/boundary ownership fences Ask feedback and navigation.
A failed driver cannot regain authority merely because an Ask completes.
Configured Claude launch/resume, a real five-minute stall and rendered Desktop
agreement were unproven. The real Codex Ask fixture used synthetic feedback and
predated the generic loop-decide revision. It did not prove automatic Home/auth
propagation; old development-Home routing is superseded by one Home.

Stall sampling retains the first PID/start identity and preserves unknowns;
active tool CPU and pending Ask take precedence. Free disk, not aggregate build
size, gates resources. Cleanup must not remove another active checkout's build.


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

LOO-295 owns continuation; LOO-296 broader recovery. Historical detailed proof
is retained at `1a691ac6a222b95c46859c9c06d162d6442950a4:.lf/directions/task-continuation.md`.
The later three-owner model supersedes separate ordinary/Task progress stores.

Capture every router and alternative before execution; recover saved boundaries
without consulting a changed catalog. Invocation identity fences replacement
workers because versions and generations can repeat. Successful provider evidence
must precede decision consumption. Finishing a review or Flow does not complete
a Task. Ask writes serialize and persist exact feedback before teardown; failed
launch recovery preserves native identity and owned live clients.

The live demo required manual binding and provider Home repairs, so it never
proved automatic propagation. Forwarded-account and ambient-auth questions remain
separate from removing injected Codex OAuth. Unknown active attempts require exact
termination evidence; silence or absent listeners cannot authorize cancellation.
Custom/queued/uncaptured history was unresolved before Jack's later import cut.
The old default gate failed because of inherited control pins despite separately
passing suites; no configured recovery/chat claim follows. Pin fixture executables,
clear authority and retain system tools. Follow-up ownership/installed acceptance
remain unresolved; do not duplicate the already implemented Wave deletion.


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

## Task execution authority (branch evidence, 2026-09-23)

LOO-286's September 23 intermediate Task/Run/Playhead model is superseded by
LOO-298's September 30 decisions above. Its exact checkpoints (`11fa65881`,
`b302db9b5`) and failed preservation assumptions remain in the historical memory.
LOO-287 owns later repository-wide reduction; neither completion nor deployed
acceptance follows from these notes.

Retain the obligations: captured definitions survive catalog changes; invocation
identity fences claims and late results independently of version/generation;
only exact owned process evidence authorizes termination. Interactive stops do
not complete Tasks. Ask feedback and navigation remain separate. UI projects
shared state and does not own scheduling. Unknown process state is not death.

Earlier configured proofs remain unestablished: concurrent starts converging on
one worker, actual provider death/same-cursor recovery, late-result refusal,
helper non-authority, app close/reopen continuity and Flow reselection. The old
migration tests explicitly discarded controller-only Task and Project progress;
they never proved lossless cutover. Jack later discarded historical import and
intermediate-draft preservation in LOO-298; do not restore those requirements.
The cause of the old `run_liveness` removal remains unknown. Earlier automatic
idle triggers, finite waits and worker-lifetime questions require reconciliation
with current design rather than duplicate Tasks. Source reads once saw more live
processes than installed 0.12.19; those observations did not establish deployment.


## Observation must not manufacture idle work (2026-09-23)

The task-viewer repair (`7a41fc0b2`, `d7e221fda`, `aabc03966`) removed duplicated
identity/liveness readers after duplicate LOO-293 helpers. WorkCatalog owns stable
Work lookup; TaskExecutionSnapshot projects execution once for status and UI.
Neither grants signal or Flow authority. Current Session ownership supersedes
the old Run-manifest ancestry lookup.

Pinned executable names are legitimate; PID/start identity, not basename, proves
ownership. Missing samples stay unknown. Retain last-good evidence, unresolved
review waits and closed history. A finished launcher does not prove client death.
Old source reads found more live processes than installed 0.12.19 and preserved
them in dry-run prune; they proved neither installed schema nor cutover acceptance.
LOO-286/287 retain unresolved deployment obligations. Test launch isolation must
remove inherited authority and stop only identified fixture children.


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

The global-cmds branch separated installation from checkout integration.
LOO-292 owns installation/rebase acceptance; LOO-287 owns target-first command
scope and ordinary-folder execution. Exact tests, failures and unresolved
receipt redesign survive at `1f2d2c051:scratch/` and the historical memory.

Installation downloads verified published artifacts and delegates store advancement
to candidate promotion. It performs no Git/Homebrew/uv/source maintenance.
First-install recovery has no prior selection: read-only candidate preflight must
work before ordinary startup authorization. Matching versions do not prove exact
schema/app completeness. Bound installed subprocess probes. Published fallback,
artifact bytes, store and switch progress retain separate ownership.

Jack selected positional install cadence: weekly Monday 09:00 by default,
with daily/hourly/five-minute choices and login activation. Source cwd is not a
scheduler dependency. Simulated launchctl passed; actual login/wake remained
unproven. Explicit target commands must not depend on irrelevant Git; genuine
repository-discovery errors remain errors. The legacy Python refresh alias was
removed because PATH delegation could recurse through its source updater.

Machine installation resolves the OS account home. HOME/LF_HOME alone cannot
isolate promotion: use a disposable OS account/container without live installs.
Ubuntu 24.04 reached old-binary promotion where Debian bookworm lacked required
GLIBC. Candidate fixtures passed fresh/repeated install, repair, dirty-repo
preservation, failed-activation recovery and authentic 0.12.18 transition.
Published 0.12.20 still failed clean-home promotion; never overwrite that failure
with fixture success. macOS install and populated historical migration remained
unproven. Await artifact copying and verify archive hashes before serving;
fixture copy races caused different CLI bytes/segfaults. Keep ps on isolated PATH.

LOO-287 retains target-first Work/PM/chat/cron resolution and ordinary-folder
completion without implicit Git checkpoint. New-repository initialization remains
unselected. Receipt deduplication needs interrupted-install proof, not blind field
removal. No generic installer redesign was approved.


## Shipped

Historical shipped entries (PRs #818, #852 and earlier installation/PM work)
are retained in the pre-curation memory. Their old command names and Run-owned
placement model are superseded by current CLI docs and the three-owner model.
Retain these lessons: branch names are hints, never ancestry authority; landing
must not rename a live checkout; deterministic sync classifies authored changes
before invoking an agent; OAuth refresh preserves a rotated refresh token; cron
continuity judges the latest due interval against exact scheduled receipts.
LOO-241's historical-gap fix replaces the older eight-gap daily telemetry blocker.


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

## Planning model (settled, PR #852)

Native Linear hierarchy replaced labels: Wave → Initiative, Project → Linear
Project, Task → issue. Current contracts live in AGENTS.md and the planning
architecture. Stable provider identity survives renaming. Ambiguous ownership
stays unresolved; diagnosis never authorizes guessed repairs. Date-less standing
Projects are allowed. Detailed seeding and retired command semantics remain in
the pre-curation history linked above.

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
