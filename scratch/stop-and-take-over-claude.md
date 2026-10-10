# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The outcome is accepted; the remaining Claude transport proposal is a draft.
Reconciled 2026-10-10 against `f43f18afa`: durable result receipts include
uncorrelated observations (`a82bc9da0`); OpenCode conformance now proves separate
request completions instead of idle-based coalescing. Dependencies
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

## Public Claude client mismatch (2026-10-10)

The pipe-owner draft is insufficient for the planned public native-client path.
`connect_live_agent` selects only Codex/OpenCode, and
`resume_session_with_env` has no Claude `NativeConnection` implementation.
An lf-owned Unix socket does not supply a protocol the native Claude client
can consume. A fake client accepting invented arguments would hide this gap.

Claude Code 2.1.296's `--help` and `attach --help` expose background-session
attachment. The [official agent-view documentation](https://code.claude.com/docs/en/agent-view#dispatch-an-agent-from-your-shell)
explicitly excludes `--print` from background launches. Its
[supervisor lifecycle](https://code.claude.com/docs/en/agent-view#the-supervisor-process)
can restart provider processes. This is not evidence that a native client can
attach to the existing stream-json provider. Adopting that supervisor without
redesign would bypass lf's pre-exec identity, last-holder cleanup and exact stop
ownership. Only version/help were run; no background session was launched.

Dependent Claude transport work stops at this design mismatch, not at a claim
that takeover is impossible. Before choosing the pipe-owner implementation,
establish a supported client bridge that preserves the same AgentProcess and
routes writes through the frozen attachment fence. If none is available, the
unresolved product choice is an lf-owned terminal client for the stream provider
versus redesign around Claude's native background lifecycle. Neither is approved
by the existing draft; `--resume` cannot substitute for live takeover. Stop and
OpenCode acceptance remain unchanged. OpenCode shell correlation and public
fixtures remain independent unfinished work, not evidence for Claude's path.

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
   next turn. `record_ordered_session_result` retains uncorrelated observations
   and returns the original receipt on replay, so early output cannot consume a
   later input. Transport still must preserve consumption order across previously
   unread output; deduplication cannot establish which admissions preceded it.
   Client completion projection must share that decision: the current launcher
   mapper still has its own counters and is not made replay-safe by this history
   repair. Replace that coupling with the transport, not a second result parser.
   The rate-limit branch also emits failed completion before History sees the
   line. Preserve account-limit reporting/routing, but reconcile its completion
   with durable turn evidence rather than retaining a second completion owner.

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

Release child memory's entry-point lesson still applies: common custody and
harness tests do not prove `session connect` reaches the surviving provider.
The public fixtures must cross claim, native-client launch and client exit, not
invoke the common holder directly.

Neither transport gap proves takeover impossible or authorizes a permanent
refusal. The public Claude client mismatch above supersedes the earlier claim
that no design choice blocks dependent transport implementation.

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

Removed OpenCode's ignored `live_send_current_coalesces_into_one_boundary` and
its exclusive completion-wait helper: idle-based coalescing contradicts native
request receipts. The deterministic conformance fixture now proves a failed
request leaves a queued request open, each completes under its own ID and repeated
snapshots emit neither completion again. The optional basic live smoke remains;
this does not prove public takeover or configured steering.

OpenCode keeps one start-emission flag per recovered request: admission alone
does not prove its receipts saved or a start was emitted. Failed saves leave
that boundary unreported. Observation checks its owner once and extracts starts
in place before output/completion, without temporary partition lists.

The surviving owners are common group close/custody, `run_native_client`, saved
request/reply receipts and one ordered permission/message `Snapshot`. Creation
and bounded replies use `with_attached_http`; streamed prompt/command dispatch
ends its fence at the last socket byte. These are distinct boundaries, not
interchangeable HTTP helpers.

## Surviving implementation and proof limits

[Execution architecture](../docs/architecture/execution.md#headless-custody-and-stop)
owns the implemented contract. Detailed fixture evidence is retained at
`668cd51817e01a9bd0b12ae8b350800214ef8f0a:scratch/stop-and-take-over-claude.md`,
“Implemented boundary and evidence limits.” The remaining work above consumes:

- **Common custody/stop:** private AgentProcess FIFOs, custody-before-claim for
  public Codex/OpenCode attachment, exact-identity group close and death recording
  under frozen authority. Holders survive teardown until lf exit, confirmed group
  death or watchdog closure; the watchdog is outside the provider group.
  Unknown/duplicate identity and unrelated Codex conversations refuse. Leader
  death cannot settle or signal surviving helpers. Predecessor FIFO paths refuse
  without migration. Throwaway fixtures prove holder death orders, natural-exit
  release and current/stale settlement, not public provider takeover.
- **Claude observation:** `4834c18f4` saves origins before pipe writes;
  `ac0f659aa` follows current display attachment without refreshing caller tokens.
  History fixtures cover lost/repeated echoes, changed captures, foreign providers,
  native-thread binding, detached tools and replacement isolation. Atomic-result
  fixtures cover concurrent/reopened readers, conflicting repeats, missing UUIDs,
  uncorrelated results and rollback at completion. These cannot recover unread pipes
  or repair launcher-local completion counters and failure tails.
- **OpenCode startup/readback:** saved endpoint/creation intent, exact-title recovery,
  retained rules and provider-owned stderr. Subscribe before permissions/messages
  readback; empty snapshots cannot idle submitted turns. Creation cancellation and
  SIGKILL fixtures prove no replay, including absent/ambiguous/renamed matches.
  Reader/startup failure and drop detach without signaling; failed pre-exec retains
  positive non-start evidence.
- **OpenCode relay:** frozen authority ends at the last socket byte, not flush,
  headers or execution. Responses never repoll a completed connection. Fixtures
  withhold headers/output across A → B → A and cover partial-write cancellation,
  immediate empty responses, exact arguments/origin and stale/repeated refusal.
  Native/manual permission replies share the saved-origin writer with headless
  recovery; lost replies read back without replay and preserve rejection text.
  The public stand-in proves pending-identity recovery and client-only exit, not
  public death orders, shell correlation or configured-provider behavior.

Foreground exclusion now precedes OS death inspection; the prior order allowed
headless close to settle an exited terminal provider. The regression covers live
and exited foreground children across all providers. Common stop also covers
A → B → A, rejecting both stale tokens before the current attachment closes.
These repairs change neither schema nor installed data; Linux acceptance remains
CI-owned.

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

## Checks

Prior source checks: `cargo test -p loopflow --lib --no-run`, `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` and `lf context --skill compress`: pass; network-isolated `cargo test -p loopflow --lib harness::conformance_tests::opencode`: 2 pass; realign `git diff --check` and `lf context --skill realign`: pass (prose only); public takeover remains unfinished, Linux acceptance CI-owned.
