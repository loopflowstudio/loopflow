# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The outcome is accepted; the remaining Claude transport proposal is a draft.
Reconciled 2026-10-10 against `2c881348e`: atomic result correlation is
`2d018f7ae`; the latest cut simplifies OpenCode observation only. Dependencies
#1519/#1520 are integrated at `be4a2b2af`, satisfying Jack's steer `28e0c5cc`; #1521 is integrated through
`df5169ab9`. LOO-443's remaining item 3 was read at
`4f6ed76b2^:scratch/introduce-agentprocess-record-the-provider.md`.

## Outcome and preservation

The current attachment can stop its headless AgentProcess; superseded attachments
cannot. Takeover preserves AgentProcess identity, native history and outstanding
turns in both death orders: launcher first and attacher first. Foreground terminal
providers stay outside headless group control. Tests signal only their own
throwaway children; the lifeline shell stays dash-compatible (`kill -s TERM -- -pgid`).
Jack accepted the SIGKILL timing demonstration, three historical orphans and
configured-provider runs as out of scope. Stop-only work is not independently
shippable or Task completion. This plan grants no installation or delivery authority.

## Remaining implementation

1. **Replace launcher-owned Claude transport.** Anonymous stdin/stdout/stderr
   die with the launcher; saved origins cannot recover unread output. The draft
   proposes a per-AgentProcess transport process owning all three pipes and the
   existing stream/history reader, not another answer parser. Drain output without
   a client; publish a private Unix endpoint before takeover; validate frozen
   attachments at the transport owner before writes. Lost acknowledgements and
   transport death retain uncertain inputs without replay. This is unimplemented,
   not an accepted relay design.

   Result recovery cannot reconstruct a reader-local pending queue and replay
   results against it: two reconstructed readers can assign one result to two
   turns. The surviving history path now selects an admission and commits output,
   usage and completion atomically, deduplicating the native result UUID within
   its AgentProcess. Conflicting repeated payloads refuse without consuming the
   next turn. `record_ordered_session_result` returns without a receipt when no
   admission exists. Replaying that result after a later admission can therefore
   assign old output to new work. The transport must retain consumption order
   and uncorrelated observations, not replay the whole stream against current
   admissions. UUID deduplication protects already-correlated results only.
   Client completion projection must share that decision: the current launcher
   mapper still has its own counters and is not made replay-safe by this history
   repair. Replace that coupling with the transport, not a second result parser.

   The draft must preserve `dispatch.rs`'s bounded write fence: the transport
   owner validates and holds it through the provider write, not the caller while
   waiting for a second process to acquire the same fence. Responses and stream
   draining stay outside it. Transport survival must not create a permanent
   lifeline writer that prevents last-client-exit cleanup. These are design
   constraints, not implemented transport behavior.

   `ops/human_session.rs::open` reaches live dispatch before requiring native
   identity, but `connect_live_agent` still excludes Claude. Extend it without
   inventing an ID or launching a replacement before Claude's first native output.
   Use common custody-before-claim, retaining pre-exec recording, closed-stdio
   safety and failed-claim release. Codex harness reconnect consumes an already
   admitted attachment; it is not the public claim path.

   Remove the launcher-local failure paths in the same cut: `spawn_reader` emits
   failed completion after EOF **or a history-write error**; `send_input` calls
   `kill_process` after a seed write error. Client loss, uncertain writes and
   failed durable observations prove neither provider death nor turn completion.
   Interrupt recording while a throwaway provider survives and prove its pending
   turn remains uncertain. Preserve immutable request origins, current-owner
   display activity and frozen caller dispatch/stop authority as separate concerns.

2. **Finish OpenCode shell correlation.** Native commands already carry message
   IDs. Pinned v1.2.0 [ShellInput](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/session/prompt.ts)
   has no `messageID`, so injecting one cannot correlate native history. Shell
   remains required and explicitly refused until correlation preserves uncertain
   attempts without replay or borrowing a concurrent turn's origin. A response
   alone is insufficient after loss. Retain the existing permission-reply writer.

