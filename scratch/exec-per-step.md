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

## Current boundary

Discovery is published at `dbad495f4` (Rust 2,029 passed/16 skipped; Swift 291
passed). The first publish attempt made a preparation commit and then returned
a mutation-lock conflict; the clean retry succeeded and GitHub reports that head.
No concurrent writer or lock cause was established.

Worker-claim handoff is implemented locally before the child-executor conversion.
The actual worker atomically adopts the exact startup claim; the launch caller
waits for that handoff. Seven focused ownership/stop tests pass in
`.lf/tmp/cut-i/exec-worker-focused.log`, including two actual sleep processes:
startup cannot acknowledge the launcher, stop targets the adopted worker, and
the launcher survives. The store proof preserves capture/cursor/generation and
rejects writes with the old claim. These fixtures do not prove public Task worker
startup, a surviving step after driver death, or the complete parent tree.

Mechanical boundaries now execute through hidden `lf __flow-step ID VERSION`.
The child uses the driver's exact executable and inherited data context, reads the
captured operation, and records its own Exec on the existing Flow start/result.
The driver consumes a saved success without creating another step process.
No new reservation object, result protocol or migration is introduced.

Recovery waits for a surviving mechanical step before reclaiming its driver;
missing exact process evidence remains unresolved. Task status includes the
selected live step, checkout restoration retains its ownership, and stop waits
for both driver and step death before releasing the claim. These are selected
operation links, not authority inferred from causal ancestry.

The initial stop proof caught an unintended Interrupt write for unknown process
identity. It is fixed: interruption still requires live driver or selected-step
evidence. Focused proof covers real public child processes, success then failure,
killed-driver recovery without replay, Task Started/claim retention, stale child
rejection, and prior stop/missing-outcome cases. Logs and final validation follow
in the checkpoint evidence; this remains a mechanical slice, not full item 1.

Next: move agent reservation and execution into this same child entry. Delete
TaskLauncher/SavedLauncher by moving Task seed/account/steer/attachment support
into the common provider executor. Capture ownership, surviving provider recovery,
agent-issued descendants and driver-handoff ancestry remain outstanding. The full isolated Rust matrix passes before this checkpoint; rerun it
before publishing another source change. Attribution reduction and the
separate naming commit still follow the complete process-owner conversion.

Review before checkpoint: moving the operation into a child exposed three
concrete edges. The synchronous operation stays on a blocking thread because
some operations own a Tokio runtime. A retained successful result bypasses child
creation on resume. Unknown Task process identity must leave interrupt history
unchanged. All three are reflected in the code and focused proof. The final
focused command passed 13 tests, with one nextest leaky-handle diagnostic in the
existing execution-state test (`.lf/tmp/cut-i/exec-step-final-focused.log`). The
architecture check passes after registering the new process boundary.

