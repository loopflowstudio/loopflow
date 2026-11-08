# LOO-810: server recovery, mutation and provider-transition contracts

Stacked on LOO-593 (#1813, revision 2). Five rows of the 2026-11-07 report card
grade B. This plan closes each gap where the code lives, keeps the three
recovery contracts separate (Work readings, terminal, provider conversation)
and adds no event log, generic execution owner or byte-path role for `lf`.

## Direction · 2026-11-08

Jack Heart: "ok, lets do C with simpler records"

Jack Heart: "lets pursue"

Build C: submitted creation requests survive Desktop restart. Use one immutable
client record containing request ID, original destination, exact versioned
payload and submission time. Persist before first send; the server owns all
milestones and outcomes. Reconcile on reconnect and startup, then remove the
record after presenting the result. Unknown effects remain inspectable and do
not authorize another launch. No editable outbox or generic mutation queue.

Keep the complete Work protocol cutover, one creation result shape for CLI and
server, and the short provider-transition explanation plus history. The latter
two are the proposed implementation defaults carried forward with this design.
Implementation proceeds through the existing feature Graph's pursue step to demo.

[The exploration](explore.html) retains the alternatives as background. Its
interactions simulate failures and create no Work.

### Source comparison · 2026-11-08

Jack Heart: "how bad are the recovery records? how does etiher approach compare to t3code, paseo, orc, or paradev's approaches"

Source inspection led to C limited to immutable submitted creation requests.
Most difficult
server work (fingerprint, resource IDs, concurrency and interrupted-effect
handling) is already required by B. C adds remembering which requests to recover.
The smallest client record is request ID, original destination, exact versioned
payload and submission time, saved before the first send. The server owns the
outcome; a second client-side milestone state machine is unnecessary. Resolve and
remove the local record after showing its result; a failed removal can repeat
lookup safely. Keep unknown launches inspectable. Editing queued payloads,
offline composition/delivery, attachment queues and arbitrary mutation replay are
excluded. Receipt expiry must never turn an old request into a new creation.

Inspected November 7 clones plus November 8 reads of the corresponding upstream
source files; no competitor app or crash tests were run:

- **T3:** server `EventSink.commitCommand` commits command receipts, events,
  projections and effect-outbox entries together. Mobile also persists outgoing
  messages, including thread creation; it confirms persistence before dispatch,
  reloads at startup and reconciles delivered creations. This is C-like and
  broader: edit races, attachments and queued delivery add much of its complexity.
  [Server](https://github.com/pingdotgg/t3code/blob/71c2d3a302092b94511eb7e6ab4e6844a66dbf22/apps/server/src/orchestration-v2/EventSink.ts),
  [mobile storage](https://github.com/pingdotgg/t3code/blob/71c2d3a302092b94511eb7e6ab4e6844a66dbf22/apps/mobile/src/state/thread-outbox-storage.ts),
  [delivery](https://github.com/pingdotgg/t3code/blob/71c2d3a302092b94511eb7e6ab4e6844a66dbf22/apps/mobile/src/state/threadOutboxDelivery.ts).
- **Paseo:** creation persists fingerprint, resource IDs and milestones; an
  interrupted in-flight effect becomes outcomeUnknown. Its CreationClient keeps
  operations in memory and reconnects by observing a receipt, submitting again
  only when no receipt is found. The app create-flow store is also in memory in
  these paths. Closest to B, with useful receipt observation already included;
  no C-style cold-app recovery is established by those paths.
  [Server](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/creation/index.ts),
  [client](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/client/src/creation/index.ts).
- **Orca:** operation-specific approaches. Orchestration has SQLite mutation
  receipts with caller/request identity, method/payload checks and pending/completed
  state. Worktree creation instead reuses an in-memory promise/result by mutation
  ID, retaining success for 60 seconds. Terminal creation separately reconciles a
  stable handle against the host's process inventory. These establish neither
  uniform durable retry nor a general C-style client outbox.
  [Receipts](https://github.com/stablyai/orca/blob/e799cb419e89e1aa52f49feec0e273f8f613d737/src/main/runtime/orchestration/db/mutation-receipts/mutation-receipt-store.ts),
  [creation](https://github.com/stablyai/orca/blob/e799cb419e89e1aa52f49feec0e273f8f613d737/src/main/runtime/terminal/launch/terminal-create-deduplication.ts).
- **Paradev:** chat command dedupe caches 500 results in memory. Mobile retries
  queries only; its source explains that retrying workspaces.create can duplicate
  a workspace. B and C would offer stronger creation retry than these paths.
  [Dedupe](https://github.com/ParadevAI/Paradev/blob/61645f4852d3f7ecca7f58a6f6d68d7bc9dc460c/packages/chat-runtime/src/commands/commandDedupe/commandDedupe.ts),
  [retry policy](https://github.com/ParadevAI/Paradev/blob/61645f4852d3f7ecca7f58a6f6d68d7bc9dc460c/apps/mobile/lib/errors/transport-retry.ts).

### Implementation constraints

- Count producer reservations before dispatch. Kickoff's serialize-then-reserve
  queue budget alone does not meet the brief's retained-work requirement. Bound
  concurrent materialization as well as queued bytes, including across clients;
  distinguish this accounting from a measured process-RSS cap. Limits remain
  provisional. Reads may be refused before running; admitted writes cannot be
  reported as unrun when delivery fills the budget.
- A retry must correlate the exact creation request to its launch attempt;
  finding an arbitrary current Task Graph cannot prove this request started it.
  Receipt advancement and resource/input creation must be atomic where local;
  interrupted external work remains unknown. Cover concurrent duplicate requests,
  restart, and the Session stop/append crash windows, not just a dropped socket.
- Retain server receipts in this version. Do not introduce pruning and an expiry
  protocol for small creation records. A failed read is not absence, and a receipt
  for a removed resource never authorizes recreating it. Future retention must
  keep old request IDs from becoming new creations.
- Record observed provider transitions, not a success claim at process
  reservation. Provider IDs can arrive later. Keep unavailable lineage unknown.
  An attach result can report retained without overwriting the last actual
  provider change. `original_running` must be dated or refreshed, not presented
  as current forever. Keep `started`: account failover clears the resume token
  and carries the original task prompt, not the previous conversation history.
- `busy` is operation-specific: the current checkpoint overflow ends that
  subscription and requires resubscribe; it does not require reconnecting an
  otherwise healthy connection. Document recovery by operation and error scope.

## Demo

Desktop attached to a running Task; the Mac sleeps; it wakes. The window shows
**Reconnecting…**, then the same Task, the same terminal screen (same owner,
no input replayed) and the conversation's continuation line reading "Same
process". A New Task sheet submitted as the connection dropped files exactly
one Task. Quit Desktop before the answer returns, reopen it, and recover that
same Task from the submitted request and its receipt. An unconfirmed launch
shows "Task created; start unconfirmed" and starts nothing again. Headless
analog at gate: the acceptance checks below.

## Implementation contract

### 1. Retained-work budget (structured-state recovery)

Per connection, in `service.rs`:

- **Open limit:** 256 open requests plus subscriptions. The slot is taken at
  admission, before dispatch; over the limit answers `busy {id, data:
  {limit: "open", max: 256}}` and nothing ran. Writes have a second limit of
  16 open writes, since a finished write can only wait (below).
- **Byte budget:** an `Outbox` replaces the `mpsc` queue. Reserve producer
  headroom before dispatch or subscription materialization, conservatively up to
  `MAX_MESSAGE` when the operation has no smaller bound. Serialize in the worker,
  shrink the reservation to actual bytes and carry it through queue and socket
  delivery. Budget starts at 64 MiB per connection and 256 MiB across the process;
  both include producer reservations, not only enqueued messages. Bound temporary
  value construction as well; serialized accounting does not measure RSS.
  Errors and subscription acknowledgments need bounded control headroom, with
  a send deadline when the peer drains nothing.
- **When a reservation fails:**
  - A read is refused before dispatch with `busy {id, data: {limit: "retained_bytes"}}`.
    Reads have no effect, so "retry shortly" is honest.
  - A write is admitted only after its producer reservation. After admission its
    effect finishes; backpressure never turns it into an "unrun" refusal.
  - A subscription waits for producer room before pulling. It retains its
    checkpoints and cannot pull another reading until delivery acknowledges the
    first. Notifications coalesce while waiting. Order stays: null response,
    readings, the final reading before `ended`, and error frames last.
- `welcome.limits` gains `max_open` and `max_retained_bytes`;
  `lf server status` clients gain `open` and `retained_bytes`. The 5 s send
  timeout stays: a client that takes nothing still loses its connection.
- Deltas stay out. If mobile measurement justifies them later, model a
  latest-value-per-entity collection with a monotonic sequence, bounded
  tombstones and a snapshot when a cursor expires. Written in the README as the
  only sanctioned direction.

### 2. Terminal recovery

Behavior is kept: exact-owner continuation, 1 MiB spool with 60 s grace,
fresh-screen fallback, no input replay, owner-held geometry. Work is tests plus
one fix, using the existing `transport_tests.rs` fixtures (proxy, observer,
`Fixture::serve(true, …)`):

| Case | Existing | Add |
| --- | --- | --- |
| (a) phone asleep while Desktop resizes and types | `server_reconnect_preserves_newer_viewers_input_and_geometry` :5988 | newer viewer resizes during the outage; read the PTY size back after recovery; check the terminal-mode screen shows the newer output |
| (b) return after retention expired | `a_resume_past_the_spool_is_refused…` :4715, `server_reconnect_refreshes_expired_output…` :6196 | expiry by the production 60 s grace, no `Out` bytes before the refusal, snapshot content, terminal mode |
| (c) old owner ends, Session gets a new owner | none | exact-owner attach answers `not_found {owner_lf_process_id, current_lf_process_id}`; attach by Session reaches the new owner; no provider launched by either |
| (d) one viewer stops draining | `slow_observer_does_not_block_output…` :1354 | through the server bridge: one stalled bridge, one healthy bridge receiving, and the same client connection answering a read and a subscription checkpoint meanwhile |
| (e) old socket finishes after a replacement | `server_move_lost_ack` :6051 | replacement that asked for free claims (no takeover); old controller's socket closes after; replacement keeps input and geometry |

Fix: a stream-mode resume carrying the viewer's own `reconnect` identity while
that viewer's previous connection is still open at the owner supersedes the
stale connection instead of being refused. It takes only claims that
connection held, never another viewer's. Covered by a variant of (e).

### 3. Durable creation receipts (mutation intent)

`task.create` and `session.new` get a retry contract; nothing else changes.
Every other write keeps "admitted mutations finish; a lost answer is
`outcome_unknown`".

- **Wire:** `request_id` (required, 1–128 bytes, client-chosen; Desktop uses a
  UUID per sheet submission) on both. `Write::TaskFromSession` becomes
  `Write::TaskCreate {directory, request_id, title, request, wave, graph,
  session, run}`, the full `lf task create` shape, so Desktop's New Task and
  "make this conversation a Task" are one served kind. `wave: null` is the
  repository, as Desktop's `--no-wave` today. The CLI mints a fresh
  `request_id` unless `--request-id` is given, so agents and scripts can retry
  safely too.
- **Result:** `TaskCreateReceipt {task, graph, session_id, milestone}` for
  both `run` values; the CLI prints the same. `milestone` is `filed`,
  `started` or `admitted`.
- **Store:** one migration, `creation_receipts (request_id PRIMARY KEY, kind,
  fingerprint, task_id, session_id, artifact_key, milestone, result, launch, created_at,
  updated_at)`. `fingerprint` is SHA-256 of the canonical operation JSON
  without `request_id`. The row is written, with the Task ID or the new
  thread's artifact key reserved, in the same transaction as the first
  effect, before any provisioning. Retain these small receipts without age-based
  pruning in this version; request identity must not become reusable.
- **Retry:** same `request_id`, same fingerprint: continue from the recorded
  milestone and reuse every recorded ID (`create_task_in` already accepts the
  identical `NewTask`; `primary::ensure_task` is find-or-create). Different
  fingerprint: `conflict {request_id}`, nothing runs.
- **Interrupted external effect:** `run` records `run_requested` before
  starting `task.run`'s detached command and `started {graph_process}` when it
  registers. A retry that finds `run_requested` reconciles only the launch
  correlated to that exact creation request. An unrelated current Graph cannot
  establish success. Missing confirmation answers
  `outcome_unknown {task, milestone: "run_requested"}`, and the client inspects
  the Task. Never a second launch from a receipt that cannot say what happened.
- **`session.new`:** records `stopping` before `stop_session_agent`, then the
  new artifact key with `admitted`. A retry at `admitted` returns the recorded
  `SessionRecord` without stopping anything. Reserve the artifact identity and
  record input advancement atomically with the receipt. At an interrupted stop
  or append, reconcile that exact attempt; never stop a newer process or append
  another input because a receipt is incomplete.
- **Receipt lookup:** a read-only shared protocol operation returns the recorded
  creation outcome or confirmed absence for a request identity. Unavailable reads
  remain unavailable. It never starts work and cannot recreate a removed Task.
- **Desktop:** `createTask`, `createTaskFromSession` and `session.new` use the
  same submission recovery mechanism. Save `{request_id, destination, payload,
  submitted_at}` before first send; version the record/payload for decode. No
  local receipt copy, milestone ledger or execution state. Reuse an existing
  app storage owner where it fits, scoped to the original Machine/repository.
  On reconnect or cold startup, look up the receipt. Present known outcomes;
  resubmit the identical request only on confirmed absence or a server-declared
  safe continuation. Retain unknown outcomes for inspection; never guess that an
  interrupted external effect is safe to repeat. Remove the local record only
  after reconciliation is presented; failed removal repeats lookup safely.
  Multiple windows share one recovery owner. Storage failure keeps the form and
  reports the error before send. Delete `TaskCreateRunFailed` stdout parsing.
- The shared functions own receipts (`ops/task.rs` `task_create`,
  `human_session::new_thread`), so the CLI and server share one path;
  `service.rs` only passes the request through.

### 4. One client protocol

- `server/README.md` gains **Client contract**: what a client may cache
  (welcome's `instance` and limits; last readings as stale display only), what
  it re-reads on reconnect (every subscription, from a complete reading; every
  attach by exact owner), and recovery by error scope. Lost connections reconnect;
  per-request `busy` delays that operation; checkpoint overflow resubscribes that
  subscription. `incompatible`, `invalid`, `not_found` and `conflict` require the
  corresponding correction, not blind reconnect. `outcome_unknown` means inspect
  a receipt when available, otherwise inspect the resource; never generic retry.
- Desktop stops shelling out for Work: `task.create`, `task.run`, `task.ship`
  move to served kinds this Task. `task.move --machine`, wave machine set,
  `wave.remove`, `wave.rename` and process history also use served kinds. Done means `grep "await run(" swift/Loopflow/Services/RegistryQuery.swift`
  finds no Work mutation. The same-user socket stays local; remote admission
  is out of scope.

### 5. Provider transitions

- **Type:** `ProviderTransition {kind, from_provider_session,
  to_provider_session, from_agent_process, to_agent_process, original_running}`
  with `kind` one of:
  - `retained`: the same AgentProcess keeps serving (attach to a live owner).
  - `resumed`: a replacement AgentProcess on the same provider session ID
    (`lf resume` after exit, account move, Claude respawn, rollover).
  - `forked`: a new provider conversation carrying the old one's history,
    `seed: native | log` (`lf resume --fork`, `session.new`, Codex `/fork`, a
    provider change by `-a`). `original_running` says whether the source still
    runs beside it: the Paradev trap made visible.
  - `started`: a provider conversation without the old history. With
    `from_provider_session` set, the earlier conversation was left behind
    (account failover, Codex `/new`).
- **One writer:** recorded as a `provider_transition` observed event once the
  serving process and provider conversation are known, including in-process
  switches. Reserving a process does not prove a transition succeeded. Unknown
  lineage stays unknown; attach can report retained without replacing the last
  actual provider-change event. It replaces `agent_session_selected` (steers.rs
  and its reader in `codex_connection.rs:1112`). Shipped rows keep their old
  payload; readers accept both until those rows age out of history display.
- **Readers:** `SessionRecord` gains `transition: Option<ProviderTransition>`
  (the latest; null before any launch). `session.history` shows each.
  `lf resume --fork`'s new Session records `forked {from_provider_session,
  original_running}`, linking it to its source.
- **Desktop:** the pane's connection row shows the latest transition after any
  change: "Same process", "Resumed in a new process", "Forked from <title>,
  original was running at fork", "New conversation; earlier one left behind".
  Current source liveness requires a fresh reading, not the historical boolean.
  Reading a transcript grants nothing: a read-only pane still needs Move here.

## Forbidden

A universal event log; replaying raw input; retrying any write without a
receipt; a generic execution owner; receipts in memory only; a second served
kind for New Task beside `task.create`; Desktop keeping its CLI path beside the
served one; `lf` in the provider byte path.

## Delete — do not maintain

The unbounded message queue, adoption-only `Write::TaskFromSession`, Desktop
`TaskCreateRunFailed` parsing and Work subprocess paths, provider-selection
writer, and closed-socket resume precondition are removed. The remaining
`agent_session_selected` reader translates released history only; runtime tests
query `provider_transition`. No deletion targets remain.

Compression keeps request slots with their producer reservation through worker
completion and delivery; task completion no longer returns an unused request ID.
Task creation decodes one recorded receipt, and provider lineage uses its typed
record for both writing and reading. Desktop claims recovery before any await,
so destination lookup cannot admit two recoveries for one submission.

The original baseline analysis and implementation slices are at
`758309facbef6368627cc9cd2a426de9230e5fe0:scratch/loo-810-server-contracts.md`.
The contract above and acceptance checks below remain current.

## Checks

### Shipping docs · write-docs step

The user contracts are in `docs/{server,work,sessions}.md`; the protocol
reference is `rust/loopflow/src/server/README.md`. Keep those aligned with
`lf server describe` and its fixtures.

Write-docs checks, 2026-11-08:

- `git diff --check` — passed; implementation/gate checks follow below.
- `lf open --host desktop --file docs/server.md --file docs/work.md --file docs/sessions.md --file rust/loopflow/src/server/README.md --file docs/dev/server.md` — failed: "task_7dede9dbe66d41ed98569d943ed63ccf has no workspace in this Desktop window; nothing changed." Docs remain in this checkout; demo can present them in its Task workspace.
- Runtime tests skipped for this prose-only step; the implementation and gate run the checks below.

### Implementation checks · 2026-11-08

All commands run with inherited `LF_*` unset and stdin closed. The candidate CLI
is recorded in `loo-810-sync.md`; fixture runs pin `LOOPFLOW_TEST_LF` to it.

- `cargo test -p loopflow --lib ops::human_session::tests::creation_retry_reconciles_publication`: passed; an interrupted stop stays unknown, an admitted input publishes once, a later input is untouched.
- `cargo test -p loopflow --lib ops::human_session::tests::new_thread_before_a_first_provider`: passed; original prompt and retry identity survive.
- `cargo test -p loopflow --lib store::sqlite::provider_transitions::`: 2 passed.
- `cargo test -p loopflow --lib server::outbox::`: 2 passed with a 16 MiB answer, shared process pressure and a worker surviving cancellation.
- `cargo test -p loopflow --test served_reads served_task_create_survives_a_dropped_connection_and_server_restart`: passed, including concurrent duplicates, conflicting content and an unrelated launch.
- `cargo test -p loopflow --lib task_graph_launch_tests::task_create_run_starts_the_graph_and_admits_the_request_as_first_turn`: passed after retaining the writable creation store and waiting for the registered Graph in the fixture.
- `cargo test -p loopflow --test transport_tests resume_supersedes_its_open_socket_without_taking_a_newer_viewers_claims`: passed.
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed after sync.
- `cargo test -p loopflow --test served_reads served_session_new_recovers_its_input_after_a_lost_reply_and_restart`: passed; exactly one appended input across the lost reply and server restart.
- `cargo test -p loopflow --lib server::service::tests::flooded_requests`: passed after retaining construction/pull capacity through cancellation; no runtime shutdown panic.
- `scripts/test_desktop.sh --no-parallel --filter 'CreationSubmissionTests|ServerProtocolTests|ProviderTransitionTests|NewTaskTests|SessionControlsTests|WorkDestinationTests/newTask|RegistryQueryTests/contentRoutedStepHistory'`: 32 passed headlessly; the additional `WorkDestinationTests/submittedCreationHasNoSecondSubmitButton` check passed from that build.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`: passed.

### Compression checks · 2026-11-08

Same environment and candidate CLI as above.

- `cargo test -p loopflow --lib server::`: 28 passed, including request/write permit ownership after cancellation.
- `cargo test -p loopflow --lib store::sqlite::provider_transitions::`: 2 passed with typed lineage.
- `cargo test -p loopflow --lib task_graph_launch_tests::task_create_run_starts_the_graph_and_admits_the_request_as_first_turn`: passed with one receipt decode.
- `cargo test -p loopflow --test served_reads served_task_create_survives_a_dropped_connection_and_server_restart`: passed.
- `cargo test -p loopflow --test integration session_steers::public_context_capability_survives_claim_changes_and_rejects_children_and_replacement`: passed using the new transition event; corrected an initial invocation naming a nonexistent standalone target.
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed.
- `scripts/test_desktop.sh --no-parallel --filter 'CreationSubmissionTests|RegistryQueryTests|ServerProtocolTests|ProviderTransitionTests'`: 40 passed, including concurrent recovery during destination lookup.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`: passed.
- `uv run python scripts/test_network.py uv run python tests/e2e/codex_terminal.py --lf target/debug/lf --codex /Users/jack/.local/bin/codex --new-thread`: plain and wrapped native PTY cases passed; `/new` and resume selected the new conversation, with no live fixture providers remaining. First run saw one unsettled observer after renderer exit; both processes later completed. The check now waits up to five seconds for teardown. Passing artifacts: `/private/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-codex-terminal-4xv_2wcn`.

Gate owns affected suites and the complete terminal acceptance set:
`server_reconnect_preserves_newer_viewers_input_and_geometry`,
`grace_expiry_refuses_bytes_before_repainting_the_terminal_screen`,
`exact_owner_refuses_a_replacement_while_session_attach_finds_it`,
`a_stalled_bridge_leaves_the_healthy_bridge_and_control_checkpoint_live`, and
the stale-socket case above. It also owns merged-frontier migration checks and
CLI/server parity across the remaining Work operations. The grace test uses
61 seconds of production time. Mac sleep and quit/reopen remain demo/review.

## Done when

The README states each row's contract beside its code; the flood test shows
bounded retained bytes and honest `busy` refusals; a dropped New Task resend
files one Task, including after Desktop restarts; saved client requests contain
no duplicated outcome state; unknown launches stay inspectable; the five terminal
cases pass with claim and geometry assertions; every provider-changing operation records retained, resumed,
forked or started, and Desktop's connection row shows it.

## Exclusions

Remote admission and pairing; delta subscriptions; structured chat; receipts
for writes other than `task.create` and `session.new`; LOO-796's reconnect UX
(reading, copying, drafts, focus); editable pending submissions, offline creation,
attachment queues, cross-device submission sync and arbitrary mutation replay.
