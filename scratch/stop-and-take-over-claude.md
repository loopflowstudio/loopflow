# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The outcome is accepted; the transport design below remains a draft (2026-10-10).
Source reconciliation: `196b57ac2` (2026-10-10), including startup preservation
at `3806dd4da`, cancellation proof at `cd9f78dd0` and shared ordered readback.
Dependencies #1519/#1520 are integrated at `be4a2b2af`, satisfying Jack Heart's
steer `28e0c5cc`. LOO-443's remaining item 3 was read at
`4f6ed76b2^:scratch/introduce-agentprocess-record-the-provider.md`.

## Outcome and preservation

The current attachment can stop its headless AgentProcess; superseded attachments
cannot. Takeover preserves AgentProcess identity, native history and outstanding
turns in both death orders: launcher first and attacher first. Foreground terminal
providers remain outside headless group control. Tests signal only their own
throwaway children; the lifeline shell stays dash-compatible (`kill -s TERM -- -pgid`).
Jack accepted the SIGKILL timing demonstration, three historical orphans and
configured-provider runs as out of scope. No installed-store writes or delivery
are authorized by this reconciliation. Stop-only work is not independently
shippable or Task completion.

## Implemented boundary

Runtime settlement, Claude stop/interrupt and OpenCode stop share exact-identity
group close under the attachment fence, recording death before releasing it. Unknown identity
and duplicate ownership refuse; Codex also refuses unrelated conversations.
Identity is rechecked after Codex I/O. Leader death alone grants neither group
settlement nor authority to signal surviving helpers. Both harnesses drop their
child handles only after common close succeeds.
OpenCode prompt, abort and stop share history's frozen attachment; config changes
cannot refresh control authority. Abort uses the bounded fenced HTTP writer without retries. Failed startup
detaches without erasing admitted providers or uncertain native effects.
Dropping OpenCode no longer signals its child. All three providers now use a
named FIFO keyed by AgentProcess identity in the store's private namespace.
Codex public attachment acquires custody before claiming; failure drops only
that prospective holder. Successful holders remain non-writing standbys until
lf exit, confirmed group death or watchdog closure, without `HELD_LIFELINES`.
The watchdog has its own group: counting it as a provider helper would deadlock
EOF cleanup after natural exit. Leader death alone never releases surviving
helpers. Custody observation sends no signal and changes no attachment authority.

OpenCode permission recovery is implemented (`3441b0ea4`, simplified by
`62b6c27df`): startup and wake edges acquire pending permissions, then native
messages. Frozen ownership and saved request origins select replies; Session
observations retain attempts before fenced HTTP. Lost responses trigger pending
readback, never replay. The reconstructed fixture needs no stream reader or prior
native observation; it covers both applied and unapplied replies, not public
attachment or process death.

Fixtures cover current/stale settlement and foreground exclusion for all providers,
Claude harness stop fencing, and helpers surviving leader exit. They prove neither
public takeover nor Linux acceptance. Invalid trace IDs and foreground defaults
in Claude fixtures were repaired; production schema and installed data are unchanged.

## Remaining implementation

1. **Replace launcher-owned-only Claude transport.** Its anonymous stdin/stdout/
   stderr and pending correlation die with the launcher; a named watchdog FIFO
   cannot preserve communication. Give the existing stream reader a per-AgentProcess
   transport process owning all three pipes and native-history correlation, rather
   than adding another answer parser. Drain output without a client. Publish a
   private Unix endpoint before takeover; validate frozen attachments at this owner
   before writes. Lost responses or transport death retain uncertain inputs, never
   replay them. This proposal is unimplemented, not an accepted relay design.
   Public `open` currently requires saved native identity before reaching live
   connection dispatch. Publishing a transport endpoint alone cannot cover takeover
   before Claude's first native output: admission and connection must preserve the
   pending native-identity state without launching a replacement or inventing an ID.
2. **Finish OpenCode public recovery.** Harness startup now consumes the saved
   server URL/native Session and retains common custody instead of spawning again.
   It subscribes to SSE before initial message readback, recovering request origins
   without another edge or replay. New servers write stderr to a private per-AgentProcess
   file beside the FIFO; no launcher-local pipe/logger remains. Reader failure and
   harness drop detach without stopping a reused provider; pre-exec failures retain non-start evidence. Startup reads back the saved permission rules; it may finish previously
   unattempted setup, but never repeats an uncertain write or adopts changed configuration.
   Public `ops/human_session.rs::open` still dispatches live connection only for
   Codex. Replace that dispatch and preserve client-only settlement: harness reuse
   alone cannot satisfy the demo. Retain the implemented permission recovery above.
   Startup now saves the endpoint before spawn/readiness and records creation
   attempts in existing Session observations before fenced HTTP. Returned native
   identity commits before permission setup. One bounded blocking writer retains
   the fence through caller cancellation; attempts are keyed by AgentProcess,
   not attachment. Reconnect with an uncertain creation never creates another
   native Session. Permission setup retains its original rules and attempt;
   readback can settle an applied write, but an unresolved write never replays.
   Failed startup with saved reachability detaches without erasing the endpoint.
   Loopback fixtures reopen SQLite and transfer attachment after lost creation
   and applied/unapplied permission responses, including changed configuration
   and stale-writer rejection. These are startup-owner proofs, not public
   SIGKILL/takeover or actual process-death proofs. Public interruption before native identity remains open. The
   cancellation fixture now pauses creation and permission responses after
   acceptance, aborts the caller and races transfer through a separately opened
   SQLite store. Transfer waits for response/receipt settlement; reopening and
   reconnecting retain identity and original permissions without replay.
   A lost creation response still has no recoverable native ID: retaining its
   attempt prevents duplication but does not yet provide a usable public takeover.
   Keep frozen ownership for recovered prompts, permission replies, abort and stop.
   Include public recovery after HTTP acceptance but before first observation,
   and both launcher/attacher death orders. Never resubmit uncertain input.
