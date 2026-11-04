# Recovery and delivery semantics

2026-11-03 · Research for Jack Heart · Recommendations distinguished from current behavior

**Treat reconnect as three recovery problems: current Work, terminal presentation, and the outcome of mutations.** A live socket establishes none of their freshness or delivery guarantees. The implementation should tell the phone when each stream has recovered, rather than replacing “disconnected” with “connected” and enabling every action immediately.

## What the current code guarantees

[Rust watches](../rust/loopflow/src/lf/commands/watch.rs) emit a complete reading initially and when its result changes. Store revisions decide when to reread; each reading uses a pinned SQLite snapshot, while Git and process facts have separate observation clocks. Frames contain `answers`, `unavailable` and `body`. They are not an event log. An unavailable reading is explicitly different from an empty result.

[Swift WorkObservation](../swift/Loopflow/Services/WorkObservation.swift) merges independent watches and assigns a sequence inside one observation. A checkpoint asks selected watches to answer after a command commits. It proves subsequent observation, not durable command acceptance. Its sequence resets when the observation is recreated and cannot be reused as a cross-restart replay cursor. Independently consistent parts also do not constitute one globally atomic snapshot.

[Terminal transport](../rust/loopflow/src/transport/README.md) has different semantics: one owner retains terminal state; a dropped viewer has 60 seconds and a 1 MiB byte spool for exact continuation. Expired/missing bytes require a fresh repaint. Input claims release on disconnect and must be reacquired. This is already a strong local foundation, but reconnect data lives in owner memory.

The reviewed [typed Write envelope](../rust/loopflow/src/lf/commands/write.rs) does not provide general client-operation deduplication. A Process record and a successful TCP write are not equivalent to a durable, queryable acceptance receipt. Some domain operations have revisions or their own recovery behavior; each must retain that meaning.

## Three explicit contracts

| Stream | Recovery identity | Recovery rule | Retention boundary |
|---|---|---|---|
| Work | Machine + subscription + scope generation | Fresh complete reading; optional later event replay | Current authoritative projection, not every intermediate change |
| Terminal | Machine + owner incarnation + viewer stream + byte offset | Continue retained bytes, otherwise reset from snapshot and apply subsequent output | Owner life, spool bytes, reconnect grace, retained scrollback |
| Mutation | Machine + authenticated principal + operation ID | Query/retry the same semantic operation; return recorded outcome | Durable receipt lifetime explicitly advertised |

**Proposed Work contract:** begin with snapshot replacement, matching existing watches. Give every subscription a generation so late results from a previous Task or repository cannot overwrite the new view. If incremental events are later justified, introduce an epoch and monotonic cursor scoped to that stream; changing server incarnation, projection meaning or authorization forces a fresh snapshot. Store revisions remain internal invalidation signals unless a replay log is deliberately added.

