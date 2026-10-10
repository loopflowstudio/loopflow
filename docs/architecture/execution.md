# Execution

```bash
lf -b implement
lf session list --interactive false --json
lf session connect SESSION
```

These commands use the current parser. This guide specifies the accepted
lifecycle; [cutover status](../architecture-reference.md#cutover-status) records
which owners and proofs remain unfinished.

The command has a process. The agent has an LfSession. A Flow is one lf process and the step processes it starts. Their completion and authority are different
facts.

## Request flow

```text
argv -> Process admission -> Skill discovery -> prompt -> provider route
                                                        |
                                                        v
                                     LfSession reservation + input capture
                                                        |
                                                        v
                                      AgentProcess and native conversation
                                                        |
                                     provider outcomes / retries / usage
                                                        |
                                                        v
                                           LfSession history
```

The same admission applies to headless and interactive skills, inline prompts,
helpers and reviews. It needs no planning parents, but requires its Machine's
writable conversation store before provider launch. Optional Work enrichment
confers no Flow authority. A failed admission cannot become an invisible file-only
conversation. Large captured payloads remain outside SQLite behind indexed references.

## Capture intent once

Discovery selects one repository override, builtin or installed Skill. Prompt
assembly captures the selected instructions, exact provider strings, attribution,
explicit documents and launch options. Definitions are not reconstructed from
current files when continuing historical work.

A Flow process compiles its graph and all routing alternatives before the
first step and holds them in memory. A direct conversation captures its selected
Skill or inline prompt. Reconnect retains that conversation; a new Flow compiles
current source. Current credentials and checkout contents remain live inputs to execution,
not evidence that old captured intent changed.

## Record actual processes

Process admission records one actual lf process and immutable causal ancestry.
Nested wrappers reuse the process identity and cannot finish the outer command
early. Direct child commands name their invoking Process. Agent-issued commands
record the incoming agent bit plus stable Session/AgentProcess provenance,
then resolve the current attached LfProcess once at admission.

AgentProcess rows use the same `processes` inventory, with kind `agent`, exact
PID/birth, served Session, original parent and current attachment. Session rows
retain the current AgentProcess reference and native thread. Detached and replaced
rows survive until positive terminal evidence; attachment absence never removes
them from inventory. Shell helpers do not become fake lf invocations. After handoff, new
commands from the continuing provider name the newly attached LfProcess; delayed commands from
a replaced provider retain their historical origin. A parent exiting never
rewrites existing descendants. These causal links grant neither signaling nor
Flow authority.

Process completion records the observed command result, end time and known exit
code or signal. Missing terminal evidence stays unknown. Installation/bootstrap
commands must reach their exact-store authority checks before any logging-induced
store open; unavailable-store coverage is reported explicitly. An inspection
Process does not reserve agent work or mark a Task Started.

The source cut is incomplete: optional/unattributed launches still need conversion.
Captured Claude and native restarts reserve a fresh AgentProcess and update the
capture's attachment; pending operations retain their old snapshots. Owned native
launches record before exec without changing their terminal or process group.
Focused admission, replacement and cleanup fixtures pass. Top and Task gates
share unfinished-row selection and identity judgment. Missing lf receipts,
missing agent birth and unavailable OS samples remain Unknown rows with LfProcess IDs;
active Sessions retain their records while reporting unavailable sampling.
Zombies, birth mismatch and valid absence count as death. Observations grant no
signal or settlement authority. Public Task-status/scheduled agreement and
installed acceptance remain unproved.
Invocation-owned retry settles the exact old AgentProcess before reserving its
replacement, retaining native/account history. Account failover selects a fresh
thread; same-account retry retains it. Generic stop still preserves takeover.
Headless launch requires an attachment; caller tokens name AgentProcess identity,
not the retired provider generation.

## Publish before spawn

1. Resolve one Machine for the store and payload root; validate typed ancestry.
2. Reserve the conversation, history/capture reference and exact attachment together.
3. Publish immutable launch input and record publication before spawning.
4. Start or connect the native provider and retain exact AgentProcess/thread/client
   evidence, distinct from the attached LfProcess.
5. Append correlated provider outcomes and usage; settle command completion
   under its Process lifetime.

Headless Claude, Codex and OpenCode prepare their lifeline before spawning. The child
establishes its process group, waits for watchdog readiness, then asks a parent
thread to run its recording callback before exec. This persists OS identity in the AgentProcess row; headless launch
without an attachment is refused. The shared harness launch holds the attachment fence through recording
and saves failed spawns on the record. Admission is synchronous so async cancellation
cannot discard an admitted child before the harness receives it. Failed recording refuses exec;
a failed exec retains any recorded identity without claiming provider execution.
No post-spawn bind is needed. Every provider uses a private named FIFO keyed by
AgentProcess ID beside its store, independent of its communication endpoint.
Codex's public connection acquires custody before claiming the attachment;
a failed claim drops only its prospective writer. Successful holders remain
non-writing standbys through attachment transfer and harness teardown until lf
exit or confirmed provider-group death. The last writer's exit closes the lifeline and
stops the group, including after SIGKILL. A custody worker releases its descriptor
when the provider group dies or the watchdog closes; no process-global writer list remains.
The watchdog runs outside the provider group to avoid counting itself as a live
helper. Leader death alone never releases custody; unknown evidence retains it.
Custody never grants write authority.
Headless launch requires the invocation's attachment: Claude, Codex and OpenCode
refuse to start a provider without one. Runtime settlement closes their recorded
headless groups under that attachment fence, refusing ambiguous OS ownership;
Codex additionally refuses a server hosting unrelated conversations. Claude stop/
interrupt and OpenCode stop use common group close, recording death under the fence.
OpenCode abort uses bounded fenced HTTP without retries; drop never signals its
child. Failed startup retains the child for fenced cleanup and reports refusal.
Runtime settlement alone does not establish takeover safety. Public live connection
currently dispatches only to Codex; Claude and OpenCode still take native resume.
OpenCode's saved server URL is not sufficient for takeover: its request map is
launcher-local, and origin is saved only when native output is observed. Reconnect
must preserve pending request correlation as well as the server and native Session.

Owned native launches use the same pre-exec recording channel under the attachment
lock, but no headless group/watchdog setup. Failed recording prevents provider code
from running. Captured native waits retain the spawned attachment snapshot: a late
wait cannot mark a replacement exited. Native terminal process groups are unchanged;
foreground orphan cleanup remains unfinished.
Fresh native launches and saved-history resumes share invocation admission and
attachment settlement. A recorded client move is an intentional command exit,
not provider failure; it does not establish a successful provider turn.

Scheduled settlement reads unfinished AgentProcess rows, but live-orphan
termination covers noninteractive providers only: Codex app-server, OpenCode
serve and headless Claude groups. Native foreground coverage remains unfinished;
recording a foreground provider alone does not establish orphan cleanup. Unknown identities and
duplicate PID/birth records remain non-signallable. Failed OS observation makes
scheduled reconciliation fail with the affected LfProcess ID, without settling that row.
The reaper's identity, command and descendant observations use the shared OS
reader; failed descendant inventory refuses before signaling. Codex close and
scheduled termination share group-wide observation: unreaped zombies are dead,
but a live helper prevents settlement even after the leader exits.

Prepared rows without publication are recoverable preparation failures. Missing
spawn evidence is uncertainty, not permission to duplicate a possibly live AgentProcess.
Recovery preserves recorded inputs and exact native evidence. File publication
and SQLite settlement have an explicit recoverable boundary; neither alone is a
claim of successful execution.

Resume checks the attached LfProcess through the shared record/OS judgment.
One attachment lock spans observation, exact AgentProcess close and replacement;
SQLite is released during provider I/O. Detached does not imply dead. Resume and
invocation retry use the recorded provider/interactivity and refuse duplicate
live PID/birth ownership. Unknown spawn remains unresolved; never-launched
reservations can retire without claiming OS exit. Death and the new attachment
commit together, preserving native history and without inventing an outcome.
Native foreground and non-Codex live close still require their own lifecycle
coverage; this source path does not establish configured resume acceptance.

## Connect and transfer attachment

Codex's private Unix WebSocket supports multiple clients on one active native
thread. Attachment alone does not revoke the old client's writes. Loopflow's
conversation-scoped relay therefore checks the exact Session attachment at socket
dispatch, including queued requests and approval replies. A bounded native send
and attachment transfer hold the same per-Session OS lock, without holding a
SQLite transaction over transport I/O. A send timeout
has an unknown outcome and is not retried automatically.

Native launch carries the capture or connection owner's frozen attachment in
memory, separately from stable tool provenance. Admission validates that token;
it never reconstructs authority from the current record. The client connects to
the local relay, while the recorded upstream endpoint identifies the AgentProcess.
Both initial validation and actual spawn reject a replaced attachment, including
A → B → A. Connection exit or interruption records an attachment outcome without
closing or ending the surviving provider; orphan settlement remains independent.
Composed stand-ins cover launch and pre-launch takeover, not configured Codex or
a complete native-client relay exchange.

A passive viewer subscribes without claiming the Session. A formerly attached LfProcess can
keep receiving and retaining provider history after transfer but cannot start or
steer a turn or change current attachment, connection, process evidence or stream
attention. Retaining history grants no native-write or Flow authority.

Native request correlation freezes its initiating Process, Work and capture before sending, not when a delayed
start arrives. Claude input UUIDs, OpenCode message IDs and Codex request/reply
correlation carry that snapshot. Broadcast-only starts retain unknown attribution;
late correlated replies can fill it, but cannot borrow a newer capture or bind.
The snapshot supplies history only and never authorizes dispatch.

The continuing AgentProcess retains its identity while each attachment
receives a fresh opaque token, including reattachment of the same lf Process.
Release revokes that token as well. The token is a compare-and-swap witness,
not another process identity or lifecycle owner. Caller provenance, turn origin
and Program Status name the AgentProcess itself; no generation counter remains.

Attachment exit retains an exact history-event reference rather than deriving a
receipt from counter arithmetic. Migration preserves old event keys and payloads,
including captured input, native identity and historical outcomes.

`session connect --replace` stops the exact owned clients and reconnects to the
live AgentProcess, preserving the active turn and sibling conversations.
There is no separate Session AgentProcess-restart operation. Mere process silence,
tmux visibility, causal ancestry or a stored active label grants no termination
authority, and recovery never authorizes killing a shared AgentProcess for one thread.

## Outcomes, retries and usage

LfSession history owns provider outcomes. Process owns command completion. A
provider can succeed before the command fails later, and a stopped Flow's history
outlives its command. Failed or interrupted conversation work remains history;
continuation appends a new result to the same conversation.

The Flow process holds the cursor and starts each step as a child `lf` process: the
plain command, `lf -b skill <name> [message]` or the operation's own. A step
knows nothing of its Flow. The Flow process appends the step's Process, graph node and
iteration counts to FlowProcess, beside the Flow's name and launched graph. A
step's result is how its process exited. A deciding or routing step gets its
answer contract in its message and the Flow process reads the final answer of the
Session turn that step Process captured; an invalid answer is corrected by
resuming the same conversation (`lf -b session resume ID MESSAGE`), at most
twice, then the Flow fails. A mechanical step is its own child Process and invents
no LfSession. After an operation the Flow process stops the Flow when a landing of
its checkout is still being watched; neither failed. A killed Flow process leaves its Processes
as history; nothing resumes it. The caller inspects them before launching fresh
work. Unknown liveness stays unknown. Cursor movement cannot prove exactly-once
external effects.

Blocked records the reason and stops at the current Flow position. Existing logs
and outcomes provide the evidence. The Wave operator resolves
impediments or discusses missing judgment in its ongoing chat. Nothing retries
or resumes a stopped Flow.

Usage keeps provider-authored stream/receipt identity. Reduce cumulative samples
once; never add checkpoints as independent consumption. Retries retain separate
outcomes and measurements. Missing counters and unknown finality stay missing.
The shared typed history reader selects captured events and unlinked native turns
before decoding payloads. Native receipts without a start stay discoverable with
unknown Process/Work ownership and partial usage coverage. Recorder outcomes remain
separate from provider completion and Process exit.
Binding affects later work and preserves earlier usage ownership. The history
reader owns this single attribution choice. Active-turn allocation uses recorded
start/assignment evidence; uncertainty never becomes an invented token split.

## Read indexed history

Conversation, Flow and command summaries filter and page in SQL before opening
payloads. Exact detail can load the selected captured input, transcript or final
answer. A provider-neutral exact final-answer receipt and recovered streamed
prose remain distinguishable; incomplete extraction is labeled.

Session inventory reads the selected rows, Work labels and indexed Flow metadata;
it does not decode captures or Session history. Readiness text remains the exact
recorded Session field. Local active-client receipts may establish Active; without
that observation, explicit readiness or closure, the displayed state is Unknown.
It does not mean failed or unavailable for connection. Connect and completion
validate the exact capture and native history before acting. A recorded Flow with
no selected occurrence keeps its membership with an unknown position; missing
historical membership never becomes Independent. Unknown or corrupt detail remains
available for exact inspection instead of preventing unrelated rows from listing.

Session lists support contains-search and title/ID ordering; stable ID pages serve
Desktop's complete-inventory reconciliation. Names, readiness text and iteration
tuples vary in size, and listing checks exact client receipts. The query therefore
has no constant-byte or constant-time guarantee. Measurement status belongs in
the [cutover status](../architecture-reference.md#cutover-status).

Typed Task/Wave links survive landing and provider replacement and takeover. Readers
never use live PR eligibility, path names or mutable manifests to recover identity.
Default interactive visibility does not hide headless history from explicit queries
or make it impossible to resume. Desktop and CLI consume the same fields.

Current-state conversion and its preservation boundary belong in
[Data and persistence](data.md#reads-and-cutover). Runtime reads use the SQLite
owners, with no legacy import or file fallback.

[Planning](planning.md) owns Flow progression and planning ancestry.
[Data and persistence](data.md) distinguishes record ownership from payload storage.
