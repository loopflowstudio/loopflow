# AgentProcess (LOO-443)

Jack Heart requested this cutover on 2026-10-09. The Task directive is accepted;
the implementation choices below are a draft based on the source inventory at
`3e1e6245c`. Jack authorized parallel work with LOO-441, followed by integration
of its LfProcess/LfSession rename before publication. One architectural PR; no
partial publication or installed-store migration.

## Intended outcome

Every provider OS process started by Loopflow has a durable AgentProcess record.
Top, monitor, resume admission, Task blockers and orphan settlement use those
records. A process disappears from live views on exact observed death, without
erasing its history. A stale attached lf invocation cannot send native writes,
change current attachment state or signal after takeover; retaining provider
history remains allowed. Session/native history, pending input and captured
attribution survive.

## Inventory and draft choices (reconciled 2026-10-09)

- `process.rs::LfProcess` and `store/sqlite/processes.rs` own recorded lf invocations.
  The draft uses **one processes table**, with lf/agent kind and nullable kind-specific
  fields; common identity, PID/birth, parent, command, cwd and terminal evidence
  remain shared. LOO-441 is integrated; the common projection still carries both
  kinds. Separate that projection from the lf-only model without inventing a
  second inventory or making AgentProcess a fake lf invocation.
- Attachment control uses a fresh opaque `AttachmentToken` on every claim and
  release. `SessionAttachment` is a compare-and-swap capability, not another
  process record or lifecycle owner. A → B → A cannot revive A's first token.
  This is the October 9 implementation choice, not a new approval attributed to
  Jack Heart. The AgentProcess retains identity and original parent across live takeover;
  its row now owns the attached LfProcess and token. Numeric provider generation
  still remains in caller/status/history wires pending the complete lifecycle cut.
- OpenCode `start_inner` allocates a dedicated port/server per harness and creates
  or resumes exactly one native Session; stop removes that server. Codex creates
  a private socket per launch and reconnects to the same native thread. New
  AgentProcesses therefore serve one LfSession. Existing duplicate PID/birth
  observations must be retained as migration conflicts, never silently deduplicated
  into signal authority. No configured OpenCode data was queried.
- `ProviderProcess`, `OwnedProviderProcess`, `LiveProviderProcess` in the former top
  implementation reconstructed provider attribution from process trees/receipts and the
  OpenCode JSON registry. `SessionProcessObservation`/`SessionProcessOwnership`
  add another SQL projection of the Session's loose process columns. The record cut replaces
  this composition with records plus a read-local OS observation. Unknown external processes confer no signal authority.
- The former `engine_orphans` read Session columns separately and rechecked under the Session
  lock. Preserve exact PID/birth, command/group checks, takeover serialization,
  unknown-evidence refusal, and descendant cleanup in the surviving record path.
  Its deleted `recorded_engines` query only included PID/birth groups with a non-null
  attached LfProcess; a detached provider is omitted when no peer retains an
  attachment. An in-memory execution of that exact SQL confirms the omission.
  The replacement inventory must include unfinished AgentProcesses independently
  of attachment presence. One shared table alone does not fix this reader rule.
- The separate post-spawn lifeline bind is deleted. `spawn_agent_process` consumes the command,
  prepares the lifeline, then retains it only after successful spawn. The
  pre-exec child establishes its group and awaits watchdog readiness before exec.
  This preserves provider PID == PGID and native stdio/argv/spawn-error behavior.
  Only async-signal-safe operations may run after fork. Keep dash-compatible
  `kill -s TERM -- -pgid`. Native foreground terminals need their own process
  control treatment; no headless process-group change may break their TTY.

## Predecessor removal (completed and remaining cuts)

- Session `provider_pid`, `provider_started_at`, `provider_endpoint`,
  `provider_generation` and `provider_process_lfid`; replace with AgentProcess.
  The record cut moved `attached_process_lfid` and `attachment_token` there.
  `SessionDriver` and `driver_generation` are removed in the attachment slice;
  historical migration fixtures retain released field names. No compatibility
  writer or second attachment counter remains.
- `provider:<n>:reserved/spawn_requested/spawn_failed/exited` writers/readers:
  use AgentProcess lifecycle, retaining historical event payloads as history.
