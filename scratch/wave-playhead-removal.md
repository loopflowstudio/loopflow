# Remove the Wave Flow interpreter; preserve Wave operation

Task-ready follow-up directive, 2026-09-25. Source inspection only; no runtime
deletion, migration, live Session, or deployment is approved or performed by
this note. Human direction comes from `scratch/concept-review.md` and
`scratch/loopflow.md`: Tasks are the current execution UI focus; ordinary Flows
may run about Waves. Do not extend the old Wave interpreter for loopflow parity
or preserve it for hypothetical future use.

## Outcome and boundary

Wave operators keep cadence, chat, current work visibility, safe interruption,
and actionable failure/recovery while each governance wake runs the existing
bounded `wave/operate` behavior. Remove the Wave root/queue/return-stack Flow
sequencer. Keep recurrence in the existing scheduler. Do not introduce a new
Wave Flow lifecycle, scheduler, or parallel continuation store.

This is separate from LOO-295 because it crosses the resident/listener wire,
historical Wave journals, and Swift failure presentation. Task XOR, ordinary
Flow pinning/routing, and consolidation of Task/ordinary traversal remain
LOO-295 integration work; their acceptance must not depend on extending this
obsolete interpreter. This follow-up owns the shared-type move required by its
deletion, not a rewrite of Task or ordinary Flow authority.

Paths below are relative to `rust/loopflow/src/` unless explicitly prefixed.

| Remove or change | Exact boundary and surviving responsibility |
| --- | --- |
| `controller/wave/playhead.rs` | Delete executable `Playhead` stack settlement, root wrap, `enqueue`, `InvocationState` queue/return bookkeeping and navigation projections after migration. Keep only necessary historical decoding under a historical owner. Move shared Flow types before deleting their current home. |
| `controller/wave/runtime.rs` | Remove `ensure_playhead`, `restart_legacy_playhead`, `enqueue_flow`, `journal_playhead_locked`, cursor-bearing runtime state and playhead broadcasts. Rework `start_body`, `finish_body`, `finish_body_locked`, skip, and resident finalization around actual attempts and outcomes. Preserve journal ownership, pending message consumption/requeue, provider session attribution and failure evidence. |
| `controller/wave/runner.rs` | Replace `run_pass`'s fetch/advance loop and selected-step dispatch with one bounded governance turn per scheduling decision. Remove `spawn_wave_step` and Flow composite fallback. Keep harness preparation, owned live client lifecycle, streaming, interrupt/timeout teardown, and supervisor failure reporting. |
| `lf/commands/flow.rs`, `lf/mod.rs`, `bin/lf.rs`, `controller/wave/mod.rs` | Remove hidden `__flow-step` / `Commands::FlowStep` / `run_step` and Wave `--restart-flow` plumbing once old-state disposition is available. Keep ordinary named/explicit/bound Flow execution and its resume/decision commands. |
| `controller/wave/server.rs`, `wire.rs`, `runtime.rs` | Remove `/playhead`, the `playhead` SSE event and subscription fields, and `ContextResponse.playhead`. Keep resident attach/context and ordered turn/body evidence required for live operation. Recast attempt fields only where necessary; do not delete the resident protocol wholesale. |
| `controller/wave/journal.rs` | Stop writing new `PlayheadChanged` execution snapshots after the cutover. Retain readable old events and their body/session linkage; add an explicit durable disposition for unfinished legacy continuations. Keep turns, messages, consumption facts, observations and failure history. |
| `swift/Loopflow/Models/ChatTurn.swift`, `Services/WaveChatClient.swift` | Remove executable playhead/queue projections and SSE handling after consumers move. Preserve historical turn bodies and provider attribution. Coordinate DTO changes and fixtures across producer and consumers. |
| `swift/Loopflow/Models/AttemptFailurePresentation.swift`, `swift/LoopflowMac/Views/WaveChatView.swift` | Preserve visible failure, superseded-attempt and retry information using surviving attempt/Run evidence before removing the playhead parameter. Do not make a failed attempt disappear merely because the next/return UI disappears. |
| `engine/builtins/wave/flow/wave.yaml` | Stop using this one-step definition as resident control state. It currently contains only `wave/operate`; retain it as an ordinary finite Flow only if a real caller needs that catalog entry. Audit callers before deletion. |