3. **Carry common custody through new public transports.** The anonymous
   lifelines, optional launch path, endpoint-derived FIFO and `HELD_LIFELINES`
   are deleted. Claude/OpenCode connection must acquire the existing common
   custody before claim, like public Codex connection. Keep pre-exec recording,
   closed-stdio safety, failed-claim release and both death orders. Codex harness
   reconnect consumes already-admitted authority; it is not the public claim path.
4. **Prove the public path.** Both death orders, AgentProcess identity and pending
   turns, A → B → A stale-token rejection, current stop, foreground exclusion,
   missing identity and no retained dead-provider lifelines. Include takeover before
   first native output and while a permission is pending. Linux CI owns platform
   acceptance. The demo attaches a second lf, SIGKILLs the launcher, checks `lf top`,
   then stops from the current attachment, for Claude and OpenCode.

Neither current transport gap proves takeover impossible or authorizes a refusal.
These are implementation choices, not missing human input or waived acceptance.
Removed already: Codex-only signaling, runtime live-close dispatch and Claude's
direct-child stop, OpenCode's direct-child/group shutdown and retrying abort.
The shared close and Codex unrelated-thread inspection survive.

## Delete — do not maintain

Remaining: launcher-owned Claude pipe transport and its exclusive fixtures;
Codex-only public connection dispatch. Preserve the existing stream/history
parsers, exact request origins, native history, permissions and stale-write fences.
Removed: OpenCode SSE-only permission replies, mapping-side reply requests and
their now-single-field `MappedEvent` wrapper;
OpenCode creation retry (`send_request_with_retry`), unconditional
reconnect spawn and launcher-local stderr logger;
anonymous/optional lifelines, endpoint-derived FIFO location,
`HELD_LIFELINES`, and claim-before-custody in public Codex connection;
`open_agent_session`'s unfenced startup HTTP and its helper-only creation fixtures,
replaced by persisted startup attempts, readback and attachment-transfer proofs.
Duplicated OpenCode blocking-worker/fence/client setup is replaced by
`with_attached_http`; creation, configuration, replies, prompts and abort retain
one timeout and cancellation boundary. Native receipt grouping borrows messages
from the readback snapshot instead of cloning their full JSON on every wake.
Output and permission recovery now share one ordered Snapshot (permissions,
then messages), deleting the second message acquisition per pending reply batch.
The reconnect fixture publishes an assistant during permission acquisition and
requires that same startup read to recover its pending turn without an SSE edge.
This preserves output-before-reply handling and frozen, no-replay writes; it
proves no public takeover.

## Related work and review

LOO-450's local `5ce7cd5e4`, simplified by `4c79cccf7`, routes headless Claude Flow steps through ClaudeHarness
and saves native Session identity under the fence. It is not in this branch or
local main. Reuse that history path when integrated; preserve schema, skill-input
and correction turns. Its Flow fixture proves no launcher-independent transport.

Source reconciliation (2026-10-10, rechecked at `196b57ac2`): local main remains `be4a2b2af`;
LOO-450's reconciliation `784b162e0` is still outside this branch. Its plan retains
non-null schema replay and Task-to-publication proof as open; its unified seed
write/error path and native Session selection must survive transport extraction.
`b5b089978` completes the earlier OpenCode
stop/abort/drop step using frozen history ownership, not mutable launch config.
Release is the only immediate child Wave; its complete goal and memory were read.
Its entry-point lesson applies: harness close tests cannot establish public handoff.
Connection reuse now recovers native messages after subscribing.
Launcher-independent Claude transport and public OpenCode recovery remain
implementation work. Common custody is implemented below that public boundary.
Earlier review details: `5ea5cf5d6:scratch/stop-and-take-over-claude.md`.

Review (2026-10-10): starting the custody worker before provider spawn/attachment
claim keeps thread-creation failure before effects. Birth is captured before exec,
not sampled after a fast child exits. Public Codex's fixture now uses the real
headless launcher rather than a native child lacking custody. Existing live
providers with the predecessor's FIFO location refuse handoff without changing
history or stopping them; this source change does not migrate running providers.