- `harness/engine_orphans.rs`, `store/sqlite/engine_orphans.rs`: absorb exact
  orphan settlement into the same record reader used by Task gates.
- `harness/opencode_runtime.rs` and its JSON registry writer/reader/fixtures.
- Top's ProviderProcess/OwnedProviderProcess/LiveProviderProcess attribution
  layer; SessionProcessObservation/SessionProcessOwnership and exclusive tests.
  `ActiveSessionReader`, its no-op invalidation and async wrapper are also deleted;
  one synchronous record/OS snapshot serves one-shot and watch reads.
- Per-harness spawn recording is replaced by `harness::agent_process::spawn`.
  Native post-spawn writers (`begin_provider_spawn`, `record_provider_process`
  and terminal util's post-spawn block) are deleted; `spawn_native` owns native
  admission. Both use one parent-side pre-exec recording channel. Shared
  `open_owner` replaces three harness-local store-opening sequences without
  refreshing their attachment snapshots.
- Metadata-only `CaptureHandle::fail_and_begin_attempt` is removed; invocation
  retry settles/replaces the record before advancing capture metadata. The private
  metadata reducer remains for history/usage projection.
- Native `spawn_native(None)`, capture launch without an attachment, and
  `native_provider_driver`'s missing-provenance-as-client inference are removed.
  Raw headless optional admission remains a deletion target.
- Top’s private OS sampler/elapsed parser and receipt-only birth comparison are
  removed. `journal::OsProcess` owns single-PID and inventory observation, including
  zombie rejection. Malformed samples fail observation rather than inventing absence.
- Remaining provider-engine close names and messages (`close_engine` is removed).
  `bind_group_to_driver`, `prepare_lifeline` and the exposed prepare/retain
  lifeline type are deleted. Keep `engine/` as Loopflow machinery and the
  surviving launch-path proofs. Codex's write-only `endpoint` and OpenCode's
  mirrored `child_group` are removed; Codex retains its pre-handle group slot.

## Current implementation boundary (reconciled 2026-10-09)

`446cfb2b5` supplies one `processes` table (`lf` / `agent`), attachment tokens,
and shared record readers; `ea205e3d0` removes inferred discovery. The single
Task draft backfills parent, PID/birth, endpoint, attachment and available
spawn/exit evidence before dropping Session process columns. Duplicate PID/birth
rows stay separate and non-signallable. Native thread/history stay on the Session;
provider generation still survives in records and caller/status/history wires.

Top, active Sessions, Task membership and scheduled orphan settlement query
unfinished AgentProcesses, including detached/replaced rows; live views still
filter out unknown identities. Settlement rechecks
attachment under the Session lock without holding SQLite across OS I/O. It still
excludes interactive agents and recognizes only Codex app-server/OpenCode serve
for live-orphan termination. Only Codex supplies a named FIFO for reconnect;
Claude/OpenCode anonymous lifelines do not establish takeover coverage.

Shared headless and owned native admission record argv and OS identity before
exec, under the attachment lock. Native admission preserves foreground groups,
TTY and stdio. Synchronous admission cannot lose an admitted child to cancellation
before returning it. Admission consumes each reservation once; a duplicate cannot
mark a running row spawn-failed. Raw headless harness paths still permit missing
attachments; captured native admission now requires one.

Captured Claude and native retries reserve a fresh AgentProcess only after exact
exit/spawn failure, advancing capture settlement and the next launch together.
Dispatch/history/waits retain immutable snapshots; old waits cannot end a
replacement. Only newly constructed harnesses refresh from the capture owner.
Claude dispatch uses bounded writes on blocking workers, not the sole reactor.
Native setup cleanup records successful waits, retains failed-wait uncertainty,
refuses stale cleanup and never settles a remote client's provider. Settlement
holds the Session lock, not SQLite, across provider close.

`263b6adfb` removes Codex's write-only endpoint and replaces its
unshared atomic group slot with an owned optional PID. It retains that slot
through failed startup before the child handle is installed. OpenCode derives
the group from its child instead of mirroring it. Native publication/opening
failures share one cleanup path; a published client guard survives through
cleanup. Codex relay and close terminology now uses attachment/AgentProcess;
retained socket/FIFO names remain unchanged. This does not complete the broader
vocabulary or optional-launch cut.

LOO-441 is integrated from pinned main `461577746` (#1516) through the owned
sync. No publication, installed-store migration or installed acceptance is claimed.
Earlier implementation/review detail and contrary evidence:
`e0d360e6a:scratch/introduce-agentprocess-record-the-provider.md`.

## Remaining implementation

**Invocation-owned replacement (2026-10-09):** the library admission
counterexample is repaired without weakening the saved-thread check or changing
Codex's takeover-preserving `stop`. Retry now holds the capture's exact attachment,
closes or observes the old AgentProcess dead, and atomically retains its end,
reserves a fresh identity and selects the next native thread. Failed close or
stale authority leaves the old record unchanged. Only the capture owner advances
its snapshot; pending dispatch/history keep theirs. Account failover explicitly
clears the resume selection, including one inherited from the original launch;
same-account retry retains its thread. Prior account/native observations remain
history. Metadata-only retry is no longer a production capture API.

Settlement reads the recorded provider/interactivity, not next-launch settings.
Unknown or duplicated OS identity grants no signal authority; native foreground
providers keep their launcher-owned teardown. The owned Codex close uses the
shared zombie/identity judgment. Generic harness teardown errors now propagate.
The regression uses different native threads for revoked and fallback accounts
and checks distinct ended AgentProcesses, one Session/parent and retained
thread/account pairs. Its socket fixture exits on disconnect; the separate
throwaway-group check exercises live close with an absent endpoint, not a live
Codex thread-inventory exchange. Configured-provider acceptance remains open.
Earlier failing proof and analysis: `3f530ffce:scratch/introduce-agentprocess-record-the-provider.md`.


1. Eliminate optional attachment paths in raw headless harness starts. `run_agent`
   now enters the existing invocation runtime before creating its implicit capture;
   nested calls reuse their parent. Native helpers also admit at entry, creating
   a capture only when absent. Native spawn requires an attachment; missing
   provenance no longer selects the remote-client path. Explicit remote endpoints
   are checked against the saved AgentProcess, and client spawn holds its attachment
   fence without recording provider exit. Library account failover now uses the
   replacement above. Raw `Harness::start` remains: its
   optional config/open_owner/headless spawn still
   permits unrecorded children. Admission belongs at the invocation entry, not in
   a spawn callback inventing a parent. Keep the lock across admission/recording;
   failed spawn is non-start evidence and uncertain spawn remains unknown. Native
   foreground orphan handling must preserve TTY/process-group behavior.
2. Remove numeric provider generation from runtime caller/status fences in favor
   of AgentProcess identity. Request correlation already freezes Process, Work and
   capture before send in Claude, Codex and OpenCode history; `SessionTurnOrigin`
   still stores provider generation, not AgentProcess identity. Preserve that
   correlation while moving history and performed-work filters to the agent record;
   neither a delayed Started event nor a process's launch capture identifies a
   later request. Rewrite this same migration, never add another Task draft.
3. Complete stop/release authority across providers, both takeover death orders,
   and FIFO ownership. Cover Claude/OpenCode anonymous lifelines and the reaper's current
   Codex/OpenCode-only, noninteractive selection without applying headless group
   control to a foreground TTY. `HELD_LIFELINES` still retains superseded writers
   until lf exit. No current live attachment means orphan settlement, not invented exit;
   unknown attachment liveness stays unknown. Resume must consult the AgentProcess
   before treating a detached attachment as replaceable. Unfinished rows with
   unknown PID remain diagnosable. Top, active Sessions, receipt checks, gate and
   reaper now share `journal::OsProcess` parsing and identity judgment; zombies
   are dead, invalid observations remain errors/Unknown. This removes the prior
   source-level disagreement, not the missing-identity display gap below. No
   configured orphan was signaled and OS death supplies no successful outcome.
4. Replace remaining predecessor fixture assumptions and restore required
   active-Session, native-history and Task-membership coverage on records. Prove
   headless public top/Task-status/scheduled-entry agreement, not only reducers
   or SQL. Source inspection at `48aaf72a1` separates three visibility gaps:
   - `collect_activity` drops AgentProcesses lacking PID/birth; valid absence,
     birth mismatch and zombies are dead, not unknown rows to resurrect.
   - `load_snapshot` selects LfProcesses through receipts, unlike the gate's
     `open_processes` inventory. An unfinished same-boot lf row without a receipt
     can block a Task without ever reaching activity projection.
   - OS sampling errors fail the entire top snapshot (and prune before reaping);
     active Sessions clear their list and report Unavailable. The gate retains
     Unknown. `ActivityState` has no unknown case yet.
   Rendering needs recorded identities plus explicit unavailable observations for
   both kinds, with Rust/Swift/DTO changes together; it must not infer liveness or
   control from display. Public fixtures need missing receipts, missing agent
   birth, failed sampling and exact death, preserving caller-lineage exclusion.
   One OS parser does not supply equal record selection or error handling. The
   detached/replaced-record observation fixture now passes: rename, capture
   replacement, provider snapshots, exact Task exclusion and observed death
   without payload files. This does not exercise public commands or uncertain OS
   identity. CLI Waiting and work-watch fixtures now join process
   rows rather than deleted Session columns; other fixture repairs remain.
5. LOO-441's LfProcess/LfSession rename is integrated from #1516. Update the final
   model/API docs and all wire fixtures after the generation cut. The common
   LfProcess projection still carries both kinds during this draft cutover.
6. Gate owns affected Rust/Swift/DTO and materialized-migration verification plus
   Linux lifeline checks. Demo still requires exact SIGKILL removal within two
   seconds and no invisible Task blocker. Only the installed scheduled path may
   settle Jack Heart's three pre-#1512 orphaned Codex processes. Branch fixtures
   never signal them or migrate that store.

## Preservation counterexamples

Live takeover must not kill the continuing AgentProcess when its original parent
exits. Same-PID/different-birth evidence never authorizes signaling. Stale writes
must fail even while the AgentProcess survives. Unknown or shared historical
ownership must not become a kill permission. Native history and unfinished
captures survive migration and failed admission. The watchdog must not inherit a
lifeline writer or hold provider stdout/stderr open.

## Implementation review

Prior review corrected migration insert/FK order, mutable next-launch settings
leaking into process snapshots, SQLite held across close, stale retry/wait
snapshots and duplicate launch settlement. The exact findings, fixture fixes,
checks and source references remain at `e0d360e6a`, this plan. Release's
operation-entry lesson still applies: internal helpers do not prove public
top/Task-status/scheduled agreement or installed settlement.

This reduction retains Codex's pre-handle group slot: deriving it from `child`
would lose failed-startup cleanup. OpenCode installs its child before exposing
the group, so no second slot is needed. Combining native setup errors must not
drop the published client guard before cleanup; it remains in the outer scope.
No new signal authority, attachment refresh or lifecycle owner is introduced.

October 9 realignment read Release's GOAL and full MEMORY, the only immediate
child scope found here. Its operation-entry lesson applies to both visibility
and scheduled settlement: shared internals do not prove public outcomes. Local
main remains `461577746`; #1512 and #1516 are integrated. No additional upstream
integration is claimed. The lifecycle and visibility gaps need implementation,
not a new product decision or relaxed acceptance.

Review caught two boundary errors and repaired them: native clients must hold the
attachment fence at spawn, and a helper-created capture must finish with the
actual command result rather than its drop fallback. Native helper fixtures now
select the same test ledger as their Session, preserving FK ownership.
The network runner clears `LF_BIN`; initial library probes consequently reached
installed Claude in a fresh fixture provider home and got login failure, with
external networking denied. No configured-provider acceptance is claimed.
Integration EnvGuard now pins the compiled CLI inside the boundary. Source review
also removed the eager cwd lookup when an explicit working directory is supplied.

Prior admission checks: `91d184b1c:scratch/introduce-agentprocess-record-the-provider.md`.
Compression moves OS observation below presentation and deletes the native-client
elapsed-time parser. Invalid ages report unavailability; missing PID/birth rows
still need visible unknown nodes. Replacement review preserved the generic stop
boundary, corrected closing against mutable Session provider settings and kept
telemetry loss nonfatal after the durable replacement transaction. The live-close
fixture's invalid trace identity was repaired before its passing run.

Check: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, isolated lib filters `invocation_`, `saved_thread_rejection`, `subscription_limit_fails_over_without_resuming`, `telemetry_loss_cannot_keep_retry` (7 tests) and `agent_tests` (14 tests) pass; broader Rust/Swift/DTO/materialized and Linux checks remain gate/CI-owned.
