# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The outcome is accepted; the transport design below remains a draft (2026-10-10).
Source reconciliation: `1ce7d0119` (2026-10-10), including native permission
choices at `157a29596`, exact reply receipts and shared mutation decoding.
Local main remains `df5169ab9` (#1521). Creation recovery, shared prompt
submission and ordered readback remain implemented. Earlier iteration feedback's
manual-permission implementation item is satisfied; rendered UX and public
death-order proofs are not.
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

### OpenCode public connection (implemented)

Harness startup now consumes the saved
server URL/native Session and retains common custody instead of spawning again.
It subscribes to SSE before initial message readback, recovering request origins
without another edge or replay. New servers write stderr to a private per-AgentProcess
file beside the FIFO; no launcher-local pipe/logger remains. Reader failure and
harness drop detach without stopping a reused provider; pre-exec failures retain non-start evidence. Reconnect preserves native permissions
without another setup mutation.
Public `ops/human_session.rs::open` now dispatches Codex and OpenCode through
the same custody-before-claim and client-only settlement path. OpenCode reuses
the existing harness reader and native `attach` through an authenticated local
HTTP relay. Pending native identity reaches saved-creation readback before
ordinary resume. Prompts and abort are fenced; answer streaming is outside
the fence. Repeated prompt IDs refuse replay. Native reads preserve queries
while pinning the saved working directory. Manual permission replies now share the saved-origin, no-replay writer with
headless recovery. The native reader leaves choices pending for its UI; stale,
foreign and repeated replies refuse. Command/shell mutations still refuse;
rendered native permission UX is unproved.
The public stand-in fixture covers identity recovery and client-only exit,
not process-death orders. Retain the implemented permission recovery above.
Startup saves the endpoint before spawn and creation intent before HTTP.
The October 10 protocol correction below removes separate permission setup:
creation saves/sends the original rules together with a per-AgentProcess title.
A lost response is recovered by exact-title native listing; zero or multiple
matches retain uncertainty without another create. Native title changes before
identity recovery remain unresolved, not permission to choose a nearby Session.
Attachment transfer retains the same AgentProcess and saved payload despite
changed configuration. Existing native conversations retain their permissions.
The focused fixture kills a throwaway creation worker after HTTP acceptance,
before response/identity persistence, reopens SQLite and recovers on takeover.
This is process-death startup recovery, not public attachment or both provider
death orders. Preserve frozen authority for prompts, replies, abort and stop.

## Remaining implementation

1. **Replace launcher-owned-only Claude transport.** Its anonymous stdin/stdout/
   stderr and pending correlation die with the launcher; a named watchdog FIFO
   cannot preserve communication. Give the existing stream reader a per-AgentProcess
   transport process owning all three pipes and native-history correlation, rather
   than adding another answer parser. Drain output without a client. Publish a
   private Unix endpoint before takeover; validate frozen attachments at this owner
   before writes. Lost responses or transport death retain uncertain inputs, never
   replay them. This proposal is unimplemented, not an accepted relay design.
   Public `open` now reaches live dispatch before requiring native identity,
   supporting OpenCode creation readback. `connect_live_agent` still excludes
   Claude. Its extension must preserve pending identity before Claude's first
   native output, without launching a replacement or inventing an ID.
   `claude_history::History` also owns a frozen attachment and an in-memory
   request map/pending queue. Moving pipes alone leaves correlation and attention
   tied to the launcher. Transport extraction must preserve original turn origins
   while accepting only the current attachment for new writes; custody is not
   write authority.
2. **Revise OpenCode command/shell dispatch before implementing it.** The pinned
   v1.2.0 routes await execution before sending headers, unlike message/prompt_async.
   The current relay fence would block takeover/stop until execution or timeout;
   merely admitting these routes is unsafe. Separate proven dispatch from response
   collection, retaining frozen authority and no replay. Native shell also omits
   messageID; its correlation needs a supported solution. Manual permission replies
   are implemented through the existing receipt writer, and native readers no longer
   auto-approve. Command/shell remain required, not accepted exclusions. Public
   process-death proofs remain in step 4, distinct from creation-worker death.
3. **Carry common custody through new public transports.** The anonymous
   lifelines, optional launch path, endpoint-derived FIFO and `HELD_LIFELINES`
   are deleted. Codex/OpenCode public connection acquires common custody before
   claim; Claude must use the same path when its transport exists. Keep pre-exec recording,
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
Claude's native-resume fallback for a live headless provider. Codex-only public
connection dispatch is removed; native remote endpoints now carry strings
rather than misrepresenting OpenCode HTTP addresses as filesystem paths. Preserve the existing stream/history
parsers, exact request origins, native history, permissions and stale-write fences.
Removed: OpenCode SSE-only permission replies, mapping-side reply requests and
their now-single-field `MappedEvent` wrapper;
OpenCode creation retry (`send_request_with_retry`), unconditional
reconnect spawn and launcher-local stderr logger;
anonymous/optional lifelines, endpoint-derived FIFO location,
`HELD_LIFELINES`, and claim-before-custody in public Codex connection;
`open_agent_session`'s unfenced startup HTTP and its helper-only creation fixtures,
replaced by persisted startup attempts, readback and attachment-transfer proofs.
Removed in the protocol correction: unsupported permission PATCH and its
exclusive fixtures; saved native permissions survive on creation/reconnect.
The shared public attachment path owns custody, claims and client-only settlement;
Codex and OpenCode transports have separate client functions. OpenCode's discard-only
event-drain task is deleted: its native UI renders output while the reader retains
history and permission recovery. GET and prompt/abort responses share one streaming
translation, preserving status/content type and leaving answers outside the fence.
Permission replies instead return a boolean after the receipt writer confirms
HTTP success or pending-list readback; they do not forward the upstream body.
The native permission path reuses the existing reply writer. Its retained response
is now the exact HTTP payload, including rejection explanations: review found
the predecessor receipt hardcoded `once` even for native rejection. Repeated
choices preserve the first intent and never replay it. Relay mutations share
one bounded body decoder; permission and prompt dispatch remain separate. Automatic replies are removed from native
attachment readers; headless readers retain recovery.
Duplicated OpenCode blocking-worker/fence/client setup is replaced by
`with_attached_http`; creation, replies, prompts and abort retain
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

At `07dc2e34f`, main `df5169ab9` is integrated; #1521 makes checkout files
own Wave documents and pins Linear sync to committed local default-branch bytes.
It changes no provider transport. LOO-450's `784b162e0` remains outside this branch. Preserve its unified seed
write/error path and native selection when extracting transport. Its non-null
schema replay and Task-to-publication proof remain open.

Detailed reviews and compression history are retained at
`07529ac3db346196031f26115a9ec63768dcdaae:scratch/stop-and-take-over-claude.md`.
The surviving constraints are reflected above: record birth before exec, acquire
custody before claim, retain immutable request origins and uncertain HTTP attempts,
and never signal on reader failure or drop. An empty snapshot cannot clear a
submitted turn. Existing providers with predecessor FIFO paths refuse handoff
without migration, history changes or signaling. Release's entry-point lesson
still applies: harness fixtures cannot prove public handoff. Release is the only
immediate child Wave; its complete goal and memory were read on October 10.

Ordinary input and steering share `submit_prompt`: save one request identity,
then enqueue it through fenced HTTP. Native observations alone supply admission
and completion. The obsolete reconnect fixture route for the deleted
`open_agent_session` detail GET is removed; creation readback and pending
permissions keep their surviving fixtures.

## Provider protocol evidence (2026-10-10)

OpenCode v1.2.0's [Session routes](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/server/routes/session.ts)
accept title/time in PATCH, permissions only on creation. The pinned
[native prompt](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/cli/cmd/tui/component/prompt/index.tsx)
uses the streaming message route; [native attach](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/cli/cmd/tui/attach.ts)
receives relay authentication through private environment, never argv/debug output.
These contracts explain the saved creation payload and dispatch-only fence above.

Creation-worker cancellation/SIGKILL fixtures cover applied, unapplied, ambiguous
and renamed readback without replay. The relay fixture withholds an answer across
A → B → A, rejects stale writes, preserves attribution, then drains that answer.
Manual permission fixtures retain a rejection's explanation and reject foreign,
repeated and stale choices; reconnect covers headless automatic recovery and
native pending-choice preservation. The native reply uses the pinned
[permission route](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/server/routes/permission.ts).
They establish neither public process-death orders nor configured-provider behavior.
Full protocol correction and replaced PATCH evidence:
`b10b6ff065:scratch/stop-and-take-over-claude.md`, “Startup protocol correction”
and “Public OpenCode transport.”

## Checks

Checks: `git diff --check` passes (prose-only reconciliation); `1ce7d0119`'s recorded build/fmt/Clippy and focused permission/recovery/prompt-stream tests remain applicable, not rerun; public death-order acceptance remains unfinished and Linux acceptance CI-owned.