Rex starts subscription delivery before reading its snapshot, then instructs clients to apply only events beyond the response sequence. It closes lagging control clients beyond an 8 MiB event backlog instead of silently skipping events. Borrow this explicit cut between snapshot and subsequent updates. [Rex API](https://www.superlogical.com/rex/docs/reference/api)

AHP distinguishes replayable state actions from ephemeral notifications and returns either replay or snapshots on reconnect, including subscriptions that cannot resume. Its explicit result shape is useful; adopting its entire agent model is unnecessary. [AHP lifecycle](https://github.com/microsoft/agent-host-protocol/blob/main/docs/specification/lifecycle.md)

**Proposed terminal contract:** retain existing bounded continuation. A phone resuming with live emulator state may use the exact byte offset; a newly launched phone process should request a snapshot unless emulator/parser state was persisted consistently with that offset. Snapshot reset, resize and replay completion are barriers. Do not append a snapshot to old display content or allow live bytes to overtake its final chunk. An owner change invalidates prior input authority even if the durable Session ID is unchanged.

Mosh synchronizes current screen state rather than delivering every intermediate terminal output, making recovery under poor connectivity efficient. That is a useful distinction between visual state and audit history; it is not a substitute for durable conversation records. [Mosh](https://mosh.org/)

Eternal Terminal uses authenticated reconnection, bidirectional received-sequence exchange and catch-up buffers; a fresh client or lost history follows a negotiated reset. Its surviving terminal can re-register after the server restarts. The lesson is that byte continuation requires retained identity and history at both endpoints, not merely reconnecting TCP. [ET protocol](https://github.com/MisterTea/EternalTerminal/blob/master/docs/protocol.md)

## Mutation acceptance and the crash window

**Proposed minimum:** every structured mutating request carries an unpredictable operation ID and an exact typed payload. The host stores its principal, payload fingerprint, acceptance state and resulting domain/Process identity. Reusing an ID with different arguments is an error. A retry reads the same operation; a new ID expresses a new intent.

Stripe’s v1 API stores the first executed response under an idempotency key, rejects mismatched parameters and documents key expiration. Its newer v2 behavior differs, illustrating why “idempotent” needs a precise contract. Borrow stable semantic request identity and explicit retention, not payment-specific retry policy. [Stripe idempotency](https://docs.stripe.com/api/idempotent_requests?lang=curl)

For an entirely local database change, write the mutation and receipt in one transaction. For launching a process or calling an external provider, that transaction cannot make both systems atomic. Persisting “accepted” before dispatch prevents loss of the intent, but a crash after dispatch and before recording its result still leaves ambiguity. Reconcile using a stable execution identity and an authoritative owner acknowledgement. Where the provider offers no correlation/deduplication, report `outcome_unknown`; do not invent exactly-once execution.

Use separate states for received, durably accepted, execution started and completed. A UI button may clear its sending state after durable acceptance, while execution remains pending. For operations not yet supporting durable acceptance, expose the weaker contract and disable automatic resubmission after uncertain disconnects. Never automatically rerun a Graph merely because its response disappeared.

The same principle applies to prompt submission. An optimistic message and a canonical acknowledged message must share an operation identity. A late transport error must not remove a prompt already observed canonically. Paseo separates live deltas from authoritative history, uses epochs for replacement, and distinguishes canonical submission acknowledgement from RPC settlement. Its pagination continues until catch-up is complete rather than treating the first page as freshness. [Paseo timeline sync](https://github.com/getpaseo/paseo/blob/main/docs/timeline-sync.md)

**Raw terminal keys are not structured mutations.** Current input frames are authorized bytes without per-input durable acknowledgement. If Enter reaches a shell and the connection drops, replaying it can run another command. Keep offline text in a local unsent draft; after reconnect, show the recovered terminal and require deliberate submission. Reliable retransmission would need owner-side sequence deduplication tied to an incarnation, plus a definition of what acknowledgement means. Receipt into memory cannot prove execution by the program.

## Bounded queues and responsive controls

Current owners use 64 KiB stream frames and 4 MiB payload/queue limits; slow viewers disconnect while execution keeps draining. Preserve this isolation through every added gateway and phone queue. A bounded owner queue followed by an unbounded WebSocket queue is still an unbounded system. Bound logical snapshots as well as frame sizes and decode incrementally.

Paseo documents incremental replay, immediate first-chunk flushing, per-viewer replay isolation and coalescing before IPC. It also reports large agent messages delaying terminal echo on shared event loops. These are concrete reasons to measure the whole pipeline and keep large history/diff work from monopolizing it. [Paseo terminal performance](https://github.com/getpaseo/paseo/blob/main/docs/terminal-performance.md)

**Proposed policy:** prioritize revoke-control, cancel, health and small mutation receipts over bulk replay. Coalesce replaceable Work snapshots; never silently drop terminal bytes inside an established exact stream. On overload, reset or disconnect with an explicit recovery reason. Give every client its own budget so a suspended phone cannot stall Desktop. Separate streams still need scheduling: naming a control channel does not prevent shared CPU or socket contention.

## Proof before trusting recovery

Test disconnect immediately before acceptance, after acceptance, after side effect and before result; terminate the adapter at the same points. Verify one domain mutation or an honest unknown outcome. Exercise a spool overrun, new owner, stale scope reply, slow phone, simultaneous viewers, split UTF-8/escape sequences and replay during continuous output. Confirm cancellation remains responsive under a large diff and terminal flood. Foreground recovery must refresh state before claiming freshness; cached content can remain readable throughout.

These are protocol acceptance tests, not a demand to build an offline synchronization platform. Start with authoritative snapshot replacement, bounded terminal recovery and a small set of mutation receipts. A future hub can later use the same semantic-operation boundary for queued event delivery.
