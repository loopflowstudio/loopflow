# Simplification Opportunities

## Product intent

Loopflow conducts durable Work through short-lived, fenced Runs while remaining reconstructible across provider and process loss. The core should expose product boundaries—Work, Epoch, Run, Turn, and Steer—without leaking launcher ceremony or maintaining parallel accounting truths.

## Opportunity 1: Make Run a three-operation state machine

**Misalignment**: Current Task and Project process generations each expose reserve, activate, finish, revoke, and finish-after-reap operations. The design draft then adds boot, continue, settle, interrupt, reap, and retry as if each were an independent Run concept.

**Symptom**: The same lease state machine is duplicated for Task and Project, while crash recovery must reason about combinations such as terminal Work with an active lease or a dead process pinned in `revoked`.

**Realignment**: Give Run three domain operations:

- `reserve` atomically claims the Epoch's one active slot and records `Starting`;
- `advance` consumes one execution boundary and atomically chooses the next Turn/Launch or ends the Run;
- `stop` immediately fences authority, owns physical cleanup, and reaches `Done` only after executor absence is established.

Execution `started`, progress, and reaped are receipts inside those operations. `revoke` and `reap` are internal stop phases. `retry` is reconciliation creating another Run with the prior Run as trigger. `interrupt` terminates a Turn or opaque Launch interaction, never a Run by definition.

**Cascade**: Task and Project lose parallel process-generation APIs. Keeper recovery uses the same Run state machine. The active slot, stale-write fence, cleanup obligation, and retry precondition become one aggregate instead of cross-checked Session fields.

## Opportunity 2: Make Turn the only usage grain

**Misalignment**: `run_events` and `agent_turns` both record token and cost measurements. The former stores usage beside process/skill boundaries; the latter stores provider-measured Turn usage and context metrics. Readers must know which snapshots are cumulative, which are deltas, and which missing reports were defaulted to zero.

**Symptom**: The usage-delta migration repairs both ledgers independently, `lf usage` still reads `run_events`, and provider mappers can overwrite real or absent usage with defaults. Trace `RunId` also conflicts with the proposed product Run.

**Realignment**: Persist normalized provider usage only on Turn. Derive Run, Epoch, Work, repo, provider, model, account, skill, and flow totals by joining and summing `Run → Launch → Turn`. Missing provider usage remains `None`, never zero. Raw provider events remain audit artifacts, not query authority. Remove token, cost, provider, and model accounting from `run_events`; retain only execution lineage until Launch/Handle/Exec receipts can replace that table entirely. Rename its current trace and process identifiers instead of treating them as product Run ids.

`provider_account_limits` remains separate. It is the latest observed subscription-window state, not an additive spend ledger.

**Cascade**: One parser path owns usage end to end. Run settlement and reaping no longer coordinate accounting writes. Retries retain their own Turn costs and roll up through lineage without overwriting the failed attempt. `lf usage`, monitoring, and budgets all read one fact at different groupings.

## Opportunity 3: Separate recovery from replay

**Misalignment**: “Retry a Run” suggests repeating an opaque execution, even though agents have already changed a durable workspace and may have made external side effects.

**Symptom**: Recovery code must guess whether delivery or execution was safe, while provider/process death is conflated with replay permission.

**Realignment**: A failed or stopped Run becomes immutable. Reconciliation may reserve another Run only after the prior executor tree is absent. The new Run reconstructs current workspace and external evidence; it never blindly replays the old command stream. Unknown local filesystem effects are inspected in place. Unknown non-idempotent external effects produce a typed Wait until reconciled. Loopflow-owned external mutations use idempotency keys where available.

**Cascade**: No `retry(run)` mutation, replay flag, or successor lease state is needed. Recovery becomes ordinary new execution from durable truth, and unsafe ambiguity becomes visible Work state rather than hidden runner policy.

## Opportunity 4: Replace sleep and block with typed waiting

**Misalignment**: `Waiting` and `Blocked` are stored as distinct lifecycle states even though both mean that Work remains open, has no active execution authority, and needs another fact before a useful Run can start. Free-text reasons then conflate a known dependency, an exhausted runtime retry, an unreachable Home, and ordinary quiescence.

**Symptom**: Status transitions must guess which parked label applies, legal-action tables branch over combinations that differ only by who can satisfy the wait, and “unblock” can clear a label without making the missing fact true.

