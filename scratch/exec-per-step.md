# One Exec per Flow step

LOO-298 · Jack Heart · 2026-09-29. Implementation design, not acceptance.

Finish line: Task and taskless Flows show driver → step → agent-issued command
in `lf ps` and `lf exec list`; killing the driver leaves a surviving step to be
observed on resume without another agent. Typed decisions settle once. Stop and
liveness name the actual driver, including after startup handoff. Existing
provider-generation and sibling-conversation proofs remain binding.

## Owners and execution

- Exec owns the actual lf process and its exit. No launch object, reservation ID,
  launch table, or second outcome owner. Captured input remains immutable Session
  history: it records instructions, not an agent start. Its nullable `exec_id`
  names the executing process; publication remains a later artifact fact.
  Imported capture without a proven process keeps no Exec. Review preparation
  can precede its eventual `session open` Exec without inventing one.
- One child entry point executes the selected captured Flow boundary. The driver
  supplies Flow identity/version and its existing claim, not recompiled skill
  content. Child registration precedes reservation; the child's transaction
  writes the capture with its own Exec. Remove reservation from Task claim
  acquisition and ordinary driver traversal.
- Reuse the direct command's prompt/capture/provider executor for each agent
  child. Delete TaskLauncher and SavedLauncher. Move Task seed, account choice,
  steers, interrupts, activity and attachment handling into that common path;
  retain Task review parking as policy, not a second provider executor. Forward
  attached driver input to the active child. Preserve direct shell directives,
  transient retries, raw output and Task agent precedence.
- Mechanical boundaries execute in a child lf too. Existing Flow operation
  events name that child's Exec; ordinary command dispatch performs the effect.
  Missing operation completion remains inspect-before-retry, even if lf exited.
- Driver reads the exact selected native result from Session history. Child exit
  is transport/process evidence, never the decision. No stdout result protocol.
  Human reviews remain their existing `session open` processes and completion
  transaction; a waiting driver does not start another provider.

## Reservation, death and ownership

A child checks the exact boundary/claim and atomically selects its capture before
provider effects. Only its recorded Exec can publish/execute that selection.
Concurrent child starts may create command observations, but cannot create two
provider starts. Existing artifact publication protects immutable bytes; it is
not the process-admission owner. A stale child rejected after claim replacement
performs no provider effect.

Before replacing a dead driver's claim, inspect the selected capture/operation's
Exec using recorded PID/start identity. Wait for a live step to finish under its
original claim; do not steal its Session driver or invalidate its selected turn.
Then reclaim and consume its exact successful result. A dead step with surviving
provider uses existing native-history recovery. Unknown process evidence stays
unknown; no duplicate execution. Pre-publication death retains the old capture
and selects a new one only after exact death evidence. Imported missing Exec
uses existing explicit recovery, never fabricated liveness.

Task startup hands the launcher's claim to the actual worker transactionally,
matching the complete prior claim. Stop/liveness read that actual owner. Startup
acknowledges that handoff rather than mistaking a reserved artifact for a worker.
Explicit stop also observes the selected step's exact Exec; a dead driver alone
cannot release live work. Durable interrupts remain the first provider request.

## Parent tree: before and after

| Existing pointer | After |
| --- | --- |
| `execs.parent_exec_id` | Single causal lf tree: driver → step → agent-issued child → grandchild. |
| `LF_PROCESS_ID`, `LF_TRACE_ID` | Direct lf child inheritance; provider environments currently strip both. |
| `LF_AGENT_CALLER`, caller Session/provider generation | Keep: resolve the current conversation driver through generation and origin; stale engines retain proven historical caller. Shared engines must not borrow sibling ownership. |
| `via_agent` | Review whether derivable from retained caller identity; never lose the distinction used for authority. |
| `caller_flow_turn`, `AgentCaller.flow_turn`, index | Delete: no production writer. |
| `LF_PARENT_RUN_ID` | Delete: no production reader. |
| Session driver/provider Exec + generations | Keep: mutable driver and surviving/shared engine are different facts. |
| Session event Exec/capture references | Keep: native turns and instructions are not nodes in the process tree. |
| `flow_sessions.parent_id` | Keep: runtime loop nesting, not Exec ancestry. |
| captured caller key / manifest parent | Remove new ancestry copy; Exec supplies it. Retain old import decoding. Separate replay source and Ask identity from ancestry. |
| journal parent process copy | Trace journal is outside this naming cut; verify whether its writer still needs a duplicate. |

## Attribution: before and after

| Fact | Owner and reduction |
| --- | --- |
| Current conversation Task/Wave + source/bound time | AgentSession. Task implies Wave; existing ancestry validation stays. |
| Flow work | FlowSession; Session membership must agree on writes. Runtime children derive/validate against parent. |
| Prospective history owner | Immutable capture or native start observation, not every output/usage/completion. Derive child receipts through that observation; validate Task→Wave and live Session agreement on new writes. Preserve imported evidence exactly. |
| Post-hoc usage alternative | One history-reader choice of current Session assignment versus retained observation; keep prospective until Jack decides. Timestamps alone cannot reconstruct same-second binding order. |
| Bound repository | Wave, via Task when present. Remove unvalidated Session repo copies; keep nullable unbound observation and validate assignment. |
| Exec cwd/repo | Command context, distinct from work performed. |
| Exec Wave string | Delete; derive performed work from Session/Flow events. |
| Manifest subjects/Flow copies | Import evidence only; new readers derive from SQLite owners. Old manifests still decode unchanged. |
| Started | Existing monotonic materialization, set by actual work reservation; an inspection Exec cannot set it. |

Implement process ownership first, then attribution reduction, then the separate
Exec/Session naming commit. No installed-Home access. Proof uses isolated stores,
scripted providers and real lf subprocesses; configured acceptance stays explicit.
