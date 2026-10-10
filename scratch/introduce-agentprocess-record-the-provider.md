# AgentProcess (LOO-443)

Jack Heart requested this cutover on 2026-10-09. The Task directive is accepted;
the implementation choices below are a draft based on the source inventory at
`3e1e6245c`. Jack authorized parallel work with LOO-441, followed by integration
of its LfProcess/LfSession rename before publication. His steer `0aa2c34c`
then requested publication without merge, superseding the earlier no-partial-
publication note: #1519 is open for review on main `906576f39`, tracking this
branch, with the remaining cuts below, not as complete or landable. No installed-store
migration.

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
  its row now owns the attached LfProcess and token. AgentProcess identity
  replaced the numeric provider generation everywhere it fenced or attributed.
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
  saved captures survive. Exact connection admission is described below.
  Intentional client moves remain successful command exits, including their
  attachment outcome.
- The metadata-only retry entry is removed: `fail_and_begin_attempt` is now a
  private reducer with one production caller, which runs only after invocation
  retry has settled and replaced the record. In-module fixtures still call it
  directly for history/usage projection.
- Native `spawn_native(None)`, capture launch without an attachment, and
  `native_provider_driver`'s missing-provenance-as-client inference are removed.
  Native admission no longer reconstructs authority from `AgentCaller`; the
  command carries the capture or connection owner’s frozen attachment separately.
- Optional headless admission: `open_owner` returning `None`, `spawn`'s ownerless
  branch, Codex's unfenced writer and Claude's unfenced stdin/kill branches are
  removed. History structs keep an optional owner only for parser fixtures.
- Top’s receipt-selected inventory, `ProcessSnapshot`, and the exclusive
  `read_process_snapshot` transaction wrapper are removed. One unfinished-row
  query now serves Task membership and activity; receipts establish identity,
  not inventory membership.
- Process-group signal-0 existence checks are removed. The shared OS inventory
  records group membership and distinguishes live descendants from unreaped zombies;
  failed observation never proves group death.
- Top’s private OS sampler/elapsed parser, receipt-only birth comparison and the
  reaper's command/tree samplers are removed. `journal::OsProcess` supplies identity,
  parent/group, command and zombie evidence. Malformed samples fail observation;
  failed descendant inventory refuses before signaling rather than inventing absence.
- Codex close's separate signal/wait loop is removed. It retains native-thread and
  exact leader checks, then uses the shared group termination path; leader death
  alone cannot settle a group with surviving helpers.
- Capture's `driver` slot and `claim_conversation_driver` name are replaced by
  attachment naming; there is no second owner. The reaper's unused
  `settled_drivers` counter and serialization derives are removed. Its callers
  consume orphan IDs, reaped count and errors, not a separate wire report.
- Remaining provider-engine close names and messages (`close_engine` is removed).
  `close_agent_process` requires the saved endpoint/thread pair; its unused
  no-inspection branch is deleted. Missing socket handling still preserves the
  existing exact-identity/group close rule.
  `bind_group_to_driver`, `prepare_lifeline` and the exposed prepare/retain
  lifeline type are deleted. Keep `engine/` as Loopflow machinery and the
  surviving launch-path proofs. Codex's write-only `endpoint` and OpenCode's
  mirrored `child_group` are removed; Codex retains its pre-handle group slot.

- Store lifecycle writers named for the Session or the native launcher:
  `record_session_provider_launch`, `record_session_provider_process`,
  `record_native_provider_exit` and `session_provider_process` are now
  `record_agent_process_launch`/`_identity`/`_exit` and `agent_process_identity`;
  headless and native launch share one pre-exec recorder and one progress query.
  `SessionAttachment.provider_process_lfid` keeps its name: it is the
  AgentProcess's parent LfProcess, which `AgentCaller` carries as its origin.
- The numeric provider generation: the draft's `processes.provider_generation`,
  `session_activity.provider_generation`, `SessionAttachment`/`AgentCaller`/
  `SessionTurnOrigin`/`SessionSummary` fields, `LfProcess.caller_provider_generation`,
  the Session/SessionEvent/LfProcess DTO keys with their Swift mirrors and
  fixtures, and `observe-status --generation`. AgentProcess identity replaces each.

## Current implementation boundary (reconciled 2026-10-09)

`446cfb2b5` supplies one `processes` table (`lf` / `agent`), attachment tokens,
and shared record readers; `ea205e3d0` removes inferred discovery. The single
Task draft backfills parent, PID/birth, endpoint, attachment and available
spawn/exit evidence before dropping Session process columns. Duplicate PID/birth
rows stay separate and non-signallable. Native thread/history stay on the Session.