Final checkpoint proof: 2,032 materialized Rust tests passed, 16 skipped, none
unrun, fail-fast disabled. All-target Clippy, formatting and architecture checks
pass. The full matrix exposed the simulated worker's missing startup adoption
and landing's old in-process ancestry assertion; both fixtures now model the
actual contract. Landing's repair command also consumes its thread tool caller
before invoking rebase. [Evidence](evidence.md#mechanical-flow-step-checkpoint)
retains intermediate failures and exact logs. No agent-step or configured-provider
acceptance follows from this mechanical checkpoint.

Compression keeps claim selection and process observation separate: both Exec
and Task-owner readers now share the receipt's PID/start-time comparison, while
retaining their distinct receipt matching and terminal-evidence fallbacks. Step
launchers take the claim from the already-validated Flow snapshot rather than a
second argument. Flow rendering uses only the captured steps; unused repository
arguments, infallible Result wrappers and a redundant XOR adapter are removed.
The 52 selected source tests pass in `.lf/tmp/cut-i/exec-compress-focused.log`,
covering worker handoff/stop, surviving mechanical children, Task/taskless
continuation and rendered review/XOR paths. This reuses the existing fixtures;
it does not repeat or replace the full materialized matrix above. All-target
Clippy (`exec-compress-clippy.log`), formatting and architecture checks pass.

## Slice review · 2026-09-29

Scope: mechanical child execution at `3c5bb5ab3`, worker startup handoff at
`5bd311697`, and the retained compression edits. Jack Heart's full agent-step
conversion remains the next implementation boundary. This review does not accept
the complete Exec-per-step design or LOO-298.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Each mechanical boundary has a real Exec | Driver delegates the saved operation to a child | One hidden child entry reads the captured boundary; start/result reference its Exec | Public CLI two-step success and success-then-failure fixtures inspect distinct child Execs and retained outcomes | Covered locally |
| Driver death preserves live work | Resume waits without repeating the effect | Exact selected operation Exec is observed before claim replacement or recovery | Public CLI kills the driver during a scripted operation; resume waits, advances once, retains original parent and unknown driver outcome | Covered locally |
| Child admission cannot move the cursor | Validate boundary/version/claim before effects | Start transaction selects once and sets Started; completion compares the selected start and Exec as well as claim/version | Task child fixture rejects stale version, runs after source deletion, preserves driver claim/cursor and records one effect on replay | Covered locally |
| Stop follows actual ownership | Adopt worker at startup; retain live selected steps | Exact startup claim transfers to worker; stop observes driver and selected operation separately | Worker handoff, surviving-step stop and unknown-identity fixtures; real sleep processes, simulated Task startup | Covered at fixture boundary; configured Task startup remains unproven |
| Compression preserves authority and rendering | Remove duplicated arguments/comparison code only | Claim comes from the validated Flow row; receipt readers retain different matching and fallback rules; rendering uses captured steps | Source review and 52 retained focused passes, including review/XOR rendering | Covered locally |
| All agent steps use the child entry | Common provider executor; capture belongs to child | TaskLauncher and SavedLauncher still execute agent work in the driver | Reachable-path inspection | Outstanding next slice |

Ownership review found one production start/result writer for Flow operations:
`execute_step` calls the existing transactional methods. The separate direct
operation CLI remains a normal command, not a second Flow progression writer.
No new table, result protocol, Session for mechanical work, or ancestry-based
signal authority is introduced. A successful child exit cannot substitute for
the saved operation result: cursor settlement validates that result again.
The compression does not merge Exec and Task-owner receipt lookup semantics.

The public CLI proofs use real lf processes and isolated SQLite stores with
scripted effects. They do not establish configured providers, rendered Desktop,
exactly-once external effects after missing completion, or agent-step ancestry
after driver handoff. The existing full-tree/import obligations remain intact.

Fresh validation: `.lf/tmp/cut-i/exec-review-full.log` passes the isolated,
materialized Rust matrix with fail-fast disabled: **2,032 passed, 16 skipped,
none unrun, no leak diagnostic**. This includes the public mechanical-step and
driver-death demonstrations above. `.lf/tmp/cut-i/exec-review-source.json`
records the source snapshot and command; all 466 Rust, Cargo and fixture inputs
still match. Only this review note changed after snapshot creation. Fresh
formatting, diff validation and architecture checks pass
(`exec-review-architecture.log`). The unchanged compression source retains its
all-target Clippy pass in `exec-compress-clippy.log`; Swift retains the prior
291-test result, with no Swift changes in this slice.

Disposition: the mechanical slice and compression are coherent for checkpoint
publication. No new bounded implementation defect was found in this review.
Next action: move agent capture reservation and execution into the shared child entry, then
delete TaskLauncher/SavedLauncher while preserving Task input/account/steer and
native recovery behavior. Attribution reduction and the separate naming commit
follow that conversion; this slice supplies no Flow navigation or Task settlement.

## Direct skill command cut · 2026-09-29

Jack Heart raised the finish line: a step executes the same `lf skill` command
as a direct caller. Moving provider spawn alone is insufficient. Starting a Flow
compiles every referenced definition and XOR alternative before any step runs.
Composition provenance remains a display lens; only runtime loop passes have
additional FlowSessions. This cut is implemented locally; final acceptance and
the remaining proofs below are outstanding.

The table uses baseline `fd9cf980b`. Paths are under `rust/loopflow/src/`;
“after” line numbers identify this working cut, before the separate naming commit.

| Step kind | Before entry | After entry and child command |
| --- | --- | --- |
| Op | `lf/commands/flow.rs:811`, mechanical child entry | `flow.rs:808,865`: `lf __flow-step ID VERSION`, dispatching the captured op through its ordinary command operation. Existing mechanical success/death proof remains applicable. |
| Ordinary skill | `SavedLauncher`, `flow.rs:878`; `run_saved`, `run.rs:57`, inside driver | `flow.rs:718,865` → `lf --batch OPTIONS --__flow-step TOKEN skill NAME` → `bin/lf.rs:511` → `run.rs:78`. Capture belongs to the child Exec. |
| Task skill | `TaskLauncher`, `controller/task/mod.rs:210,258` | Same child command and direct skill entry. `controller/task/mod.rs:36` only drives the saved Flow and retains Task unblock policy. |
| XOR router | Ordinary or Task launcher, depending on caller | Same `lf skill ROUTER` child. Captured instructions supply the typed path contract; XOR itself remains graph structure. |
| Human review skill | `ops/human_session.rs:1091` and `ops/flow_session.rs:132` prepared interactive children | `lf --tui --model MODEL --as task:ID skill NAME MESSAGE` for a Task review; `lf --tui [WORK/MODEL] skill NAME MESSAGE` for a standalone review. Existing Session token selects captured instructions. Direct dispatch now accepts those instructions before catalog lookup, including after the source disappears. Readiness/completion remain review policy. |
| Authored subflow | Compiled into captured steps | No command, Exec, claim or FlowSession. Every contained skill/op follows the rows above. |
| Runtime loop pass | Shared driver traversed its saved child FlowSession | Same driver and existing loop history; its skills/ops use the child command above. No new driver. |

| Machinery / launcher | Before | After |
| --- | --- | --- |
| Prompt/context | `run.rs:222,248` Task preparation wrappers; `controller/task/mod.rs:478` appended feedback and output contract separately | Deleted. `run.rs:78` supplies captured/Task message input; common `build_prompt_at` at `run.rs:377` assembles context. |
| Capture/publication | Task launcher built its own RunSpec/capture; driver reserved input on claim | Deleted. `run.rs:835` publishes through the common capture path with the original validated claim. Child reservation records its Exec; a claim alone no longer marks Task Started. |
| Harness, routing, retries, raw output, usage, final answer | Task launcher had a second provider event loop and fixed account | Deleted. `engine/agent.rs:1147,1219,1547` owns the ordinary command path and retries. Reserved headless inputs now retain its normal RunLaunchRequest. |
| Agent precedence | Task `resolve_task_agent` preferred skill default over repo config | `engine/launch.rs:214` supplies one precedence rule; persisted Task choice is an explicit override. |
| Steers, attachment, interrupts | Task launcher event loop and terminal receiver | `ops/task_input.rs:21,132,180` supplies message/control input to the common native loop. Child inherits stdin, reads durable steers/interrupts, and independently refreshes Linear comments. Cursors survive transient retries. No Task harness remains. |
| Journal skill events / automatic checkpoint | `flow.rs:975,1019` duplicated command wrappers | Deleted. `bin/lf.rs:463,511` owns both. Common Flow drive brackets initial execution, Task worker and resume with Flow events. |
| Failure reporting | Generic child status discarded the provider reason | Driver reads selected input's recorded error; scripted OpenCode failure now reports `work-proof: opencode_error: fixture failure`. |
| Alternate launcher tests | StepLauncher, SliceHarness, RecordingControlHarness, UnusedHarness, CreateHarness injection | Deleted. Retained store tests assert state directly; real lf child tests and scripted native providers prove execution. Session executable pinning remains ordinary fixture isolation. |
| Task launch preflight | Task validates writable Git/store boundary and managed provider credentials before effects | Retained policy exception for Jack's review. No selected account is forwarded to a second launcher. The skill command chooses its account normally. See safety boundary below. |

Inspection of `engine/invocation.rs:117`, `engine/flow.rs:350,904,1050` and
`engine/execution.rs::FlowEngine::tick` finds Flow-name loading only while
compiling/selecting a new invocation. Execution traverses captured skills, ops
and XOR alternatives. No lazy Flow-name lookup was found in the runtime driver.
The existing source-removal and nested-route tests remain relevant.

### Controls and safety boundary

Task seed, workspace context, user name, write scope and execution boundary are
inputs to the shared command. Task capability-text classification is deleted;
ordinary outcome classification governs retry/failover. The Git-operation fence,
raw recording, context flags and context/token display come from the direct path.
Claude's native path now carries Chrome selection, ordinary API-key filtering and
account routing, which the audit found missing.

A live step retains the driver's original claim until it exits. Stop, status and
checkout recovery now inspect selected skill Execs as well as op Execs. Native
completion does not fabricate the dead driver's command outcome. Causal ancestry
still gives no authority to interrupt a shared provider engine.

The managed terminal proof found that killing the tmux worker can also hang up
its skill client. That observation is retained, not recast as a surviving child.
The replacement fixture keeps its detached native engine alive and exposes its
history to a new client. Resume preserves the original selected native turn and
consumes that exact completion without another turn. Ordinary Flow driver death
separately proves a surviving skill child. No new detached-child process policy
was introduced to hide the terminal behavior.

One Task policy remains a review question: `ops/task.rs::preflight_task_execution`
requires a managed account and verifies it before Task/provider effects, while a
direct skill may use ambient provider credentials. Deleting this would change
Task-create preflight and attribution policy. OpenCode also lacks the Task
execution-boundary treatment used by Codex/Claude. Those restrictions remain;
Jack has not approved weakening them through the direct-path conversion. This
exception must be resolved before claiming every Task/direct difference closed.

### Audit disposition and proof

The operator audit at `.lf/tmp/skill-leak/audit.md` was read after its release.
Its earlier absence statement is superseded. Wave-name resolution and original
claim publication are repaired; public managed launch reached its provider with
Wave docs, user name and forwarded `--docs`. Child account flags use the existing
inherited-lease branch (`bin/lf.rs`), so inspection did not support the inferred
second-lease creation. A dedicated account-lease transport proof remains open.
Provider failure detail and resume Flow journaling are repaired; the unused
checkpoint wrapper is deleted.

- `task-command-ownership.log`: 97 focused passes, including ordinary skill
  driver death, mechanical recovery, Task stop/claim cases, shared-engine
  exclusion and Exec ancestry. No full final-tree matrix is claimed.
- `task-command-regressions.log`: five passes covering the two stale test
  assumptions plus common transient resume/failover behavior. These unit cases
  do not establish real account failover.
- `task-command-clippy4.log`: all-target Clippy passes. Formatting and architecture
  checks pass; no Swift change is made in this cut.
- `task-command-controls5.log` and its source fingerprint: disposable Linux,
  real public lf/tmux, scripted Linear and Codex. Attached input becomes one
  provider steer; worker death plus native history recovery consumes the same
  completion once; Project-default Task launch uses config before skill default;
  Wave/user/`--docs` context survives; Chapter rotation and second-private-Home
  public sync preserve Task, Started, PR, worktree and captured Flow.
- `task-command-interrupt4.log` and its source fingerprint: attached input then
  `/interrupt` reaches the selected native turn, releases the claim, preserves
  the exact cursor/captured graph and consumes no Flow completion. This public
  Linux proof uses scripted providers. The earlier assertion named a nonexistent
  `cursor_json` field after interruption had occurred; that failed proof stays
  retained. Subsequent Docker package failures were bypassed by caching the same
  fixture dependencies locally, without changing production behavior.
- `task-command-review3.log`: a public standalone review executes `lf skill`
  after both Flow and skill source files disappear, retains captured instructions
  and stays at review. The scripted provider remains live until startup has been
  observed through the existing Session lock. Earlier fixtures ended before the
  opener observed a resumable client; identity publication alone was insufficient.
- `task-command-review-journal.log`: the surviving-child case passes explicit
  child skill journal and resume Flow journal assertions. That run also contained
  an earlier failed review fixture; it is not a fully passing run.
- Earlier controls run recorded client interruption on worker death. Two reruns
  failed before testing because Debian package downloads timed out; another
  failed because the revised provider fixture omitted its stdio auth endpoint.
  The successful replacement does not erase those observations.

Remaining for item 1: inherited account lease/failover and Task control
continuity through retry, plus the managed decision/failure cases below. Preserve provider-generation/handoff and
shared-engine proofs. Configured providers, Desktop and installed acceptance are
still unproven. Then perform attribution reduction, Jack's naming choices,
released-populated import/canonical proof, and final documentation in the accepted
order. No publication, Task completion, promotion or acceptance follows here.

### Naming proposals for Jack Heart

These are proposals for the separate naming commit, not selected schema changes.

| Current name | Proposed name | Meaning |
| --- | --- | --- |
| `flow_sessions.parent_id` | `pass_of_id` | Runtime pass belongs to the FlowSession waiting for its result. |
| `flow_parents` / graph `parents` | `definition_path` | Ordered definition provenance for grouping compiled steps. Display only. |
| `expand_flow` and equivalent definition-to-graph terms | `compile_flow` / compile | Compile definitions, then capture the compiled graph and step input. |
| `execs.parent_exec_id` | Keep | The one causal parent relation: an Exec's parent Exec. |

### Measured deletion

Against `fd9cf980b`, Rust source/tests plus the public CLI fixture currently add
1,433 lines and delete 2,848: **net -1,415 lines**, including the new
`ops/task_input.rs` file. This counts inline tests as tests/code, not production
reduction. The controller/Flow/run preparation prefixes before their test blocks,
plus the entire new input module, separately total **-357 lines**:

| File prefix (before tests) | Before | After | Net |
| --- | ---: | ---: | ---: |
| `controller/task/mod.rs` | 776 | 226 | -550 |
| `lf/commands/flow.rs` | 1026 | 943 | -83 |
| `lf/commands/run.rs` | 1170 | 1218 | +48 |
| `ops/task_input.rs` | 0 | 228 | +228 |

Deleted fake-launcher scenarios are not counted as passing replacement proof.
The remaining-proof list above retains their behavioral obligations.


### Alternate-launcher test audit

The removed fake harness is not replacement evidence. Existing state proofs are
reused only for their own boundary:

| Removed scenario | Retained evidence / remaining command proof |
| --- | --- |
| Fresh slice turns until completion | Public multi-skill Flow cases and the managed Project-default launch. Managed repeated-decision traversal still needs a command-level replacement. |
| Selected original successful decision on recovery | `a_decision_belongs_to_the_selected_turn_and_recovery_reads_its_outcome`; original-turn recovery in the public native fixture. |
| Retry only after native death | Existing native engine-loss/retry fixtures; integrated managed explicit retry after an unresolved dead turn remains to execute. |
| Driver failure opens one unblock; feedback survives refusal | Retained keyed-unblock/store feedback checks; managed command failure → same unblock → reassessment still required. |
| Two empty decision passes | Historical policy-specific fixture removed; caller-supplied structured decision policy remains. No claim of a new configured policy proof. |
| Provider failure releases exact claim | Store claim fencing passes; managed provider-error command proof still required. |
| Pre-publication failure releases claim | Child reservation/admission and dead unpublished capture paths exist; managed command-level failure before publication still required. |
| Driver parks after releasing claim at review | Public managed controls/Chapter proof reaches review; retained `claimed_autonomous_boundary_settles_once_at_the_human_node`. |

These gaps keep item 1 open. They do not authorize another launcher or restoring
fake harness injection into production.