Source search found `WaveRuntime::enqueue_flow` only at its definition. That
supports deleting the entry point, not discarding journaled queues. `run_pass`
still reads `context.playhead`, and `server.rs` serves it: the interpreter is
reachable. `run_step` reloads mutable Flow source and executes a singleton; it
cannot remain as a hidden second interpreter after the resident cutover.

## Shared types and ownership

Move `QueuedInvocation`, `StepRef`, `StepKind`, their step-ref construction and
required stored-step decoding into the existing engine/Flow domain. Rename
`QueuedInvocation` to describe a captured invocation if appropriate; it contains
`id`, `flow`, and `steps`, not a queue. Move one implementation, preserving its
serialized shape and identity; do not copy it or re-expand saved definitions.

Known consumers include `durable.rs::FlowPosition`,
`controller/task/mod.rs`, `ops/task_execution.rs::TaskExecutionSnapshot`, and
`store/migrations.rs` fixtures/readers. Audit all imports again at implementation
time. Preserve pinned Skill bytes, policy, occurrence ID, invocation ID, step
index, iteration, Task claims, pending decisions and exact human Session tokens.
The historical `StepKind` variants and `deserialize_steps` support old data;
retire them only with an explicit data migration, not because new Flows no
longer emit them.

`BodyProvenance` and `StepOutcome` also currently live under playhead but carry
attempt history and resident events. Move needed evidence to its actual owner.
`journal::fold` uses `PlayheadEvent::BodySessionUpdated` to enrich turn bodies;
a reader that simply ignores old playhead events would lose provider linkage.

## Journal cutover and recovery

Implement and test an idempotent disposition before enabling the new resident.
Keep old journal bytes readable. Record the source journal boundary, invocation
identities, disposition and any continuation mapping durably so a crash between
classification and startup cannot duplicate work or silently retire it.

1. Completed history stays historical. An idle default root containing only
   `wave/operate`, with no active body, nested frame or queue, may retire to the
   existing scheduler. Record that retirement; do not report unfinished work as
   successfully completed.
2. Preserve every queued/nested continuation's order, captured definition,
   cursor, iteration and parent/return relationship. Transfer to existing
   ordinary invocation recovery only if its semantics and authority can be
   represented faithfully. Otherwise retain a visible unresolved legacy record
   with its saved facts and an explicit recovery/cancel choice. Never silently
   flatten dependent frames into independent concurrent Runs.
3. Custom root definitions and reader-only legacy plans without executable
   Skill content are not the default one-step root. Preserve their intent and
   missing-evidence reason for explicit disposition. Never compile current
   catalog content and label it the old continuation. Removing `--restart-flow`
   requires a concrete replacement recovery path for these records.
4. An active body requires the existing exact provider/Run ownership and
   liveness evidence. Preserve or reconcile that attempt before scheduling a
   replacement. Unknown liveness remains visible; a missing socket, tmux session,
   or listener is not authority to signal a provider or start a duplicate.
5. Preserve failure/interruption reasons, body/session identifiers and pending
   message claims across restart. A failed attempt must not become completed
   because the cursor was retired. Retrying cutover must produce the same
   disposition and no second continuation launch.

Choose the smallest representation in existing journal/Run/ordinary Flow
ownership that can express these facts. Read-only legacy decoding may survive
the deletion; an indefinitely executable legacy Wave interpreter may not.

## Behavior that must survive

- Cadence: idle heartbeat, due cron handling and its grace window, child
  observations, pause/resume and queued wake ordering. Governance completion
  returns to scheduling; it does not itself wrap or launch another turn.
- Chat: `wave/chat` observation/reply stays independent of governance. Keep
  message IDs, destinations, reply relationships, consumption/requeue semantics,
  streaming and reconnect replay. Chat arrival must retain its current wake or
  queue behavior without advancing an invisible Flow cursor.