**Realignment**: Keep the Epoch `Open`. Ending a nonterminal Run records one typed `Wait` that names what can wake the Work: input, time, external evidence, child Work, capability restoration, or effect reconciliation. Work with an active Run is Running; open Work without one is Waiting. Resolve or invalidate the Wait from its authoritative evidence rather than through generic `wake` or `unblock` mutations. An empty/manual-input wait permits indefinite dormancy. A failed Run or unreachable Home remains runtime or topology evidence until reconciliation either starts another Run or produces a specific Wait.

An interactive flow step never becomes a Work Wait. In attended mode it is a TUI `Launch` inside the active Task Run, openable in the Swift app whether or not a window is attached. `AwaitingHuman` is a projection over that live Launch, not stored Work state. In non-blocking mode the flow routes the request into the parent steering path and the response becomes a child Steer; the Task does not pretend to wait on a parent as if it were waiting on a human. There is no `Interaction` entity or `InteractionId`. The current `InteractionReview` and `InteractiveHandoff` records collapse into flow position, Launch, and Steer.

**Cascade**: `WorkState` needs only `Open`, `Done`, and `Abandoned`; `Blocked` and `Sleeping` disappear from storage and the public lifecycle. Monitoring can still group waits by `on` without maintaining two state machines. Home reachability remains an observation over execution location rather than a status copied onto every Work item.

## Opportunity 5: Make provider continuation Launch data

**Misalignment**: The draft promotes provider `Handle` and captured `Turn` into required reconstruction concepts even though Loopflow's tmux TUI surface exposes neither inner Turns nor a portable child-session graph. That makes provider transcripts secretly authoritative while claiming they are disposable.

**Symptom**: Reconstruction asks for a minimum Turn record that does not exist on every supported surface, account and provider fallback appear to change Work attempts, and native child ids acquire durable lifecycle meaning they cannot carry consistently.

**Realignment**: Reconstruct from current domain truth: Work Basis, ordered Steers, flow position, Workspace/HEAD, typed decisions and approvals, current PR/CI/review evidence, and Loopflow-mediated effect receipts. Summaries and provider events are optional context. A Launch may store an opaque resume token with the provider/account/Home that can use it; it does not create a separate Handle entity. A new Launch may resume that token when compatible or start clean from one rendered context projection. Observed Turns remain optional trace and usage evidence beneath a Launch, never the recovery floor.

Provider, model, or account fallback starts another Launch in the same active Run. A new Run is required only after execution authority ended or was fenced and reaped.

**Cascade**: `HandleId`, root/child Handle tables, resumability flags, and provider-session fields on Work disappear. Opaque TUI Launches and structured app-server Launches share one lifecycle. Reconstruction no longer depends on transcript availability, and fallback does not fabricate a new Work attempt.

## Opportunity 6: Contain the executor, not the subagent graph

**Misalignment**: The draft requires Loopflow to discover and close every provider-native descendant even though Claude, Codex, OpenCode, and opaque TUIs expose different or incomplete child-session evidence. The enforceable local boundary is already the provider process group/tmux session, not the provider conversation graph.

**Symptom**: Idle resumable child sessions look like live execution, completion depends on provider-specific enumeration, and native children appear to need independent Loopflow principals even though they execute with the root harness's environment and tools.

**Realignment**: Every Launch runs inside a Run-owned containment unit: process group/tmux session now, a stronger cgroup or sandbox where available. Run end waits until every owned Launch and containment unit is terminal and absent. Idle provider conversation ids need not be closed. Child ids and events are optional trace evidence. Targeted provider cancellation is a best-effort optimization before terminating the root Launch. A provider feature that can continue mutating after the containment unit dies is unsupported until it exposes a reliable stop/status fence.

Native subagents are not principals. They act with the root Run's effective workspace, tools, and Loopflow authority, and every mutation is attributed to that Run. Claiming a smaller authority would require a new broker or sandbox that the current substrate does not provide.

**Cascade**: No durable descendant state machine, child lease, or completion join is required. Reaping has one proof—owned containment is empty. Completion waits for possible writers, not for dormant provider transcripts. Provider-specific child inspection remains diagnostics rather than correctness machinery.

## Aligned areas

The current lease token correctly prevents stale writers, reservation before launch correctly closes the duplicate-launch crash gap, and revoked execution correctly retains the active slot until cleanup. Provider `ConversationEvent::TurnUsage` is already the right normalized boundary. These invariants should survive while the duplicated APIs and ledgers disappear.