3. **Prove the public path.** For Claude and OpenCode: both death orders,
   AgentProcess identity and pending turns, A → B → A stale-token rejection,
   current stop, foreground exclusion, missing identity and no retained
   dead-provider lifelines. Include takeover before first native output, pending
   permissions, client disconnect and lost write acknowledgement while the provider
   survives. Check pending history and current-owner attention separately from
   native-client exit. Linux CI owns platform acceptance. The demo attaches a
   second lf, SIGKILLs the launcher, checks `lf top`, then stops from the current
   attachment. Rendered native permission UX remains review work.

Neither transport gap proves takeover impossible or authorizes a permanent
refusal. No missing input or waived acceptance blocks implementation.

## Delete — do not maintain

Remaining deletion targets:
- Claude's launcher-owned pipe transport and exclusive fixtures.
- `ClaudeHarness::spawn_reader`'s EOF/history-error synthetic completion and
  `send_input`'s teardown after uncertain writes.
- Claude's native-resume fallback for a live headless provider.

Preserve stream/history parsers, native identity, permissions, saved origins and
stale-write fences while replacing these paths in one cut. Do not polish the
predecessor transport.

Already deleted: Claude’s reader-local pending-result queue and non-atomic
result receipts (SQLite now owns result correlation); anonymous lifelines, optional headless launch, endpoint-derived
FIFO paths, `HELD_LIFELINES`, provider-specific signaling, creation retries,
permission PATCH, launcher-local request maps, SSE-only permission replies and
OpenCode's ignored `ApprovalPolicy` constructor argument. Claude History no longer
retains caller write authority. Exact predecessor/replacement evidence:
`4834c18f4c70d10315e768648e5a40b1cd2f2580:scratch/stop-and-take-over-claude.md`,
“Delete — do not maintain.”

OpenCode keeps one start-emission flag per recovered request: admission alone
does not prove its receipts saved or a start was emitted. Failed saves leave
that boundary unreported. Observation checks its owner once and extracts starts
in place before output/completion, without temporary partition lists.

The surviving owners are common group close/custody, `run_native_client`, saved
request/reply receipts and one ordered permission/message `Snapshot`. Creation
and bounded replies use `with_attached_http`; streamed prompt/command dispatch
ends its fence at the last socket byte. These are distinct boundaries, not
interchangeable HTTP helpers.

## Implemented boundary and evidence limits

### Shared custody and stop

Runtime settlement, Claude stop/interrupt and OpenCode stop share exact-identity
group close under the attachment fence, recording death before releasing it.
Unknown or duplicate identity refuses; Codex also refuses unrelated conversations
and rechecks identity after I/O. Leader death grants neither group settlement nor
authority to signal surviving helpers. Child handles drop only after common close.

All three providers use private AgentProcess-keyed FIFOs beside the store.
Public Codex/OpenCode attachment acquires custody before claiming; failure drops
only that prospective holder. Successful holders remain non-writing standbys until
lf exit, confirmed group death or watchdog closure. The watchdog's separate group
avoids deadlocking natural-exit cleanup by counting itself as a surviving helper.
Custody changes no write authority. Predecessor FIFO paths refuse handoff without
migration, history changes or signaling.

Fixtures cover current/stale settlement, foreground exclusion, helpers surviving
leader exit, both custody-holder death orders and natural-exit release. They do
not prove public provider takeover or Linux acceptance. Claude fixture trace IDs
and foreground defaults were repaired without schema or installed-data changes.

### Claude observation

`4834c18f4` saves UUID/origin under the dispatch fence before pipe writes, without
inventing native identity. Native echoes admit requests; SQLite selects unfinished turns in observation
order for the same AgentProcess. Fixtures cover
loss before echo/result, changed captured input, duplicate echo, foreign-process
rejection and native-thread binding; not unread pipes.

`ac0f659aa` follows only that AgentProcess's current attachment for display activity.
Tools survive detachment; replacement providers cannot overwrite current activity.
An attachment-token change bypasses the five-second unchanged-reading interval.
The surviving-reader fixture proves A → B → A display updates, stale dispatch/stop
refusal and replacement isolation, not public takeover or repaired failure tails.