Top, active Sessions, Task membership and scheduled orphan settlement query
unfinished AgentProcesses, including detached/replaced rows; live views retain
unknown identities without inferring liveness or control. Settlement rechecks
attachment under the Session lock without holding SQLite across OS I/O. It still
excludes interactive agents; live-orphan termination recognizes Codex app-server,
OpenCode serve and, since 2026-10-09, headless Claude (program name only: it has
no subcommand, so exact PID/birth and group leadership carry the identity). Only Codex supplies a named FIFO for reconnect;
Claude/OpenCode anonymous lifelines do not establish takeover coverage.

Shared headless and owned native admission record argv and OS identity before
exec, under the attachment lock. Native admission preserves foreground groups,
TTY and stdio. Synchronous admission cannot lose an admitted child to cancellation
before returning it. Admission consumes each reservation once; a duplicate cannot
mark a running row spawn-failed. Headless and captured native admission both
require an attachment.

**Unconditional headless admission (2026-10-09, after sync with main `906576f39`,
#1499):** `run_agent` already claimed an attachment for every explicit or implicit
capture, so the only unattached starts were fixtures. `open_owner` now refuses a
missing attachment and `spawn` takes its owner by reference; no parent is invented
at spawn. Claude's stdin write and kill, and Codex's socket writer, lose their
unfenced branches. `AgentConfig.session_attachment` stays optional because the
config is prepared before admission; a start without it fails with "AgentProcess
requires an admitted invocation" before any provider code runs. Pre-exec recording
samples the child's birth through `ps`, so a launch PATH without `ps` fails
admission; fixtures that narrowed PATH now include the system directories.
The three ignored live-provider smokes admit a private Session; they were not run.
**First whole-library run (2026-10-09):** earlier passes ran focused filters only;
the full target exposed 30 failures. Product defects: #1499's `task_of_process`
read the dropped `agent_sessions.driver_process_lfid` (20 tests; SQL is not
compile-checked) and now reads the AgentProcess's attached LfProcess; the draft's
recreated revision triggers now match the released statement form. Fixture
repairs, no product change: fixtures selecting a store through `LF_HOME` pin the
invocation ledger with `with_test_ledger`, because the AgentProcess row needs its
parent LfProcess in the same database; a later turn after bind records its
request-time origin; session exit asserts shared identity evidence on a headless
Codex Session with a typed trace; the lifecycle integration query joins the
AgentProcess for the provider's parent; failed and exited starts are asserted on
AgentProcess records, not the deleted `provider:<n>` receipts. A third product
defect surfaced in the CLI suite: passive Program Status joined the AgentProcess
and recorded nothing for a native conversation Loopflow never attached to; a
missing AgentProcess is now a reading of its own in the writer and both readers.
Integration fixtures followed the deleted mechanisms: the active-Session watch
records an AgentProcess and its exit instead of a client receipt and the OpenCode
registry lock; the landing query selects `kind='lf'` children. A background run
with an open stdin made `lf task create` wait for a piped report; that was the
shell, not the product.
`migration_preserves_planning_identity_and_removes_snapshot_storage`
still fails on a missing `task_state_deliveries` table from main's `local_planning`
draft; this branch touches neither file and main's result was not reproduced here.

Fixture repair: `continuation_resumes_the_saved_thread_on_a_fresh_engine` recorded
the live test process as the previous provider and expected silent replacement,
contradicting the observed-exit rule; it now records a throwaway child and observes
its exit.

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

**Resume (`c1c09fb6c`, shared native entry `9255e9b03`, 2026-10-09):**
one attachment fence now spans LfProcess observation, recorded-provider close
and atomic settlement/claim, including input admission. Detached-live,
unknown-spawn, duplicate identity, zombie and changed-settings fixtures cover
this source boundary. The earlier feedback's request to consolidate resume is
implemented; the subsequent exact connection-admission slice is recorded below.

**Native connection (2026-10-09):** Native launch now carries the
frozen attachment from the capture or connection owner; the old provenance-to-
current-token lookup is deleted. Connection launch separates the client relay
from the recorded upstream endpoint, and validates the latter under the exact
attachment. A second exact-token check still fences actual spawn. Client exit
and interruption retain attachment outcomes without closing or ending the
surviving AgentProcess; orphan settlement remains independent. The previous
generic connection finish could close a headless Codex provider and is removed.
The composed fixture covers launch, takeover during history acquisition and
A → B → A before launch. It uses a stand-in native client and local history
server, not a configured Codex conversation or complete relay roundtrip.

**Scheduled entry (2026-10-09):** the public `task reconcile` fixture exposed
silent PID-observation failure and a non-parent reaper reporting its killed provider
as still alive. The operation now retains the shared OS reader's error with LFID
and returns failure without changing the record, attachment or history. Group
termination replaces `kill(-pgid, 0)` with the shared inventory's group/state
observation: zombies are dead even before their parent waits; sampling failure
remains failure and live descendants still prevent settlement. The CLI fixture
passes failed observation, recovery, detached settlement and activity removal using
a throwaway server-shaped shell. It proves neither configured providers nor an
installed scheduled firing. Review retained group-wide observation rather than
checking only the leader, which would lose surviving descendants. Compression found
that same leader-only mistake in Codex close and removed its private wait loop.
The throwaway close fixture has a TERM-resistant helper whose leader exits first;
shared termination must observe the entire group dead. The scheduled fixture also
allows exact PID reads while failing descendant inventory, retaining the live
provider and record without signaling. Reaper command/tree observations now come
from the shared OS reader rather than separate permissive `ps` parsers. These
checks do not establish configured-provider or foreground cleanup.

## Remaining implementation

1. **Admission types.** Headless admission is unconditional at runtime. The
   config field and history owners remain `Option`; passing the attachment to
   `Harness::start` would move the refusal into the type. `stop_native` keeps its
   ownerless branch for remote clients, which own no provider.
2. Done 2026-10-09: see "Generation cut" below. Installed migration and a
   provider conversation that outlives the upgrade remain unproved.
3. Complete stop/release authority across providers, both takeover death orders,
   and FIFO ownership. Cover Claude/OpenCode anonymous lifelines under takeover. The reaper now
   selects every noninteractive provider; foreground TTY providers stay outside
   headless group control. `HELD_LIFELINES` still retains superseded writers
   until lf exit. No current live attachment means orphan settlement, not invented exit;
   unknown attachment liveness stays unknown. Live close still supports only
   noninteractive Codex with a saved connection; native
   foreground and other-provider cleanup remain in this lifecycle cut.
   Source resume proofs do not establish configured-provider acceptance.
   No configured orphan was signaled; OS death supplies no successful outcome.
4. Complete caller-lineage exclusion, native-history coverage and two-second
   removal. Public agreement is covered (2026-10-09): through `task create`,
   `checkout`, `task status`, `monitor ps` and `task abandon`, a reserved
   AgentProcess with no OS identity holds the checkout, and every Process the
   hold names is a listed row. It uses a store-recorded row, not a launched provider. The scheduled-entry regression exercises failed
   observation, retained history, detached settlement and activity removal through
   `task reconcile`; it proves no installed firing or configured-provider behavior.
   Prune still stops before reaping on failed OS sampling. Earlier visibility
   counterexamples: `48aaf72a1`, this plan.
5. LOO-441's LfProcess/LfSession rename is integrated from #1516. Update the final
   model/API docs once the lifecycle cut settles. The common
   LfProcess projection still carries both kinds during this draft cutover.
   The generation wires and their docs moved with item 2.
   Provider-"engine" and driver-noun prose is replaced in the architecture
   reference, overview (and its rendered HTML), data, machines, execution,
   planning and conducting docs and AGENTS.md; the October 9 realignment found
   and replaced seventeen passages the earlier pass had left. The data owner
   table names AgentProcess rows as the attachment's owner. The lifeline
   paragraph states that only Codex takeover holds a named lifeline. Retained on
   purpose: the Flow driver (a different thing), the `engine.sock` socket
   filename, fixture locals named `engine`.
6. Gate owns affected Rust/Swift/DTO and materialized-migration verification plus
   Linux lifeline checks. Hosted CI on a pull request defers every build, lint,
   test, migration, Swift and architecture job while `scratch/` holds files
   (`scratch-clear` reports `candidate=false`), so #1519 has no hosted result and
   cannot have one before scratch is cleared for landing; the Task's "lifeline
   tests pass on Linux CI" is unobserved until then. That deferral hid a failing
   architecture drift check, repaired locally (below). Demo still requires exact SIGKILL removal within two
   seconds and no invisible Task blocker. Only the installed scheduled path may
   settle Jack Heart's three pre-#1512 orphaned Codex processes. Branch fixtures
   never signal them or migrate that store.

**Generation cut (2026-10-09):** AgentProcess identity is the fence and the
attribution. `AgentCaller` carries `agent_process_lfid`; a caller is current only
when that record is the Session's and still attached. Turn origin, Session
activity and Program Status name the record. A conversation Loopflow never
launched has none, and that absence is the Program Status witness: Desktop omits
`--agent-process`, and a later launch fences the passive stream like any
replacement. Session, SessionEvent and LfProcess DTOs, Swift mirrors and fixtures
moved together.

The draft backfills identity only for each Session's current generation: its
reading, its `started` events (through the released partial index, not a table
scan) and commands it issued. Earlier generations have no record and read as
unknown. Released `session_events.provider_generation` and
`processes.caller_provider_generation` stay as unread history: dropping the
first rewrites the largest table, and neither maps to a record.
`session_activity.provider_generation` is dropped. The frontier fixture asserts
current identity, unknown earlier generations and retained counters.

A provider launched before the upgrade holds an environment without the
identity. It parses as a caller that is never current: its commands keep their
origin parent, and it cannot rename or otherwise mutate its own Session until
relaunched. No source proof exercises a provider across an installed migration.
`tests/e2e/codex_connect.py` read the dropped Session columns since the record
cut; it now reads the AgentProcess. It needs live Codex and was not run.

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
remote connection admission now carries its exact claim separately. Review retained intentional move
semantics: the public command accepts a recorded stop reason even with nonzero
provider status, so attachment settlement must do the same. Fixtures exercise
both outer-runtime and direct resume, saved native identity and one reservation
per launch. Top's activity fold uses array indices instead of cloned ID graphs;
unknown parent evidence, provider-only state and cycle refusal remain unchanged.

Realignment at `726806476` identified the composed relay mismatch and token loss;
this slice removes that lookup and supplies the frozen capability end to end.
Review also found generic connection completion could close the provider after
client exit. Connection cleanup now records only its attachment outcome, including
failure rather than reporting all exits completed. Release's operation-entry lesson informs the composed fixture. Configured
acceptance, all remaining lifecycle cuts above and the full gate remain open.
Native fixtures now pin invocation and capture to the same ledger, including
blocking workers, and supply required startup evidence. The planning
reconnect fixture retains its original behavior and reports early launch errors.
Review of the surviving close/reaper paths removed an unused optional inspection
bypass and report bookkeeping. Reaper control flow now separates process
settlement from attachment settlement: exact provider death does not prove an
unknown attached lf invocation dead. Capture admission and storage use the
attachment name throughout; historical payloads and the Flow driver remain
unchanged. No installed store was opened and no configured orphan was signaled.

**Sync with main `f4356029d` (#1517, 2026-10-09):** AgentSessionId now
flows through native admission, invocation retry, connection inspection and
correlated history. Attachment tokens, request-time attribution and the deleted
engine-orphan path remain intact; no generation cut or installed-store migration
is implied. Review retained the AgentProcess endpoint owner separately from the
LfSession's selected AgentSession. Branch-only fixtures use the typed identity.


Earlier native-util/planning-reconnect, 16-test isolated lifecycle and typed-identity
sync checks are retained at `e87e9d643`, this plan.

Compress check (2026-10-09, after the generation cut): the store/LfSession/attachment triple is named `AttachmentOwner`; the reaper shares one attached-invocation death judgment. No behavior change. `cargo clippy -p loopflow --all-targets -- -D warnings` passes; `cargo test -p loopflow --lib` for `harness::agent_process`, `session_record::runtime` and the Claude/OpenCode history modules: 15 passed. Wider suites stay with gate.

**Architecture drift (2026-10-09):** `scripts/check_architecture.py` passes on
main `906576f39` and failed on this branch with three findings no earlier pass
ran it to see. The AgentProcess map row repeated `processes` and `lf monitor`,
which the LfProcess row already owns; it now names its rows within that table
and `lf top`. `session_record/runtime.rs` gated its tests with
`#[cfg(all(test, unix))]`, which the check does not recognise as test code, so
its throwaway `/bin/sleep` and `fixture` commands read as production subprocess
edges; the module uses `#[cfg(test)]` like `harness/agent_process.rs`.

Realign check (2026-10-09): `uv run python scripts/check_architecture.py` reports zero drift after repair (three findings before); `render_architecture_html.py --check` and `cargo fmt --all --check` pass; the Task's `rg` returns nothing outside applied migrations and this Task's draft. No suite rerun; gate owns it.

Check (orphan rule and public agreement, 2026-10-09): `cargo test -p loopflow --lib harness::agent_process` 7 passed; `--test process_ownership_tests task_checkout_blockers` 1 passed; fmt and clippy `--all-targets -D warnings` pass. Wider suites stay with gate.

Check (generation cut, 2026-10-09): `cargo test -p loopflow --no-fail-fast --lib` plus the ten affected integration targets, `LF_*` cleared and stdin closed: library 1766 passed, the same 4 failed (three pass alone; one is main's planning-migration fixture), integration targets pass; `swift build --build-tests` and `DTOFixtureTests` pass; `cargo fmt --all --check` and `cargo clippy -p loopflow --all-targets -- -D warnings` pass. Remaining integration targets, draft materialization, Linux lifeline and live-provider smokes stay gate/CI-owned. Earlier results: `4d720d314`, this plan.
