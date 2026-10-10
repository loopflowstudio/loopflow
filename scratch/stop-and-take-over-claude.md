# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The outcome is accepted; the transport design below remains a draft (2026-10-10).
Source reconciliation: `9bef6bbbe` (2026-10-10), including shared native-client
launch, command dispatch, saved Claude origins and the unused OpenCode policy removal.
Claude correlation uses saved request origins and reconstructs admitted,
unfinished turns from Session history; its pipes still belong to the launcher.
October 10's observation slice removes the reader's stored write attachment:
activity follows only the same AgentProcess's current display attachment, while
caller dispatch/stop authority stays frozen. Tools survive detachment in the reader;
replacement-provider output cannot overwrite current activity. Repeated activity
is saved immediately after token changes, rather than suppressed by the five-second
save interval. The surviving-reader fixture proves A → B → A display updates,
stale dispatch/stop refusal, detachment and replacement isolation; not public takeover.
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
   stderr die with the launcher; saved correlation cannot recover unread output. A named watchdog FIFO
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
   not preserve an unread pipe or prove public takeover. The reader now stores
   immutable AgentProcess identity rather than caller write authority and follows
   the current display attachment only for activity. Transport extraction must
   accept only the current attachment for new writes; custody is not write authority.
   Source inspection at `b16be2b7a` also finds two launcher-local failure paths:
   `spawn_reader` emits failed completion on EOF, and `send_input` calls
   `kill_process` after a seed write error. Transport extraction must distinguish
   client disconnect, transport failure and confirmed provider death; neither
   client loss nor an uncertain write may kill/restart the surviving provider or
   settle its pending turn. The same reader tail also runs after a history-write
   error, not just EOF: a failed durable observation cannot become provider-death
   evidence. The transport proof must interrupt recording while a throwaway
   provider survives and retain the uncertain turn. Keep the existing parser, but
   preserve the implemented separation between immutable request origins,
   current-attachment display activity and frozen caller dispatch authority.
   The launcher-local EOF/history-error and uncertain-write teardown paths remain
   deletion targets, not repaired by the observation slice.
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

Removed in the observation slice: Claude History's retained AttachmentOwner.
The surviving reader retains AgentProcess identity, saved request origins and
current-owner display activity without providing callers a refreshed write token.

Remaining: launcher-owned Claude pipe transport and its exclusive fixtures;
Claude's native-resume fallback for a live headless provider. Preserve the existing
stream/history parsers, exact request origins, native history, permissions and
stale-write fences while replacing those paths in one cut.

Deleted predecessors and their exact replacement evidence are retained at
`4834c18f4c70d10315e768648e5a40b1cd2f2580:scratch/stop-and-take-over-claude.md`,
this heading. Do not restore anonymous lifelines, `HELD_LIFELINES`, provider-specific
signaling, creation retries, permission PATCH, launcher-local request maps, or
SSE-only permission replies.

The surviving owners are common group close/custody, `run_native_client`, saved
request/reply receipts, and one ordered permission/message Snapshot. Preserve
output-before-reply ordering and exact native choices, including rejection text;
the predecessor incorrectly saved `once` for every choice. Creation and bounded
reply acknowledgements use `with_attached_http`; streaming command/prompt dispatch
ends its fence at the last socket byte. These transports must stay distinct.

OpenCode's ignored constructor `ApprovalPolicy` argument is removed with its
callers. Creation owns native rules; the headless reader replies through saved
receipts, while the native client owns manual choices. Removing the unused argument
changes neither policy nor permissions. Earlier snapshot, cancellation and reply
proofs remain applicable; none proves public takeover.

## Related work and review

LOO-450's local `5ce7cd5e4`, simplified by `4c79cccf7`, routes headless Claude Flow steps through ClaudeHarness
and saves native Session identity under the fence. It is not in this branch or
local main. Reuse that history path when integrated; preserve schema, skill-input
and correction turns. Its Flow fixture proves no launcher-independent transport.

At `07dc2e34f`, main `df5169ab9` is integrated; #1521 makes checkout files
own Wave documents and pins Linear sync to committed local default-branch bytes.
It changes no provider transport. LOO-450's `784b162e0` remains outside this branch.
Its plan was reread at
`784b162e0:scratch/let-a-flow-read-a.md`: preserve unified context/query writes
and fenced native selection, but replace its teardown-on-write-error behavior
with the uncertainty-preserving transport boundary above. Its non-null
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

`cargo check -p loopflow`, network-isolated lib filters `harness::claude_history::tests` (3) and `harness::attention::tests` (9), `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`: pass; public death orders unfinished, Linux acceptance CI-owned.