- Provider ownership: keep existing Home placement, resident attach authority,
  harness/account choice, native session association, and owned-client
  interruption/stop protections. Listener remains the journal writer;
  resident owns its live provider. Do not infer ownership from UI state.
- Failure: spawn/prepare errors, nonzero exits, timeouts and interruptions leave
  dated, attributable outcomes. Preserve the consecutive-failure cap, resident
  nonzero exit and listener-supervised revival, including message-triggered
  recovery. Listener loss tears down safely. Skip needs an explicit attempt
  disposition after cursor removal; it must not manufacture successful work.
- Task and ordinary Flow execution: exact human authority, fresh autonomous
  Runs, saved-definition recovery, pending-decision settlement and stale-result
  rejection remain under their current owners. Wave attribution still works.

## Completion proof

Use the real resident/listener path with isolated transport side effects, then
demonstrate the configured client path. A pure playhead unit test cannot prove
its own safe deletion.

| Proof | Required observation |
| --- | --- |
| Scheduling and chat | One governance turn per due wake; no immediate root wrap. Pause/restart preserves queued input; chat observes/replies independently; cron and heartbeat retain timing and consumption behavior. |
| Owned provider lifecycle | Interrupt, timeout, listener loss and proven resident death settle/requeue exactly once. A still-live or unknown provider is not duplicated or signaled without exact ownership. |
| Failure visibility | Prepare/spawn failure and failure-cap exit remain visible through journal, CLI and Swift. Recovery distinguishes current, retrying and superseded attempts without PlayheadView. |
| Populated journal upgrade | Cover completed history, default idle root, active body, failed attempt, nested frames, queued definitions, custom root and v1 non-executable plans. Edit/delete source catalogs before recovery; preserved facts and unresolved dispositions survive. Inject interruption during cutover and retry it twice without duplicate work or lost queues. |
| Task compatibility | Decode populated Task positions after the type move. Resume a saved decision once without rerunning its provider; reopen the same human token; reject stale approval and worker claims. No mutable source reload. |
| Consumer removal | Update Rust/Swift DTO fixtures and affected tests. Searches show no executable Wave playhead, enqueue API, root wrapping, hidden singleton dispatch, `/playhead`, live SSE or client queue navigation. Historical readers are explicitly scoped and exercised. |
| Real handoff | Demonstrate scheduled operation, chat, interrupt, visible failure and recovery through the configured CLI/client. Record exact commands and observations; label simulations and unavailable live evidence. |

Review the resulting diff for a single attempt owner and a single scheduler.
Do not accept renamed copies of the old interpreter, green parser tests alone,
or an empty new Home as proof of migration. Run affected suites once, Rust fmt
and all-target Clippy, plus changed Swift/wire checks.

## Documentation contribution findings

This contribution edits only `docs/authoring.md`, `docs/lf.md`, and this directive.
The guides teach loop-decide, exact navigation, keyed Ask/unblock recovery and
pinned continuation while preserving XOR and live-handoff limitations. Existing
concept-review/loop-decide/unblock skill bodies already separate review evidence
from navigation and human completion; no skill change was needed or in scope.

The broader documentation search found out-of-scope Task-only durable Flow and
Session descriptions in `docs/architecture-reference.md` and
`docs/architecture/data.md`. Reconcile those with ordinary invocation ownership
before branch shipment. Do not interpret this bounded guide repair as a complete
repository documentation audit or live runtime acceptance. Existing Ghostty
review notes remain untouched.

During this contribution, the accepted direction and runtime were updated to
remove pass limits. The guides preserve that concurrent change: pass counts
describe history, and neither autonomous nor human Iterate needs a budget reset.

Documentation checks passed: YAML examples parse; local guide links and heading
anchors resolve; the three files have balanced fences and clean whitespace;
the two guides contain no old Task-verdict commands or obsolete decision anchor.
No runtime tests were run for this documentation-only contribution. Source
inspection and these checks do not establish live behavior.
