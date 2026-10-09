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

## Delete — do not maintain (completed and remaining cuts)

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
- `replaceable_driver`, `end_abandoned_agent_process` and `claim_provider_driver`
  are removed. Resume uses the shared record judgment and invocation-owned close
  beneath one attachment fence, including input admission. No detached shortcut,
  direct receipt sampler or mutable-Session provider selector remains there.
- Native resume's separate claim/interrupt/finish path in `resume_session_with_env`
  is removed. `session_command_status_with_env` owns fresh and resumed admission;
  saved captures and explicit connection attachments survive. Intentional client
  moves remain successful command exits, including their attachment outcome.
- Metadata-only `CaptureHandle::fail_and_begin_attempt` is removed; invocation
  retry settles/replaces the record before advancing capture metadata. The private
  metadata reducer remains for history/usage projection.
- Native `spawn_native(None)`, capture launch without an attachment, and
  `native_provider_driver`'s missing-provenance-as-client inference are removed.
  Raw headless optional admission remains a deletion target.
- Top’s receipt-selected inventory, `ProcessSnapshot`, and the exclusive
  `read_process_snapshot` transaction wrapper are removed. One unfinished-row
  query now serves Task membership and activity; receipts establish identity,
  not inventory membership.
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
unfinished AgentProcesses, including detached/replaced rows; live views retain
unknown identities without inferring liveness or control. Settlement rechecks
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

LOO-441/#1516 and #1512 are integrated. The latest owned sync, `e99aee7f7`,
adds main `e69d5103f` (#1511): native Codex commands pass `--no-daemon`, while
explicit account selection reconciles Codex's separate shared background daemon.
That daemon's five-minute turn grace period is not AgentProcess takeover or
orphan policy. Private headless app-server endpoints are unchanged. The upstream
native-argv fixture checks the new flag; configured terminal behavior with the
combined candidate remains unproved. No publication or installed-store migration
is claimed.
Earlier implementation/review detail and contrary evidence:
`e0d360e6a:scratch/introduce-agentprocess-record-the-provider.md`.

**Invocation-owned replacement (`cd3e8bafa`, 2026-10-09):** the library admission
counterexample is repaired without weakening the saved-thread check or changing
Codex's takeover-preserving `stop`. Retry now holds the capture's exact attachment,
closes or observes the old AgentProcess dead, and atomically retains its end,
reserves a fresh identity and selects the next native thread. Failed close or
stale authority leaves the old record unchanged. Only the capture owner advances
its snapshot; pending dispatch/history keep theirs. Account failover explicitly
clears the resume selection, including one inherited from the original launch;
same-account retry retains its thread. Prior account/native observations remain
history. Metadata-only retry is no longer a production capture API.

Invocation-owned settlement reads the recorded provider/interactivity, not next-launch
settings. Unknown or duplicated OS identity grants no signal authority in this path;
native foreground providers keep their launcher-owned teardown. The owned Codex close uses the
shared zombie/identity judgment. Generic harness teardown errors now propagate.
The regression uses different native threads for revoked and fallback accounts
and checks distinct ended AgentProcesses, one Session/parent and retained
thread/account pairs. Its socket fixture exits on disconnect; the separate
throwaway-group check exercises live close with an absent endpoint, not a live
Codex thread-inventory exchange. Configured-provider acceptance remains open.
Earlier failing proof and analysis: `3f530ffce:scratch/introduce-agentprocess-record-the-provider.md`.


**Inventory (`5fa855c6c`, 2026-10-09):** top and Task gates share unfinished-row
selection and identity judgment. Missing lf receipts, missing agent birth and
failed sampling remain visible Unknown rows with LFIDs. Active Sessions retain
records and report Unavailable on sampling failure. Rust/Swift activity DTOs and
fixtures move together. Source fixtures compare actual blockers to top rows;
the public ps fixture covers failed sampling. These results supersede the earlier
invisible-blocker implementation finding, not public Task-status/scheduled proof.

## Remaining implementation

1. Eliminate optional attachment paths in raw headless harness starts. `run_agent`
   now enters the existing invocation runtime before creating its implicit capture;
   nested calls reuse their parent. Native helpers also admit at entry, creating
   a capture only when absent. Native spawn requires an attachment; missing
   provenance no longer selects the remote-client path. Client spawn holds its
   attachment fence without recording provider exit. The composed Codex connection
   still needs repair: `connect_live_codex` passes an empty extra environment to
   native resume and a new relay path, whereas native admission requires caller
   provenance and compares that path to the saved upstream endpoint. The lower
   fixture bypasses the relay, so its pass does not prove this entry path. Carry
   the exact connection attachment and distinguish relay from provider endpoint;
   do not infer client authority from missing provenance. Library account failover now uses the
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
   unknown attachment liveness stays unknown. Resume now holds one attachment
   fence through shared LfProcess observation, recorded-provider close and atomic
   settlement/claim. Detached-live, unknown-spawn, duplicate live identity,
   zombie and changed-settings cases have focused source fixtures. Live close
   still supports only noninteractive Codex with a saved connection; native
   foreground and other-provider cleanup remain in this lifecycle cut.
   Source resume proofs do not establish configured-provider acceptance.
   No configured orphan was signaled; OS death supplies no successful outcome.
4. Complete public Task-status and scheduled-entry agreement, caller-lineage
   exclusion, native-history coverage and two-second removal. The inventory repair
   above supplies source evidence, not those entry-point proofs. Prune still stops
   before reaping on failed OS sampling; failure must remain visible, not success.
   Earlier visibility counterexamples: `48aaf72a1`, this plan.
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

Prior findings and fixes: `e0d360e6a`, this plan (migration/FK ordering,
mutable launch settings, SQLite across close, stale snapshots and duplicate
launches); `c1c09fb6c`, this plan (atomic resume, cleanup, fixture isolation and
checks). These remain source proofs, not configured or installed acceptance.

Compression removes native resume's duplicate admission/settlement path. The
common entry admits one lf invocation and AgentProcess around the saved capture;
explicit remote ownership is not replaced. Review retained intentional move
semantics: the public command accepts a recorded stop reason even with nonzero
provider status, so attachment settlement must do the same. Fixtures exercise
both outer-runtime and direct resume, saved native identity and one reservation
per launch. Top's activity fold uses array indices instead of cloned ID graphs;
unknown parent evidence, provider-only state and cycle refusal remain unchanged.

The composed relay mismatch under remaining item 1 is source counterevidence,
not a configured failure observation. It predates this compression and needs the
connection-owner cut, not weaker native admission checks. Release's operation-entry
lesson applies: helpers alone cannot prove top/Task-status/scheduled or installed
acceptance. No configured orphan was signaled and no installed store was opened.

Check: `cargo test -p loopflow --lib --no-run`, network-isolated lib filters `top::tests`, `preferred_name_resume`, `intentional_session_move`, `provider_sigterm`, `native_` (10 cases), `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` and `lf context --skill compress` pass; full Rust/Swift/materialized and public/configured acceptance remain gate/CI-owned.
