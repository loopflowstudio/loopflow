# infrastructure wave memory

Renamed from `systems` in the 2026-07-08 wave/project/task restructure. Owns dependable self-hosting, verified releases, and architecture minimalism. The configured release schedule and accepted proof obligations govern current work; older nightly/weekly notes below are historical.

Release-specific findings live in [release memory](release/MEMORY.md).

The 2026-09-30 [LOO-298 decisions](#data-model-and-performance-decisions-reconciled-2026-09-30)
supersede older Run-owner, historical-import, pinned-development-Home and
demo-before-landing directions for this cutover. Earlier incident observations
remain evidence of their own versions, not instructions to restore those owners.

Historical detail retired for context budget is preserved verbatim at
`fe36937faf75151a2f5d7195b32ff708ce335e86:wave/infrastructure/MEMORY.md`.
Sections below marked as retained lessons summarize that tree; their original
incident receipts and unresolved proof limits remain recoverable there.

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
an owner does not resume it. Same-Home closed continuation remains unfinished;
old-Home authority is never transferred by attribution. Child-held mutation locks
and exact checkout leases protect different scopes; cleanup must independently
reacquire after dropping the parent's shared handle. Parent death and elapsed
wait grant neither mutation nor deletion authority. Main-reset/stash helpers can
replace a held lock inode; explicit source selection avoids that failure class.

The full child Release memory and goal were read in this reconciliation; it is
the only immediate child scope in this checkout. Its September 28 incident shows
why operation-level recovery must survive the scheduled entry point: an agent's
successful report of a failed release produced a misleading green cron receipt.
Jack's later steer records v0.12.24 publication/install and skill-to-Flow activation
at unchanged 10:00. That supersedes the child's dated pending-activation evidence,
without proving this accounting branch is installed or either qualifying outcome.
Release-specific detail remains in [release memory](release/MEMORY.md).

All 36 telemetry failures, including the original 35, remain dated counterevidence
from September 24. The missing `agent_turns` diagnosis is historical: integrated
scorecard source consumes SessionHistory. No configured current pass or accepted
Intelligence handoff is established. Reproduce any remaining failure before
commissioning duplicate analytics repair. Doctor still requires natural Scheduled
evidence; Recovery cannot erase missing firings. Required UI/public proof and
actual automatic settlement observations remain outstanding. The working plan
owns remaining implementation; no production release, install, schedule change,
Home transfer or review completion is authorized by local reconciliation.

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

## Synced planning lessons (LOO-334, September 29–30)

Jack selected one local planning interface: configured Linear authority for
connected repositories and private plans otherwise. Git shares Wave definitions,
Flows, Skills and bindings; this does not grant teammate execution authority.
Repository files establish Waves, never all provider Initiatives discovered during
reads. Stable Project IDs survive rename; current membership cannot reconstruct
predecessor history. Full intermediate implementation/proof remains archived.

Keep presence, freshness and execution eligibility separate. Missing list entries
and failed refreshes cannot establish deletion. Entity revisions differ from
membership acquisition time, and Project revisions cannot order unrelated Team
or Initiative relationships. Partial webhooks invalidate rather than supply facts;
unknown revision must still invalidate. Preserve removal evidence arriving during
acquisition; a null response cannot downgrade confirmed removal to absence.
Contradictory ownership persists outside rejected transactions for explicit repair.

Observational status cannot complete Tasks. Chapter retry retains stable identities,
unfinished Task/PR/worktree/invocation and frozen KR/metric results. Provider
completion precedes archival; lost responses require provider readback. External
archive/restoration and configured Linear completion were not proved by the
stateful local fixtures. Optional chapters/default Flows now follow October 2
accepted direction above. Cached outage admission, no-remote definition policy,
outward sync and populated cross-repository relocation remained unresolved in
this record. Alias repair moves Wave and normalized planning atomically, without
merging contradictory observations.

Jack selected the official runtime at new worker boundaries independently of the
store, with deliberate visible pins propagated recursively through Flow/provider
children. Ordinary installation must preserve pending review/Task continuity;
source fixtures and manual Home selection cannot establish that acceptance.
The later one-main-Home and Session/Exec integration supersede the old intermediate
schema bridge and Run ownership, without restoring a historical importer.

Recorded planning/migration/Rust/Swift and five container proofs were local,
not a full live command story. Process-global tracing made concurrent log capture
misleading; isolate the subscriber/process rather than add retries or production
test hooks. Claim acquisition is not Started, driver death is not provider death,
and coordination agreement is not shipment or completed acceptance.

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

## Branch-data incident lessons (LOO-321)

The one-main-Home direction above supersedes LOO-321's source snapshots, coupled
runtime/store selection and promotion model. Full #1308 evidence and its earlier
contracts remain archived. Preserve live installation data and native history;
source proofs use disposable Homes. A seeded copy, prepared review or green
fixture never establishes installed-worker, configured-provider or Desktop
acceptance. Historical Linux oversized-prompt and remote app transport gaps were
not resolved by the recorded four installation-container passes.

Keep source selection separate from execution authority, saved continuation
separate from new Flow selection, and current placement separate from display.
A removed checkout can be recovered from retained Task/branch/PR identity without
resetting history. Diagnostics must name exact preserved artifacts/data and
unknown evidence; returning to old bytes is not a merge of private writes.
Retired Wave controls do not remove remote Home placement or schedules.

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

## Account auth consolidation (LOO-320, September 27 evidence)

Jack approved #1307's delivered scope and delivery through the saved Flow.
Cross-account Session continuation, native refresh coordination and ranking new
Sessions by headroom were excluded. Current behavior lives in subscriptions
documentation; full accepted design, failures and receipts remain archived.

Native OAuth owns callback completion: Claude's OSC manual URL was not the browser
handoff URL. Cached login, usage windows and identity acceptance prove different
facts. Read-only cached status must avoid provider launch/decryption/broker work;
forwarded leases without a catalog remain uninspected. Persist dated returned
windows without turning omitted/expired data into zero or fresh capacity. Record
actual account selection per attempt, never inherit a retry's previous choice.
Loopflow's flock/atomic replace does not prove native refresh interoperability.

Browser login without pasted code, first managed connection, remembered Linear
profile targeting and real provider usage windows remained unproved. Configured
Claude returned `invalid_grant`; route/status disagreed on missing credentials,
and a copied database still referenced original credential homes. PTY secrecy,
manual fallback and installed acceptance also remained open. The recorded gate
passed affected Python/Rust/website/static checks, but one fetch was manually
terminated and its initial-tree fingerprint predates edits; it is not a reusable
unattended final-tree receipt. Swift/UI/slow checks remained with CI. LOO-339's
identity work above supersedes earlier cached-identity assumptions without
settling these unrelated live proofs.

## Task deletion lessons (LOO-305, September 27)

Jack scoped #1302 to provider/local deletion and command consolidation; execution
settlement remained deferred. The full design, proof and installed-database
incident remain archived. Task/Wave/repo commands follow their objects; current
CLI docs supersede retired `pm`/`work` spellings.

Fresh ownership authorizes deletion; provider acknowledgement or explicit trash
confirms it. Missing membership does neither. Narrow atomic writers retain Done
outcomes, merge evidence, timestamps, PR/Git identity and unrelated refreshed
facts. Completion and planning remain separate writers. Post-create allocation
failure needs recovery rather than compensating deletion. Branch identity is not
its upstream; historical attribution resolves independently of launch eligibility.
Removal never establishes provider death or all work's settlement.

The configured deletion demo removed LOO-299–302 and verified absence but also
advanced installed drafts, breaking the older CLI. LOO-321 owned that recovery;
Jack prohibited branch promotion or further branch-binary access to the installed
Home. Source tests require disposable Homes with inherited authority removed.
Linux TLS/provider fixtures and focused/static passes do not prove installed
acceptance or complete Session/process disappearance. Current Task admission and
completion work above supersedes unrelated Flow prerequisites from this record.

## Task convergence lessons (LOO-319, September 27–28)

Jack selected implement → compress → refresh → loop-decide; refresh integrated
rebase and realign, with publication before the existing review boundary.
Captured invocations retain their authored definitions. Historical update-wave,
review-slice and direct-demo-landing directions are not current navigation.
Current Flow files and saved captures own execution; the full dated evidence
remains archived. Reconciliation edits the plan and identified Wave memory;
readiness alone cannot release a caller or select a verdict.

Task agent choice outranks captured skill and checkout defaults. Exact selected
completion and keyed feedback fence progression; a failed attempt cannot regain
authority by looking ready. Provider tool commands need the intended executable
and Home; PATH-only repair failed the historical fixture. The real Codex policy
proof used synthetic feedback and predates generic loop-decide. Configured Claude
launch/resume, real five-minute stall and rendered Desktop agreement remained
unverified. CPU/event observations do not confer signal authority; preserve the
first observed process identity and unknown samples. Current resource/fixture
rules live in TESTING.md. The one-main-Home and Session/Exec decisions supersede
old control-pin and Run persistence mechanisms in this evidence.

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

**Integrated gate, September 30:** the archived memory retains the exact
`5dee46ca8` receipts. The materialized Rust run had seven failures, each followed
by focused repair passes; the original receipt remains failed. Python/website
passed; Swift required one fixture repair. Static checks passed. This does not
establish a full final-tree gate, hosted outcome, installed conversion or provider
acceptance. The candidate's production-prefix estimate was +6,202 lines, not a
net reduction. Dense cold/warm CLI timing and final gate remained outstanding.

Preserve stable-ID ordering through Session projection or renamed titles can
repeat/skip rows. Stacking fixtures need real published-parent facts and the
stacking transaction; generic PR writes cannot change parentage. A bad fixture
launched a real conflict agent before stubbing that path; reported lack of push
is not a credential-effects audit. Automatic checkpointing must use the same Work
binding reader as execution to avoid committing another contributor's work.
Captured Skills outrank a mutable catalog on recovery. Worker claims must name
the worker, not its launcher. Direct execution and Task execution share provider
retry/account handling; no parallel worker policy is needed.

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

## Continuation and recovery lessons (September 25)

LOO-295 owns continuation; LOO-296 broader restoration. Full historical proof,
manual demo repairs and unresolved follow-ups remain archived; neither Task is
completed by curation. Current Session/Exec ownership supersedes old Run writers.

Capture all alternatives and review policy before execution. Saved work must
survive missing source, and late replacement drivers must not consume another
invocation's feedback. A ready candidate is not a successful completion. Atomic
cursor writes cannot establish exactly-once external effects. Serialize review
writers, retain native identity and evidence, and distinguish unpublished launch
recovery from a live provider. Prove the driver as well as its reducer.

The live demo required manual Flow/native-Home corrections, so did not prove
propagation. Removing stored Codex OAuth injection addressed that login rejection,
not forwarded lease or account consistency. Unknown active attempts require exact
termination evidence before cancellation; listener absence is insufficient.
Historical queue transfer and configured chat/interruption/failure recovery
remained unproved. The recorded broad gate had a failed default receipt despite
separate passing suites after control-pin removal. Preserve that distinction.
Fixture isolation must pin the executable and retain system prerequisites such
as `ps`; an installed development CLI can otherwise mask a clean-host failure.

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

## Chapter preservation lessons (September 23)

The accepted chapter retains Infrastructure's execution/auth/placement/release
mandate; Product judges experienced usefulness and Intelligence owns observation.
Current October 2 optional-chapter decisions supersede mandatory resets. Full
chapter application receipts and earlier CLI limitations remain archived.

Historical archives stay outside live architecture discovery, preserving exact
failed vocabulary observations. Stable IDs survive PM renames; current labels
cannot rewrite captured execution. Read back provider writes: concurrent curation
once replaced a Task description after an apparently successful update. Deduplicate
Work IDs before dispositions; provider completion is not shipment or a won KR.
Consolidating files does not rebind registered Task/PR/worktree identity. The old
LOO-278 placement mismatch required supported preservation/adoption, never direct
registry edits. Replaying an archived verifier proves retained receipts, not
fresh provider state. Run-era reader mechanisms are superseded; the distinction
between bounded recent history and exact uncapped ownership discovery survives.

## Task execution authority — retained constraints

The September 23 LOO-286 Run/Playhead/task_flow_positions implementation is
superseded by the September 30 Session/Exec model and current Task membership
contract above. Detailed incidents and proof remain in the archived memory.
LOO-286/LOO-287's configured provider, concurrent-start, exact recovery, Desktop
continuity and installed projection obligations were not established by the old
fixtures. The old migration dropped controller-only Task and historical Project
positions; its passing test was not lossless-cutover proof. Later explicit
current-state conversion decisions govern, not a restoration of those old owners.

Keep captured definitions independent of today's catalog, fence late results by
exact invocation/claim, and distinguish process death from silence or age.
Helpers gain neither cursor nor signal authority through attribution. Optional
Wave hosting cannot gate ordinary Task progress. Preserve unreadable historical
records and unexpected authored work; explicit disposition precedes destructive
recovery. UI projects shared records instead of inventing controller Sessions.
The operational cause behind historical `run_liveness` deletion remains unknown;
deleting fallback prose did not repair ordinary execution. Idle triggers, worker
lifetime and installed continuity were unresolved in that dated evidence, not
permission to widen current scope or create duplicate Tasks.

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

## Installation and command scope (branch evidence, 2026-09-24)

The `global-cmds` branch separates machine installation from checkout updates.
Infrastructure's existing release/recovery mandate owns these learnings;
this curation does not bind the branch to a Task or change Wave identity.
LOO-292 retains installation/rebase acceptance; LOO-287 retains command-scope
reduction follow-ups. Both remain open. Their updated notes supersede the older
combined laptop/package/main refresh requirements, without discarding the
reported main-rebase failure or its preservation obligations.

### Keep the ownership boundaries

- `lf install` selects the latest published release and verifies its pinned
  shell installer. The shell downloads artifacts; the existing candidate
  promotion transaction alone creates/advances the store and activates bytes.
  `lf rebase` owns checkout refresh/integration and keeps its journal evidence.
  Installation must not run Git, Homebrew, uv, Python or source-tree maintenance.
- First install has no prior selection or previous published fallback. Before
  candidate handoff it can cancel to uninstalled; afterward the pinned candidate
  must recover and settle. Ordinary commands cannot use an uncommitted first
  selection. Read-only install preflight must reach its own authority checks
  before ordinary startup authorization, or first-install recovery deadlocks
  on the absent prior selection. Existing-store adoption still proves its
  store/artifact authority.
- Matching CLI/daemon versions alone do not mean current: exact-store preflight
  must succeed without migration, and macOS must have the complete matching app.
  Bound subprocess inspection so broken installed binaries cannot hang updates.
- Older CLIs transition through the external installer. A retained Python
  `refresh` alias delegating to PATH `lf install` recurses through the old CLI's
  source updater. The alias and dependent wrapper are deleted; do not restore
  them as a compatibility bridge.
- Optional discovery returns a real checkout or absence, preserving genuine
  Git failures. `CanonicalRepo::current` cannot accept an ordinary folder as
  repository scope: doing so produces plausible empty Wave/Session listings.
  Stored-locator discovery and folder prompt `working_directory` have different
  contracts. Provider `RepoId` still owns origin identity and Git URL rewriting.
- Machine commands bypass repository runtime capture; catalog/flow inspection
  must not create journal attribution. Keep explicit target/default-route
  resolution independent of irrelevant caller Git. Do not replace missing repo
  identity on a route mutation with a default-route write.
- Scheduled installation retains its existing launchd label/log destination
  and custom `LF_INSTALL_DIR`, but no source WorkingDirectory or Python. The
  accepted cadence is login plus weekly by default (Monday 09:00), with daily
  09:00, hourly, or five-minute clock intervals available explicitly through
  `lf install schedule [weekly|daily|hourly|5min]`. The clarified requirement is that
  cadence is positional, without `--every`. Calendar scheduling should coalesce
  sleeping intervals. The concurrent scheduling receipt records a public-CLI
  cadence matrix and static passes with simulated launchctl; match final proof
  to the positional-argument bytes. It does not prove actual login/wake events.

### Evidence and demo traps

Review through `ac0b51aad` records 67 focused tests and static checks, plus three
earlier checkout tests. The disposable Ubuntu 24.04 candidate demo passed fresh
installation without Git, repeat without asset downloads, missing-daemon repair,
dirty-checkout/ref and migration-ledger preservation, failed-activation recovery,
and external transition from authentic checksum-verified 0.12.18 binaries.
That older account had no populated historical Home. The candidate reported
0.12.19 with drafts materialized only in a disposable source snapshot; local
HTTPS release transport is not a public release. Captured logs and full audit
remain in branch history at `1f2d2c051:scratch/` after scratch is cleared.

The real published 0.12.20 demo failed clean-home promotion after preflight
accepted the absent store. Preserve that observation; candidate success cannot
rewrite it. Public-channel acceptance needs a released fix and repeated
fresh-install/repair proof. Real macOS app installation and populated historical
Home migration are still unproven. No full CI or current-tree pass is implied.

- Machine installation resolves the OS account home, ignoring `HOME` overrides.
  `HOME`/`LF_HOME` alone cannot isolate promotion. Use a disposable OS account or
  container with no real installation mounted; remove obsolete installer tests
  whose mocks now permit the real downloader. PATH mocks for Homebrew or uv do
  not intercept HTTP downloads or promotion. Include failed first activation and
  pinned-candidate recovery: fresh-install success alone misses dependencies on
  prior startup selection. See `TESTING.md` and `tests/e2e/install_bootstrap.py`.
- Debian bookworm could not run the downloaded release's GLIBC_2.38/2.39
  requirements; Ubuntu 24.04 reached promotion. Distinguish platform failure
  from repository discovery failure.
- Wait for artifact copies before packaging/serving, then compare archive-member
  SHA-256 with source/runtime bytes. An asynchronous copy race produced a
  different CLI and a segfault; the daemon matched. TLS fixtures also need a
  separate server leaf signed by their test CA. These were fixture failures.
- Removing Git from PATH must retain unrelated runtime prerequisites such as
  `ps`. A missing prerequisite is not evidence that the command needs Git.

### Reduction and unresolved scope

The shell bootstrap and installed release selector share one promotion owner.
Artifact bytes, selected install/store, retained published fallback, and durable
switch progress remain distinct. Advancement may finish while phase still says
`Advancing`; phase alone loses recovery evidence. Persisted duplicate receipt
facts need a coherent format change and interrupted existing-install proof,
not isolated field deletion. No such redesign was selected in compression.

LOO-287 retains target-first Work/PM/cron/chat/bound-invocation resolution and
ordinary-folder execution through provider completion without an implicit Git
checkpoint. Recheck downstream credentials/config and explicit worktree flags;
no capability registry, skill-name allowlist, fake repo, or swallowed Git errors.
The scope audit is source evidence, not proof that every command ran. Starting
a new repository remains an unselected design question: separate Git creation,
local configuration, optional provider connection and later PM setup. Today's
`init` connects an existing repository to the distributed system.

## Historical shipped index

Detailed PR #818 placement/rebase history and PR #852 native Linear migration,
skill installation, OAuth pre-emption and aligned PM output are retained in the
archived memory. Their retired command names and Run-based placement model do
not override current docs. Stable identity remains independent of names; never
infer ownership from a directory delimiter or derived Project slug.

LOO-241's durable lesson remains current: unchanged cron sync preserves activation;
legacy receipts can establish it. Doctor judges the latest due interval against
an exact Scheduled receipt, even when its target failed. Raw gap days remain
history. Product settlement needs separate release evidence.

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

## Durable infrastructure boundaries

Self-hosting remains the default; secrets belong in Doppler, not Git. Current
Wave goals own schedules; historical nightly/weekly notes are superseded. Release
owns automation while each product owns content and provider credentials. No
generic deployment platform is justified without another real product.

One writer per worktree is dispatch discipline, not a general lease. The database
is durable control state, not a message bus. Authored direction remains durable;
observation delivery must not invent another execution authority. The retired
radio/epoch/Run designs remain historical evidence only.

Opening Wave chat uses the existing local Home lifecycle; it neither promotes
binaries nor moves placement. Attempt-scoped live/failed boot evidence and private
wake edges distinguish startup acknowledgement from reconciliation polling.
A failed Wave must not terminate successful siblings. Remote app transport and
configured startup acceptance require their own proof.

A status read observing merge cannot complete a Task or mint a synthetic agent
record. Completion belongs to its authorized atomic domain writer. Performance
observations preserve missingness, original attribution, terminal-time windows
and measured/eligible coverage. Absent authority is unknown; timestamps, trace
text and zero cannot fill it. Budgets judge evidence, not correctness.

## Historical planning model

PR #852 established native Wave → Initiative, Project → Linear Project and
Task → Issue. Stable provider IDs, not derived slugs, own identity. Missing list
membership proves neither deletion nor terminal disposition; read back provider
writes. The label migration and retired `pm` commands are archived history.
Current planning architecture and the October 2 optional-Project/Flow decisions
above supersede mandatory-chapter assumptions. `doctor --planning` diagnoses
ambiguity without acquiring mutation authority. Date-less standing Projects
remain legitimate; metric completion still requires outcome evidence.

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
