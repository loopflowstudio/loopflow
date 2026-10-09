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
  Jack Heart. The final AgentProcess retains its identity and original parent;
  its row will own the attached LfProcess and token. The current internal slice
  replaces the driver counter and `SessionDriver` end to end but still stores
  attachment on the Session. Provider generation remains until the record cut.
- OpenCode `start_inner` allocates a dedicated port/server per harness and creates
  or resumes exactly one native Session; stop removes that server. Codex creates
  a private socket per launch and reconnects to the same native thread. New
  AgentProcesses therefore serve one LfSession. Existing duplicate PID/birth
  observations must be retained as migration conflicts, never silently deduplicated
  into signal authority. No configured OpenCode data was queried.
- `ProviderProcess`, `OwnedProviderProcess`, `LiveProviderProcess` in `lf/commands/top.rs`
  currently reconstruct provider attribution from process trees/receipts and the
  OpenCode JSON registry. `SessionProcessObservation`/`SessionProcessOwnership`
  add another SQL projection of the Session's loose process columns. Replace
  this composition with records plus a read-local OS observation, not another
  ownership model. Unknown external processes confer no signal authority.
- `engine_orphans` reads Session columns separately and rechecks under the Session
  lock. Preserve exact PID/birth, command/group checks, takeover serialization,
  unknown-evidence refusal, and descendant cleanup in the surviving record path.
- The post-spawn bind is deleted. `spawn_agent_process` consumes the command,
  prepares the lifeline, then retains it only after successful spawn. The
  pre-exec child establishes its group and awaits watchdog readiness before exec.
  This preserves provider PID == PGID and native stdio/argv/spawn-error behavior.
  Only async-signal-safe operations may run after fork. Keep dash-compatible
  `kill -s TERM -- -pgid`. Native foreground terminals need their own process
  control treatment; no headless process-group change may break their TTY.

## Delete — do not maintain

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
  `bind_group_to_driver` and the exposed prepare/retain lifeline type are deleted.
  Keep `engine/` as Loopflow machinery and the surviving launch-path proofs.

## Current implementation boundary

`da98efe82` and `cad03fe6d` implement only the Codex/OpenCode pre-exec
lifeline. `spawn_agent_process` is an OS-launch function, not a record writer.
`0f1280b80` implements fresh attachment claims, including A → B → A;
`69f88f30d` simplifies attachment exits around their exact event identity.
`agent_process.sql` is the Task's single draft; it currently replaces attachment
counters and preserves exit-event references, input/native history and matching
Waiting evidence. It must be rewritten in place for the complete record cut,
never followed by another draft. No AgentProcess record exists yet; provider
columns, ownership projections, registry, gate and orphan reader remain.
The source still declares `Process` and `AgentSession`; LOO-441 integration
remains before publication. The integrated #1512 base is `3e1e6245c`.

The history boundary now uses `SessionTurnOrigin`: per-request immutable
attribution, captured before sending under the current attachment check.
Claude/OpenCode keep it with their input IDs; Codex keeps it with the request
sequence and accepts both reply/notification orders. Started insertion and
origin assignment share one transaction; retained historical attribution is not
rewritten. Duplicate correlated origins retain
exact values; conflicting repeats fail without rewriting history. Uncorrelated
broadcasts retain neither guessed Work nor the latest capture. No schema or wire
shape changed; this does not implement AgentProcess records.

Two boundaries remain distinct from the launch proof:

- `HELD_LIFELINES` retains every writer until its lf process exits. A superseded
  but living attachment can keep the provider alive after the current attachment
  dies. FIFO ownership is not current SQL attachment authority. The record-based
  orphan rule must cover this order, as well as preserving takeover when the
  original launcher dies first.
- The throwaway tests wait up to ten seconds; the watchdog waits two seconds
  between TERM and KILL. Neither establishes the requested two-second live-list
  removal. No top/Task-status or scheduled-check acceptance ran.

## Remaining implementation

1. Carry the implemented attachment token onto the AgentProcess row. Preserve
   AgentProcess identity and original parent across live takeover. The focused
   regression covers A → B → A dispatch, stop/release and Waiting writes.
   This fencing choice is implemented, not an outstanding design decision.
   Integrate LOO-441 before publication; no partial-slice publication.
2. Rewrite `agent_process.sql` in place to its final shape:
   backfill AgentProcesses before dropping Session columns. Preserve native
   history, original parent, current attachment, conflicts and unknown outcomes;
   migrate activity fences and caller provenance too. Test the released frontier
   and materialized migration, not intermediate drafts. OpenCode source supports
   one-to-one launches; configured cardinality remains unaudited.
