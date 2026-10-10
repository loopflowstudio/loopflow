# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The outcome is accepted; the transport design below remains a draft (2026-10-10).
Source reconciliation: `64b785022` (2026-10-10), including process-death creation
recovery at `07529ac3d`, shared prompt submission, cancellation at `cd9f78dd0`
and ordered readback at `196b57ac2`.
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
   `claude_history::History` also owns a frozen attachment and an in-memory
   request map/pending queue. Moving pipes alone leaves correlation and attention
   tied to the launcher. Transport extraction must preserve original turn origins
   while accepting only the current attachment for new writes; custody is not
   write authority.
2. **Finish OpenCode public recovery.** Harness startup now consumes the saved
   server URL/native Session and retains common custody instead of spawning again.
   It subscribes to SSE before initial message readback, recovering request origins
   without another edge or replay. New servers write stderr to a private per-AgentProcess
   file beside the FIFO; no launcher-local pipe/logger remains. Reader failure and
   harness drop detach without stopping a reused provider; pre-exec failures retain non-start evidence. Reconnect preserves native permissions
   without another setup mutation.
   Public `ops/human_session.rs::open` still dispatches live connection only for
   Codex. Replace that dispatch and preserve client-only settlement: harness reuse
   alone cannot satisfy the demo. Retain the implemented permission recovery above.
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
Removed in the protocol correction: unsupported permission PATCH and its
exclusive fixtures; saved native permissions survive on creation/reconnect.
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

Source reconciliation at `64b785022`: local main remains `be4a2b2af`;
LOO-450's `784b162e0` remains outside this branch. Preserve its unified seed
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

## Startup protocol correction (2026-10-10)

The earlier permission PATCH fixture accepted a mutation the pinned provider does
not support. OpenCode v1.2.0's [Session routes](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/server/routes/session.ts)
accept only title/time in PATCH; its [creation schema](https://github.com/anomalyco/opencode/blob/v1.2.0/packages/opencode/src/session/index.ts)
accepts permissions. The separate permission writer, attempt and permissive
PATCH fixtures are deleted. Creation now persists and sends the rules in its original payload;
reconnect preserves existing native rules. The same saved payload supplies an exact
correlation title for lost-response readback, without another schema or lifecycle.
Missing, renamed or ambiguous native evidence never authorizes replay.

The replaced cancellation/permission evidence and full previous plan are retained
at `894bc61e5:scratch/stop-and-take-over-claude.md`; those simulated PATCH successes
establish no provider support. The surviving fixture covers cancellation with a
live blocking writer and SIGKILL of a separate throwaway creation worker before
its response, plus lost/unapplied/ambiguous/renamed creation evidence. It reopens
SQLite, transfers ownership, preserves AgentProcess identity and rejects the stale
owner without creating another conversation. It does not exercise public `open`,
provider lifeline death orders or launcher-independent Claude transport.
Release's entry-point lesson still applies. Public transport work remains open.

## Checks

Prior source checks (unchanged code): network-isolated `cargo test --offline -p loopflow --lib harness::opencode -- --test-threads=1`: 14 passed, 2 configured-provider tests ignored; `--test session_lifecycle_tests headless_history_is_discoverable_without_entering_the_interactive_list -- --exact`: 1 passed; fmt, all-target Clippy and `git diff --check` passed. Realignment: `git diff --check` passed; no code retest. Public takeover remains implementation work; Linux acceptance belongs to CI.
