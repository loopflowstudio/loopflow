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
  Move `attached_process_lfid` and `attachment_token` into that same process row.
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
- Remaining provider-engine close names and messages, including `close_engine`.
  `bind_group_to_driver`, `prepare_lifeline` and the exposed prepare/retain
  lifeline type are deleted. Keep `engine/` as Loopflow machinery and the
  surviving launch-path proofs.

## Current implementation boundary (2026-10-09 record cut)

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

The existing parent-side pre-exec handshake now writes the record for attached
Codex/OpenCode launches; attached Claude uses it too. Native foreground launches
still record after spawn. Optional attachment branches still permit unrecorded
launches; deleting inferred discovery does **not** prove those paths are covered.
This internal cut must not publish before they are converted.

### Counterexample that stops further launch work

Claude `interrupt` calls `kill_process`, then `ensure_process` can spawn another
OS process using the same cloned `AgentConfig.session_attachment`. One attachment
reservation is therefore not one OS process. Simply changing its PID would erase
history; independently claiming another record inside the harness would leave
`SessionCapture.driver` holding a stale token and break final settlement. The
record writer now refuses a different identity rather than overwrite it. This
leaves Claude interrupt/resume unfinished, not an accepted new refusal behavior.

Revised implementation direction: the existing invocation/capture owner must
explicitly replace its current attachment when a harness respawns, after exact
old-process death. The replacement commits a fresh AgentProcess and hands the
new snapshot to both native dispatch/history and capture settlement. Pending
operations keep their old snapshots; no shared mutable snapshot may silently
refresh stale operations. Uncaptured launches need the same Session/process
admission, not a second inventory or a fake lf parent. This is an implementation
choice requiring source work, not a new approval attributed to Jack Heart.

## Remaining implementation

1. Complete the launch-owner correction above. Cover every Codex/OpenCode/Claude,
   direct/native, helper and unbound path; one row per actual spawn, including
   Claude interrupt/respawn. Keep the attachment lock across admission/recording.
   Failed spawn is non-start evidence; uncertain spawn remains unknown. Native
   foreground terminals must retain their TTY/process-group behavior.
2. Remove numeric provider generation from runtime caller/status fences in favor
   of AgentProcess identity. Native request history and performed-work filters
   must name the agent record, not borrow current Session Work. Preserve origins;
   neither a delayed Started event nor a process's launch capture identifies a
   later request. Rewrite this same migration, never add another Task draft.
3. Complete stop/release authority across providers, both takeover death orders,
   and FIFO ownership. `HELD_LIFELINES` still retains superseded writers until lf
   exit. No current live attachment means orphan settlement, not invented exit;
   unknown attachment liveness stays unknown. Resume must consult the AgentProcess
   before treating a detached attachment as replaceable. Unfinished rows with
   unknown PID remain diagnosable. Reconcile zombie and unknown-OS readings across gate/top.
4. Replace remaining predecessor fixture assumptions and restore required
   active-Session, native-history and Task-membership coverage on records. Prove
   headless public top/Task-status/scheduled-entry agreement, not only reducers
   or SQL. The old directory-watcher costs no longer describe this reader.
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
counterexample above stops further launch work until the owner is corrected.

Earlier origin, token and pre-exec decisions remain in `a55f5345b` and its
references. The record cut is checkpointed at `446cfb2b5`, local and unpublished.

Check: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, network-isolated focused record/migration/reaper tests (12), repaired top projection (1), Waiting/handoff (1) and Rust DTO fixtures (22) pass. Full Rust/Swift/Linux/public-entry verification remains gate/CI-owned; Claude respawn and unconditional recording remain unimplemented.