3. Move claim, publication, native launch/resume/exit and stream writers to the
   record lifecycle, including Claude and native terminals. Retain pre-exec
   protection while eliminating the separate spawn-to-record uncertainty gap.
   Failed spawn is positive non-start evidence, not observed OS exit. Unknown
   spawn stays unknown. Historical duplicate PID/birth evidence stays recoverable
   without granting signal authority. Preserve history ingestion independently of
   the attachment claim: `record_session_event` deliberately accepts observations
   after transfer, and `record_session_turn_origin` retains correlated origin.
   The delayed-start repair now freezes initiating Work/capture before native
   requests; broadcasts remain unattributed until correlated. This observation
   snapshot is not attachment authority or another process owner. Preserve it
   when replacing provider generation with AgentProcess identity. Request
   correlation lost in a crash stays unknown; neither the latest Session
   capture nor AgentProcess's launch capture can recover it.
4. Move Task gates, active Sessions, top/monitor and scheduled orphan settlement
   to the same process inventory and delete the predecessors above. Prove both
   takeover death orders; no live current attachment means orphan settlement,
   not immediate invented exit. Reobserve exact death before removing live rows.
5. Move Rust DTOs, Swift mirrors and `tests/fixtures/dto/` together; update
   architecture and command docs. Preserve native foreground terminal behavior.
6. Gate: fmt/Clippy, Rust tests, Swift build/DTO fixtures, Linux lifeline tests and
   `rg -i 'provider_pid|driver_generation|SessionDriver|engine_orphans' rust/loopflow/src`
   empty outside applied migrations. Demo: top shows AgentProcess under its
   launcher; SIGKILL removes both live rows within two seconds, and Task status
   has no invisible blocker. The three pre-#1512 Codex orphans on Jack Heart's
   machine must be recorded and ended by the next scheduled check through the
   installed path. Branch fixtures never signal them or migrate that store.

## Preservation counterexamples

Live takeover must not kill the continuing AgentProcess when its original parent
exits. Same-PID/different-birth evidence never authorizes signaling. Stale writes
must fail even while the AgentProcess survives. Unknown or shared historical
ownership must not become a kill permission. Native history and unfinished
captures survive migration and failed admission. The watchdog must not inherit a
lifeline writer or hold provider stdout/stderr open.

## Implementation review

The October 9 capture audit found the design's counterexample in the old
reader: a first delayed Started observation selected the current capture/Work.
An AgentProcess launch snapshot would still be wrong after live takeover or
another turn on the same provider. Attribution therefore belongs to the native
request correlation, not the process lifecycle. History is retained without
current authority; dispatch and attention still require the current claim.
Codex fixtures exercise A → B → A before both start/reply orders and retain late
usage/completion on the original input. Claude and OpenCode delay their first
start until after input replacement. Binding has its own before/after request
fixture; repeat observations cannot reassign its earlier Work.


The attachment slice replaces arithmetic exit lookup with an exact event-sequence
reference, leaving old event keys and payloads unchanged. Claim clears the current
exit reference; finish saves the insert's returned primary key with revocation,
without reconstructing and querying the receipt key again. One history-query CTE
recognizes retained old receipts and new attachment exits for captured and native
turns. Plain observation lookup replaces `session_driver_exit`'s now-unused outcome
expression index. Attachment exit still does not invent provider completion.
Passive Program Status retains its provider-generation fence and cannot claim
attachment. Reconciliation corrected the architecture reference's obsolete ban
on AgentProcess and its removed driver-generation description. History retention
is explicitly distinct from current attachment mutation; no runtime change was
needed for that documentation repair. No Rust/Swift wire field changed; the
AgentProcess DTO cut remains open.

Review found that CLOEXEC alone leaves the launching child holding its own
lifeline until provider exec. The pre-exec hook now closes that inherited writer
before starting the watchdog; the before-exec SIGKILL test proves this boundary.
Closed caller stdio could also alias lifeline descriptors when Command installs
native stdio; descriptors now remain above 2, with a throwaway regression test.
The shared spawn operation now consumes the command, removing the caller-owned
prepare/spawn/retain protocol and preventing command reuse with stale descriptors.
Readiness descriptors use owned handles rather than repeated error-path closes.
These are launch proofs, not process-record or installed acceptance. Release's
child memory reinforces operation-entry verification: helper success cannot
establish top, Task status or scheduled orphan settlement.

Check: `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` passed; `uv run python scripts/test_network.py cargo test -p loopflow --lib -- harness::codex_history::tests harness::claude_history::tests harness::opencode_history::tests harness::conformance_tests store::sqlite::session_events::tests harness::claude::tests::sequential_managed_sessions_retain_their_exact_claude_engines harness::claude::tests::send_input_spawn_failure_releases_turn_guard harness::claude::tests::interrupt_without_turn_is_noop --test-threads=1` passed 32; `git diff --check` passed. Full Rust/Swift/DTO and Linux lifeline checks remain gate/CI-owned; installed acceptance remains open.
