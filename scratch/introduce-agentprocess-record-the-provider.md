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

- `process.rs::Process` and `store/sqlite/processes.rs` own recorded lf invocations.
  Choose **one processes table**, with lf/agent kind and nullable kind-specific
  fields; common identity, PID/birth, parent, command, cwd and terminal evidence
  remain shared. Keep the incoming LOO-441 LfProcess name for lf invocations;
  do not make AgentProcess a fake lf invocation.
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

## Delete — do not maintain (record cut removes these predecessors)

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
- Remaining provider-engine close names and messages, including `close_engine`.
  `bind_group_to_driver`, `prepare_lifeline` and the exposed prepare/retain
  lifeline type are deleted. Keep `engine/` as Loopflow machinery and the
  surviving launch-path proofs.

## Current implementation boundary (reconciled 2026-10-09)

`446cfb2b5` supplies records and shared readers; `ea205e3d0` simplifies observation.
Captured Claude replacement (`6fe75717f`) advances the owner's snapshot without
refreshing stale operations; `a81397932` shares fenced headless admission.
`4f7d21ff7` replaces owned native post-spawn recording with pre-exec admission and
immutable wait snapshots. `961677e04` shares owner lookup and releases SQLite during
attachment close. These slices remain runtime-unverified; unconditional recording,
takeover and public-entry acceptance remain open. Local main remains `3e1e6245c`
(#1512); LOO-441 is not integrated, and no remote dependency state was inspected.

`446cfb2b5` uses `processes.kind` (`lf` / `agent`). The Task's one draft
backfills directly from the released Session shape, including original parent,
PID/birth, endpoint, attachment token, attachment-exit reference and available
spawn/exit evidence. It drops the Session process columns. Native thread and
immutable history stay on the Session/history owners. Duplicate historical
PID/birth observations remain separate, not signal permission. No installed
store was opened with the branch binary.

Claims, transfer, release, connection publication and observed identity now write
the AgentProcess row. Native launch receipts become its lifecycle fields; old
payloads remain history. Actual argv is saved before spawn. A different PID/birth
cannot overwrite a record. The common Process wire projection includes kind,
served Session and OS birth; Rust/Swift mirrors and fixtures changed together.
Provider-generation values still survive on records and caller/status/history
wires: their removal is not implemented.

Top and active-Session views now read recorded AgentProcesses, including detached
and replaced rows, rather than inferring ownership from process trees, native
client receipts or the OpenCode JSON registry. Task membership includes served
Sessions independently of attachment presence; process liveness reads agent
PID/birth. The two orphan modules, ownership projection types, registry and
exclusive directory-watcher/attribution fixtures are deleted. Their required
preservation and public-entry coverage is not all replaced yet. Scheduled
settlement consumes these same rows, rechecks attachment under its lock, refuses
duplicate/unknown signal authority and retains PID/birth after terminal evidence.
It releases SQLite before OS termination and nested evidence reads.

That common inventory does not yet imply common coverage: `reap_in` excludes
interactive agents from live-orphan termination, and `is_agent_process` recognizes
only Codex app-server and OpenCode serve. Claude and OpenCode pre-exec launches
pass no FIFO path; only Codex has a named-lifeline reconnect handoff. These are source
observations, not evidence that orphan termination or takeover works for Claude
or native foreground launches. No broadened signaling rule is accepted merely
because a row exists.

The shared headless launch holds the attachment fence through the parent-side
pre-exec handshake and records failed attempts for Claude, Codex and OpenCode.
Admission remains synchronous: an async cancellation cannot detach an admitted
child before its harness receives it. Owned native launches now hold the same
attachment lock through pre-exec recording, without changing process groups,
controlling terminals or stdio. The headless watchdog setup remains separate from
that shared recording channel; native foreground orphan cleanup is not implemented.
Optional attachment branches still permit unrecorded launches; deleting inferred
discovery does **not** prove those paths are covered. This internal cut must not
publish before they are converted.

Captured native retries prepare a fresh record only after observed exit or spawn
failure and advance capture settlement together. Waits retain their own attachment
snapshot; delayed exit reporting refuses a replacement. A settled capture cannot
spawn again. Child caller provenance uses the newly admitted snapshot. Terminal
reconnect's remote client remains distinct from its already-running provider.
New fixtures cover native pre-exec refusal, retained PID/birth and failed attempts,
stale-launch rejection, PTY descriptors/process-group preservation and capture
replacement with late-exit refusal. All are unrun, including the earlier
Claude/admission fixtures: October 9 recovery still refuses verification below
reserve (30.1/32 GiB).

### Claude replacement snapshots (2026-10-09, source only)

Claude interrupt previously reused a cloned attachment for its next OS process.
The capture owner now prepares each captured Claude spawn: an unused reservation
stays unchanged; observed exit or spawn failure creates a fresh AgentProcess.
Unknown spawn state refuses replacement without overwriting its identity. The
capture's settlement snapshot and the new harness/config snapshot advance
together; old dispatch and history snapshots never refresh from that owner.
A newly constructed retry harness explicitly takes the owner's current snapshot.

Claude spawn holds the attachment fence through pre-exec recording. Interrupt
signals under that same fence and records exit only after waiting for its child.
Seed and steer pipe writes now use bounded fenced dispatch on a blocking thread,
leaving the current-thread runtime free to drive pipe readiness. Replacement
refreshes the child caller environment and preserves the native resume token.
This is an implementation choice under the accepted Task, not new approval.

Added fixtures cover two retained identities, native resume arguments, stale
pipe-write/signal/connection refusal, current-capture settlement, and refusal to
replace an uncertain spawn. They have not run: the last verification attempt
could not meet the disk reserve.
Uncaptured/optional launches and other providers still need the same admission;
this internal slice is not complete Task acceptance or permission to publish.

## Remaining implementation

1. Verify captured Claude/native replacement, shared headless admission and native
   pre-exec recording when capacity permits. Eliminate the remaining optional
   attachment paths in every harness and library/helper launch with invocation-owned
   admission and one row per actual spawn. Distinguish a remote terminal client
   from the provider it connects to. Keep the lock across admission/recording;
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
   unknown attachment liveness stays unknown. Native client-publication and
   interactive-open error branches in `lf/commands/util.rs` kill/wait their child
   but omit AgentProcess exit recording, unlike the normal wait path. Preserve
   the original error and record only a successful exact wait; failed waits stay
   unknown. Resume must consult the AgentProcess
   before treating a detached attachment as replaceable. Unfinished rows with
   unknown PID remain diagnosable. Reconcile zombie and unknown-OS readings across gate/top.
4. Replace remaining predecessor fixture assumptions and restore required
   active-Session, native-history and Task-membership coverage on records. Prove
   headless public top/Task-status/scheduled-entry agreement, not only reducers
   or SQL. The old directory-watcher costs no longer describe this reader. The new
   detached/replaced-record observation fixture covers rename, capture replacement,
   provider snapshots, exact Task exclusion and observed death without payload files;
   it still needs execution. CLI Waiting and work-watch fixtures now join process
   rows rather than deleted Session columns; other fixture repairs remain.
5. Integrate LOO-441's LfProcess/LfSession rename before publication. The current
   source still names `Process` and `AgentSession`; no dependency integration or
   partial publication is claimed. Update the final model/API docs and all wire
   fixtures after the generation cut.
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

The record cut removes inferred provider ownership rather than adding another
inventory. Review found and corrected two preservation defects: migration must
insert AgentProcesses before setting the Session foreign key, and provider name/
interactive launch mode belong to the process snapshot rather than mutable
next-launch Session settings. Orphan settlement cannot hold the SQLite mutex
while its callback reads attached-process evidence. The Claude respawn ownership
counterexample is addressed by the source-only replacement slice above; its
runtime proof is still deferred.

Earlier origin, token and pre-exec decisions remain in `a55f5345b` and its
references. The record cut is checkpointed at `446cfb2b5`, local and unpublished.

Compression removed the empty reader lifecycle, unused OS ancestry/command sampling
and repeated attachment-validation branches. Malformed elapsed-time samples remain
unknown rather than becoming a zero-age birth. Review corrected stale schema claims
in the architecture docs and stale Session-column SQL in CLI/watch fixtures.
Earlier focused passes are recorded at `76ea312a3`, this plan; they do not verify
the subsequent reductions.

Release's child memory was read through October 6: operation-entry tests exposed
recovery gaps that internal helpers missed. Here, public top/Task-status/scheduled
entry agreement remains required even after record fixtures pass. No child files
or configured release state changed. Review of the replacement slice caught two
operational defects before verification: blocking pipe readiness on the sole
runtime thread, and retry harnesses retaining the initial launch snapshot.
Dispatch now uses a blocking worker; only a new harness takes the current owner
snapshot. Compression now removes the three harness-local launch sequences,
reuses Claude's owner lookup, and removes settlement's one-read transaction while
keeping the Session lock. Review rejected an async spawn wrapper because dropping
its waiter could detach a successfully admitted child. A throwaway-child fixture
covers shared admission, stale-launch refusal and recoverable spawn failure; it
remains unrun. Historical capacity evidence remains at `ea205e3d0`, this plan.

October 9 reconciliation corrected the architecture diagram's obsolete direct
Session-to-driver generation fence: the AgentProcess owns attachment/token, and
the accepted LfProcess/LfSession names remain an unintegrated dependency. Local
`main` and `origin/main` still name `3e1e6245c`; no remote freshness is claimed.
Request correlation exists already; its AgentProcess identity conversion does not.
Native failure-cleanup settlement is retained explicitly in remaining work.

Earlier reconciliation corrected dispatch's stale lock documentation: native
writes retain a per-Session OS lock, not SQLite's mutex/transaction. The store
releases SQLite after validating attachment. `off_reactor` yields multithreaded
workers but runs inline on a current-thread runtime; synchronous admission must
not require that reactor. This comment correction changes no runtime behavior.

Review caught a late-wait counterexample: reading the capture's current attachment
at exit could end a replacement. Native launch returns its immutable attachment
to the waiter; the capture rejects late exit reports. Sharing the pre-exec channel
keeps the parent's recording endpoint closed before the watchdog forks.

Compression also removes SQLite's mutex/transaction from provider close in
`finish_session_attachment`: the existing Session lock excludes takeover, then
terminal writes commit together after close. Failure retains the attachment and
history; nested reads and unrelated saves no longer block behind provider I/O.
The new focused fixture checks those boundaries and lock release but remains
unrun. This follows the orphan-settlement boundary rather than adding an owner.
A proposed eager typed-row conversion was discarded: parsing historical parent
IDs before checking for an attachment token would change unclaimed-row reads.

Check: `git diff --check` passes (documentation-only reconciliation); prior fmt pass and 30.1/32 GiB reserve refusal remain at `961677e04`; build, Clippy and replacement/admission/native/close fixtures remain gate/CI-owned, not rerun here.
