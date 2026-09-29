# Execution

```bash
lf -b implement
lf session list --interactive false --json
lf session connect SESSION
```

These commands use the current parser. This guide specifies the accepted
lifecycle; [cutover status](../architecture-reference.md#cutover-status) records
which owners and proofs remain unfinished.

The command has an Exec. The agent has an AgentSession. A captured Flow has a
FlowSession. Their completion and authority are different facts.

## Request flow

```text
argv -> Exec admission -> Skill discovery -> prompt -> provider route
                                                        |
                                                        v
                                     AgentSession reservation + input capture
                                                        |
                                                        v
                                        native engine and conversation
                                                        |
                                     provider outcomes / retries / usage
                                                        |
                                                        v
                                           AgentSession history
```

The same admission applies to headless and interactive skills, inline prompts,
helpers, Asks and reviews. It needs no planning parents, but requires its Home's
writable conversation store before provider launch. Optional Work enrichment does
not confer a Flow claim. A failed admission cannot become an invisible file-only
conversation. Large captured payloads remain outside SQLite behind indexed references.

## Capture intent once

Discovery selects one repository override, builtin or installed Skill. Prompt
assembly captures the selected instructions, exact provider strings, attribution,
explicit documents and launch options. Definitions are not reconstructed from
current files when continuing historical work.

A Flow captures its expanded graph, all routing alternatives and every Skill
before execution. A direct conversation captures its selected Skill or inline
prompt. Reconnect retains that conversation; a new Flow invocation captures new
source. Current credentials and checkout contents remain live inputs to execution,
not evidence that old captured intent changed.

## Record actual processes

Exec admission records one actual lf process and immutable causal ancestry.
Nested wrappers reuse the process identity and cannot finish the outer command
early. Direct child commands name their invoking Exec. Agent-issued commands
record the incoming agent bit plus stable Session/provider-generation provenance,
then resolve the current matching driver once at admission.

The provider and shell do not become fake Execs. After driver handoff, new
commands from the continuing provider name the new driver; delayed commands from
a replaced provider retain their historical origin. A parent exiting never
rewrites existing descendants. These causal links grant neither signaling nor
Flow-settlement authority.

Exec completion records the observed command result, end time and known exit
code or signal. Missing terminal evidence stays unknown. Installation/bootstrap
commands must reach their exact-store authority checks before any logging-induced
store open; unavailable-store coverage is reported explicitly. An inspection
Exec does not reserve agent work or mark a Task Started.

## Publish before spawn

1. Resolve one Home for the store and payload root; validate typed ancestry.
2. Reserve the conversation, history/capture reference and exact driver together.
3. Publish immutable launch input and record publication before spawning.
4. Start or connect the native provider and retain exact engine/thread/client
   evidence, distinct from the conversation driver.
5. Append correlated provider outcomes and usage; settle selected Flow work under
   its separate claim, and command completion under its Exec lifetime.

Prepared rows without publication are recoverable preparation failures. A missing
spawn receipt is uncertainty, not permission to duplicate a possibly live engine.
Recovery preserves recorded inputs and exact native evidence. File publication
and SQLite settlement have an explicit recoverable boundary; neither alone is a
claim of successful execution.

## Connect and transfer the driver

Codex's private Unix WebSocket supports multiple clients on one active native
thread. Attachment alone does not revoke the old client's writes. Loopflow's
conversation-scoped relay therefore checks the Session driver at actual socket
dispatch, including queued requests and approval replies. A bounded native send
and driver transfer serialize through the same SQLite transaction. A send timeout
has an unknown outcome and is not retried automatically.

A passive viewer subscribes without claiming the Session. A former driver can
keep receiving events after transfer but cannot start or steer a turn or mutate
Session state. The continuing engine retains its provider generation while the
new driver receives a new driver generation. Engine ownership and driver ownership
must not be collapsed into one counter.

Explicit restart stops only the exact conversation owner, preserves its recorded
native identity and history, and excludes late writes from the replaced provider.
Graceful stop precedes force termination of an exclusively owned process. A shared
engine and sibling conversations survive a thread-specific restart. Mere process
silence, tmux visibility, causal ancestry or a stored active label grants no
termination authority.

## Outcomes, retries and usage

AgentSession history owns provider outcomes. Exec owns command completion. A
provider can succeed before the command fails later, and a parked Flow can outlive
a successful command. Failed or interrupted conversation work remains history;
continuation appends a new result to the same conversation.

A Flow step consumes one exact successful AgentSession completion under its
boundary/version/claim fence. Later conversation continuation does not rewrite
that consumed result. A mechanical step records its own start/result in Flow
history without inventing an AgentSession or child Exec. Recovery cannot infer
exactly-once external effects from cursor movement.

Usage keeps provider-authored stream/receipt identity. Reduce cumulative samples
once; never add checkpoints as independent consumption. Retries retain separate
outcomes and measurements. Missing counters and unknown finality stay missing.
The shared typed history reader selects captured events and unlinked native turns
before decoding payloads. Native receipts without a start stay discoverable with
unknown Exec/Work ownership and partial usage coverage. Recorder outcomes remain
separate from provider completion and Exec exit.
Under the supervisor's prospective-attribution assumption, binding affects later
work and preserves earlier usage ownership. Active-turn allocation uses recorded
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

Current Session contains-search, title/ID ordering, offset/limit and Desktop's
complete-inventory reconciliation are unchanged. Final paging and dense discovery
measurements remain separate work. Metadata selection still returns variable-size
recorded names, readiness text and iteration tuples and checks exact client receipts;
it is not a constant-byte or constant-time query guarantee.

Typed Task/Wave links survive landing and provider/driver replacement. Readers
never use live PR eligibility, path names or mutable manifests to recover identity.
Default interactive visibility does not hide headless history from explicit queries
or make it impossible to resume. Desktop and CLI consume the same fields.

Historical Run IDs, captures, sidecars, command journal rows and repeated outcomes
are one-time import inputs. Preserve unknown membership and original attribution;
do not infer an actual process from a provider-launch record alone. Ordinary reads
neither import nor fall back to those files. After verified conversion, the separate
Run product owner and its competing readers are removed.

[Planning](planning.md) owns captured Flow progression and planning ancestry.
[Data and persistence](data.md) distinguishes record ownership from payload storage.