The OpenCode regression enters real harness startup with a throwaway stand-in,
cancels after admission, transfers the attachment, rejects stale stop/abort even
after config changes, drops the old harness without killing the provider, and closes from the current
attachment without a local child handle. This is not a public takeover proof.

Compression (2026-10-10): the shared pre-exec recorder now samples birth once
and supplies that identity to both custody and SQLite; the second OS lookup is
deleted. Lifeline fixtures require a named FIFO instead of keeping the removed
optional-path shape. Remaining transport deletion targets above are unchanged.

Compression: OpenCode recovers each immutable request origin once per reader,
not on every native observation. The same map retains emitted boundaries; intent
creation no longer claims mutable reader state. SQLite decodes the saved origin
directly, without an intermediate JSON tree. Native receipts still advance on
every observation, including uncorrelated requests; public transport work above
is unchanged. At `74347955c` recovery remained only in `History::observe`;
`c8b9ca782` adds startup readback through the same reader. Public connection is still missing.

Review (2026-10-10): request intent uses existing immutable Session observations,
not Started, a new table or a second store. A reconstructed reader retains the
original capture and LfProcess after input replacement and attachment transfer;
already completed requests do not emit another completion. The focused fixture
reopens SQLite before any native observation; it is not an HTTP/SIGKILL or public
handoff proof. No configured provider is used.

Review (2026-10-10): failed reader setup must not call provider stop. The
initial new-child/reused-connection cleanup split is superseded by startup
preservation below; drop aborts only the reader. Message readback sets busy on recovered native Start,
not on empty snapshots: an empty early read must not clear an in-flight submission.
The harness fixture transfers attachment, drops the launcher harness, observes
later stderr output, and recovers an unobserved pending request without an SSE
message. This is not a second-process SIGKILL or public takeover proof.

Compression review (2026-10-10): deleted the generic HTTP retry helper, whose
only remaining caller created native Sessions. Creation errors now return without
repeating a potentially successful write; the loopback fixture counts one native
creation for both server failure and missing response identity. Readback selects
admitted messages under one history lock and emits start → output → completion
without cloning start events or repeatedly filtering lifecycle events. This does
not finish public takeover or recover an unknown creation identity.


Permission recovery (2026-10-10): the existing history owner acquires pending
permissions, then reads native messages so a permission appearing during the earlier
message snapshot still resolves its origin. Attempts live in existing observed
Session events, not another schema or in-memory dedup map. The attachment fence
covers intent, HTTP and uncertain-response readback. A pending previously attempted
reply surfaces uncertainty; absence means no pending action, not proof of approval.
The harness startup fixture recovers a pending permission without an SSE edge;
the HTTP fixture loses both applied and unapplied responses, reopens SQLite and
transfers attachment without repeating the reply. Public takeover remains open.
Protocol reference: OpenCode v1.2.0's
[permission routes](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/server/routes/permission.ts).

Compression (2026-10-10): permission recovery takes the frozen attachment owner,
not a mutex-protected stream reader. The fixture now reopens SQLite and replies
without first observing native messages, proving saved intent supplies attribution
and no-replay evidence. Message projection borrows the shared snapshot instead of cloning it;
SSE mapping returns events directly after deletion of its permission side channel.
Public takeover and the remaining deletion targets are unchanged.

## Startup review (2026-10-10)

Endpoint persistence precedes provider spawn, making the earlier failed-startup
stop branch unreachable for an admitted child; it is deleted. Readback needs the
originally attempted permissions, not a new attachment's
mutable configuration; the persisted attempt now retains those rules. Existing
Session observations own uncertainty without a schema or another lifecycle.
The shared HTTP worker keeps the attachment fence through the operation and its
receipts, even if the async caller detaches. Store and transport errors retain
their original types rather than being reclassified as invalid stored data.
Caller-cancellation during creation and permission HTTP is covered in step 2;
public interruption remains unproved. `cd9f78dd0` aborts the async caller while
its blocking HTTP worker survives; it does not kill the worker's LfProcess.
SIGKILL can lose the response and its receipt together. The saved attempt prevents
replay but cannot recover an unknown native identity. The public death-order
fixtures must cross this boundary, not reuse async cancellation as its proof.
OpenCode v1.2.0's [Session schema and readback](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/session/index.ts)
include saved permission rules. The disposable HTTP provider now retains them.
Release's operation-entry lesson still applies: these fixtures do not prove
public `open`, launcher-independent Claude transport or either death order.

## Checks

Recorded at `196b57ac2`: network-isolated `cargo test --offline -p loopflow --lib harness::opencode -- --test-threads=1` passed (14 passed, 2 configured-provider tests ignored), fmt/Clippy passed; realign: `git diff --check` passed (prose only, no suite rerun); public takeover remains implementation work, Linux acceptance belongs to CI.