### OpenCode connection and receipts

Startup saves the endpoint before spawn and creation intent before HTTP. Creation
retains original rules and a per-AgentProcess title. Exact-title native listing
recovers one identity after response loss; absent, ambiguous or renamed matches
remain uncertain without another create. Reconnect retains existing permissions
rather than applying replacement configuration. Creation-worker cancellation and
SIGKILL fixtures establish startup recovery, not public attachment death orders.

Harness reconnect reuses the saved server/native Session, subscribes to SSE before
ordered readback (permissions, then messages), and recovers saved request origins
without another wake edge. Empty snapshots cannot clear submitted turns. Ordinary
input and steering share `submit_prompt`; native observations alone supply
admission/completion. New servers own a private stderr file beside the FIFO.
Reader failure, startup failure and drop detach without signaling the provider;
pre-exec failure retains positive non-start evidence.

Public `open` shares custody-before-claim and client-only settlement with Codex.
OpenCode's native `attach` uses an authenticated loopback HTTP relay. Reads retain
queries and streaming while pinning the saved directory. Prompts, commands and
abort hold frozen authority through the complete request write, not response
headers or execution. Repeated message IDs refuse replay; partial-dispatch timeout
leaves no background writer. A flush can precede the body, so dispatch counts
actual bytes; response collection never polls a completed connection again.

Pending native permissions remain for the UI; headless recovery replies through
the same saved-origin writer. Both retain attempts before fenced HTTP. Lost replies
read back pending state without replay. Choices preserve rejection explanations;
stale, foreign and repeated native replies refuse. The reconstructed recovery
fixture needs neither a stream reader nor a prior native observation.

Relay fixtures withhold headers/answers through A → B → A and preserve arguments,
origin and delayed output while refusing stale/repeated writes. Transport fixtures
cover immediate empty-body responses and cancellation during a partial 16 MiB
write. The public stand-in fixture proves pending-identity recovery and client-only
exit. None establishes public death orders, native shell correlation or configured
provider behavior.

## Protocol and related work

October 10 protocol corrections and their previous rationale are retained at
`ac0f659aa:scratch/questions.md` and
`b10b6ff065:scratch/stop-and-take-over-claude.md`, “Startup protocol correction”
and “Public OpenCode transport.” OpenCode v1.2.0's
[Session routes](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/server/routes/session.ts)
accept permissions on creation, not PATCH. The
[native prompt](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/cli/cmd/tui/component/prompt/index.tsx)
uses the streaming message route; command/shell headers arrive after execution.
[Native attach](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/cli/cmd/tui/attach.ts)
receives relay authentication through private environment, never argv/debug output.
Manual replies use the pinned
[permission route](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/server/routes/permission.ts).

LOO-450's `784b162e0` remains outside this source boundary. Its plan at
`784b162e0:scratch/let-a-flow-read-a.md` routes headless Claude Flow steps through
ClaudeHarness. Preserve unified context/query writes, fenced native selection,
schema, skill-input and correction turns when integrated; replace its
teardown-on-write-error behavior with the uncertainty-preserving transport above.
Its non-null schema replay and Task-to-publication proof remain open. Its Flow
fixture proves no launcher-independent transport. #1521's checkout documents and
committed-default-branch Linear sync change no provider transport.

Detailed reviews and previous compression evidence remain at
`ac0f659aa:scratch/stop-and-take-over-claude.md` and its referenced history.
Release's entry-point lesson applies: harness fixtures cannot prove public handoff.

October 10 recovery review found repeated results could consume a later admission,
and a failure after output/usage could leave partial receipts. Focused history
fixtures now cover overlapping/reopened readers, changed repeats, missing identity
and rollback at completion. These are observation proofs only; launcher-owned
pipes, their failure tails and the public takeover path remain unchanged.

## Checks

Prior `2c881348e` checks: isolated `cargo test -p loopflow --lib harness::opencode` 19 pass, 2 configured checks excluded; build/fmt/Clippy pass. Realign: `git diff --check` passes (prose only); public death orders remain unfinished, Linux acceptance CI-owned.
