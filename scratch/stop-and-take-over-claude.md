# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The outcome is accepted; the transport design below remains a draft (2026-10-10).
Source reconciliation: shared native-client launch (`b16be2b7a`, 2026-10-10)
and command dispatch (`773f244b3`), including exact native permission choices.
Claude correlation now uses saved request origins and reconstructs admitted,
unfinished turns from Session history; its pipes still belong to the launcher.
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
ordinary resume. Prompts, commands and abort are fenced through the complete socket write;
response headers and answer streaming are outside the fence. Repeated prompt IDs refuse replay. Native reads preserve queries
while pinning the saved working directory. Manual permission replies now share the saved-origin, no-replay writer with
headless recovery. The native reader leaves choices pending for its UI; stale,
foreign and repeated replies refuse. Native commands share saved message IDs and the dispatch fence. Shell mutations still refuse;
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
   Claude's in-memory request map is removed. Dispatch saves UUID/origin under
   the attachment fence before writing, with no invented native Session ID.
   The existing parser recovers origins on native echo and reconstructs admitted,
   unfinished turns in observation order for the same AgentProcess. Store/reader
   reopen tests cover loss before echo and before result, changed captured input,
   duplicate echo, foreign-process rejection and native-thread binding; they do
   not preserve an unread pipe or prove public takeover. Attention still uses the
   reader's frozen attachment. Transport extraction must accept only the current
   attachment for new writes; custody is not write authority.
   Source inspection at `b16be2b7a` also finds two launcher-local failure paths:
   `spawn_reader` emits failed completion on EOF, and `send_input` calls
   `kill_process` after a seed write error. Transport extraction must distinguish
   client disconnect, transport failure and confirmed provider death; neither
   client loss nor an uncertain write may kill/restart the surviving provider or
   settle its pending turn. Keep the existing parser, but separate its immutable
   request origins from current-attachment activity writes: `History::record`
   currently sends attention through its original attachment, which becomes stale
   after handoff. Refreshing that observer must never refresh an old caller's
   write authority. These are remaining transport changes, not source fixes here.
2. **Finish OpenCode shell correlation.** Command dispatch is implemented: one
   HTTP/1 connection is polled under the fence until the complete fixed-length
   request reaches the socket, then response collection continues outside it.
   Timeout drops the unspawned connection, so a partial write cannot finish after
   authority transfers. Native command IDs retain origins and refuse replay.
   ShellInput has no messageID field, not merely an omitted native-client argument;
   injecting one cannot correlate its native history. Shell remains required and
   explicitly refused until supported correlation preserves uncertain attempts
   without borrowing another turn. The response alone is insufficient after loss.
   Manual permission replies retain their existing receipt writer. Public
   process-death proofs remain in step 4, distinct from dispatch tests.
3. **Carry common custody through new public transports.** The anonymous
   lifelines, optional launch path, endpoint-derived FIFO and `HELD_LIFELINES`
   are deleted. Codex/OpenCode public connection acquires common custody before
   claim; Claude must use the same path when its transport exists. Keep pre-exec recording,
   closed-stdio safety, failed-claim release and both death orders. Codex harness
   reconnect consumes already-admitted authority; it is not the public claim path.
4. **Prove the public path.** Both death orders, AgentProcess identity and pending
   turns, A → B → A stale-token rejection, current stop, foreground exclusion,
   missing identity and no retained dead-provider lifelines. Include takeover before
   first native output and while a permission is pending, plus a client disconnect
   or lost write acknowledgement while the provider still runs. Verify pending
   history and current-owner attention separately from native-client exit. Linux CI
   owns platform acceptance. The demo attaches a second lf, SIGKILLs the launcher, checks `lf top`,
   then stops from the current attachment, for Claude and OpenCode.

Neither current transport gap proves takeover impossible or authorizes a refusal.
These are implementation choices, not missing human input or waived acceptance.
Removed already: Codex-only signaling, runtime live-close dispatch and Claude's
direct-child stop, OpenCode's direct-child/group shutdown and retrying abort.
The shared close and Codex unrelated-thread inspection survive.

## Delete — do not maintain

Removed: Claude's launcher-local request map and pre-fence origin capture;
Session history owns request intent before native identity and admission order.
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
The shared public attachment path owns custody, claims and client-only settlement.
Codex and OpenCode retain separate transport setup; `run_native_client` owns their
common frozen caller environment and native launch. Provider setup returns success
or failure, not an always-true connection flag; only live-provider discovery can
report no connection. This removes duplicate launch/attribution code without
changing either provider's transport or permission policy. OpenCode's discard-only
event-drain task is deleted: its native UI renders output while the reader retains
history and permission recovery. The native relay's response-header fence is
deleted: GET responses retain reqwest streaming; input/abort responses use Hyper
after fenced socket dispatch, preserving HTTP responses outside the fence.
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

The command fixture withholds headers through A → B → A, preserving exact
arguments and original request ownership while refusing repeats and stale writes.
Transport tests cover empty-body responses and cancellation during a partial
16 MiB write; they prove no launcher death or native shell correlation.
Review: a flush can precede a body, so dispatch counts actual header/body bytes;
response collection must not poll an already-completed connection again.

## Checks

Checks: `cargo check -p loopflow`, `cargo test -p loopflow --lib` filtered to Claude history (2) and OpenCode pending-request recovery (1), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` pass; public death orders unfinished, Linux acceptance CI-owned.
