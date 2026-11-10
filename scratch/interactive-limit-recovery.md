# Open Task Sessions after a usage limit

November 9, 2026, demo conversation. Jack Heart:

> i have still had the issue that if i have a bunch of task sessions open and they all hit a usage limit i need to restart them all. can we automatically do soething or at least add something in hte desktop so you dont need to go to an obscure cli command

The reported outcome remains unsatisfied. No demonstration or approval of
LOO-811 occurred. During the first demo preparation its candidate binary was
absent and installed lf reported `0.15.2+7e90dafb1.dirty`. A later demo request
started a local candidate build; no installation, account switch or Session
restart was performed.

The first handoff went to LOO-813. Jack Heart then corrected the placement:
"813 was marked complete so move that" and "to 811". LOO-811 now owns this recovery outcome. Leave LOO-813 complete; its merged
PR #1863 is prior work, not completion of automatic quota recovery. PR #1862's
two-item Keychain repair is only part of LOO-811's outcome.

Jack Heart's November 9 continuation direction:

> Automatically continue affected open Sessions on another eligible login for the selected model; expose Recover affected sessions in Desktop with progress and waiting states. Preserve native history, drafts and pending approvals. Distinguish rejected turns from uncertain delivery/completed effects and never replay finished Graph steps. Account switching alone is not acceptance. Inspect Claude and Codex; the report's provider is unspecified.

The original provider remains unidentified; both providers are in scope. The
initial brief/title sync timed out; a later November 9 status read returned the
expanded title/direction and open PR #1862, without delivery status. The earlier
local candidate build (`cargo build -p loopflow --bin lf`, inherited LF_* unset)
passed in 1m 45s. No disposable demo was launched.

## Recovery design — November 9, implemented; verification remains

### Current direction — after the stopped investigation

Jack Heart:

> ok figure out ow to make progress

> do we need another strategy? it is ok if we need to restart the codex servers to switch
> or look online for how people handle

Server replacement is allowed; keeping a server process alive is not an outcome.
History, drafts, approvals and uncertain work still matter. This supersedes the
earlier claim that a credential-ownership decision must stop all implementation.
Use the existing account broker and test the native mechanism; do not wait for a
second design approval for reversible implementation. No production Session or
account is used for these probes. The other conversation's signed dev installation
does not establish recovery or resolve the ordinary-typing incident.

November 9 research found an upstream maintainer explaining that manually
changed auth files require a server restart, while account APIs are preferred:
https://github.com/openai/codex/issues/49651 (October 3 etraut-openai comment).
Issue #42771 independently reproduces stale
cross-account auth until daemon restart. The official experimental token contract
is https://learn.chatgpt.com/docs/app-server#auth-endpoints: the host supplies
tokens and answers refresh requests. It does not require a second OAuth store.

**Implementation direction:** first complete the supplied-token path using the
existing Codex account broker for stored logins and native refresh. The observer
can send `account/login/start` over its existing side connection; the native
client remains directly connected. Keep one credential/refresh authority under
the existing locks, and demonstrate callback routing with a live native client
and an injected 401 before product integration. Do not ship token delivery without
refresh, or log tokens. Server replacement remains an allowed alternative if
this fails, but do not build two implementations speculatively.

Fresh isolated Codex 0.167.0 observations (synthetic tokens, loopback only):

- `codex_auth_reload_probe.py --external-token-handoff` reports second-account
  identity and routing in the original server, leaving the auth file unchanged.
  Results: `.lf/tmp/loo-811-protocol/external-token-handoff/results.json`.
- `codex_drain_probe.py --token-handoff --approval` keeps the original native
  approval answerable, executes its command exactly once, then completes typed
  input with account headers changing from first to second. Both native processes
  survive until fixture cleanup. Results: `token-native-approval/` in that tree.
- The held-response handoff keeps the client and draft, but fails the old active
  turn; the same test without handoff completes it. Both complete later typed
  input. Results: `token-native-input-observed/` and `token-native-input-control/`.
  Do not call this an all-turn preservation pass or switch unrelated active work.
  Quota recovery targets a failed turn; newer/uncertain work must not be replayed.
- The first mixed HTTP fixture incorrectly counted plugin/MCP POSTs as model
  requests. Its initial result is invalid. Corrected probes accept only
  `/v1/responses`; use the directories above, not `token-native-input/`.
- `codex_drain_probe.py --token-handoff --quota` now passes: the first request
  receives a synthetic usage-limit 429; after account handoff the observer retries
  that exact prompt in the same thread while the native draft stays unsent.
  The retry completes, the edited draft reaches the model, and a subsequent typed
  turn completes. Headers are first, second, second, second; original server and
  client survive. Results: `token-native-quota-retry/`. All fixture children exit.

### Integration — November 9

The supplied-token path now passed native 401 callbacks both on the observer's
retry and on a later turn typed into the original TUI. Codex 0.167.0 delivered
`account/chatgptAuthTokens/refresh` to the observer; the synthetic refreshed login
was accepted, the retry and edited draft completed, and both native processes
survived until cleanup. Results: `token-native-refresh/` and
`token-native-typed-refresh/` under `.lf/tmp/loo-811-protocol/`. The held-response
failure above ended with “Fatal error: application network permission was revoked”.
It remains a counterexample to switching unrelated active work.

Claude 2.1.321 also passed the correlation probe with a synthetic quota rejection:
`claude-quota/` in the same tree. StopFailure identified the failed prompt; its
complete transcript parent chain contained no assistant/tool work, and the
`api-request` row supplied the actual selected model. The inbox UUID joined to
a different promptId and then to the successful Stop. Draft and subsequent input
completed; the fixture reaped its client and HTTP worker.

Runtime integration now uses existing Session history for failed-turn and
once-only dispatch receipts. Codex supplies tokens through the existing account
broker, then re-reads the failed thread before starting its retry. Claude's
existing watcher observes completion; detached failure workers retain credential
work beyond the native hook's lifetime. The old account-only failure handler is
removed. The served `session.recover` mutation and Desktop progress are wired.

Both two-client product probes now pass on the candidate with native clients,
synthetic logins and loopback inference. Each first-account prompt was rejected
once, each correlated continuation completed on the second login, and each edited
draft completed afterward in its original client/thread. Six requests per probe
used first, first, second, second, second, second. Receipts:
`product-codex-structured/` (Codex 0.167.0) and `product-claude-reload/`
(Claude 2.1.321) under `.lf/tmp/loo-811-protocol/`. These Sessions were unbound;
the integrated runs contained no pending approval or completed Graph step.

Three failures changed the implementation. Codex's full rejected snapshot uses
thread status `systemError`, not `idle`. Its localized “You’ve hit your usage
limit” display text escaped the generic text classifier and initially caused a
retry loop on the first login; recovery now uses structured `usageLimitExceeded`.
Claude's immediate inbox retry retained the old token; later typed input used
the replacement. Its native Keychain cache is 30 seconds. A 31-second detached
wait followed by fresh account/history reads produced the passing result above.
The wait holds no credential lock and does not itself establish adoption.

Recovery checks capacity even for an explicitly preferred login. A rejected
native model keeps its short model-specific hold when a generic usage poll shows
room. Manual launch and Graph routing retain their existing semantics. Lost
Codex replies retain uncertainty after observer restart, with no new dispatch.
Credential preparation failures remain visible as native waits.

The first product fixture cleanup ended viewers but left detached disposable
owners. All 28 identified leftovers from these private probe roots were stopped;
none remained. The fixture now stops its exact root-owned processes before
removing the store. The cleanup rerun passed and is recorded below.

Focused checks (inherited LF_* unset; logs under `.lf/tmp/loo-811-protocol/`):

- `cargo nextest run -p loopflow --lib -E 'test(session_recovery::) | test(recovery_respects_capacity_even_for_a_preferred_login) | test(lf::tests::)'` — 43 passed.
- `cargo build -p loopflow --bin lf` — passed; the final later edit only collapses an equivalent nested condition. Gate owns clippy and affected suites.
- `cargo test -p loopflow --test integration dto_fixtures::session_recovery_round_trips_the_failed_turn_and_wait` — passed.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'sessionRecoveryControlKeepsUncertaintyVisible'` — passed; earlier recovery DTO filters passed (2 tests).
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --codex /Users/jack/.local/bin/codex --output .lf/tmp/loo-811-protocol/product-codex-cleanup` — passed: both continuations and edited drafts, exact rejection counts, correlated completion, clients and owners reaped.

Main was reconciled at `4276fb28b` (merge of `4848816f7`), including #1863
`4344618dd` and #1839. `2e0018453` checkpoints the three documentation edits and
both recovery notes before that merge. The two-item Keychain repair remains.
November 9 realign fetched origin/main: still `4848816f7`, with zero commits
outside this branch. No further upstream reconciliation was needed.

### Counterexample to composing the existing operations

`human_session::open` handles Codex's explicit account change by calling
`stop_session_agent`, ending its native client as well as its app-server.
`restore::resume` reopens history but cannot restore the client's editor or
approval channel. The existing `NativeExit` also names the old attachment and
AgentProcess; replacing only the server without handing that record over would
leave quit/cleanup aimed at the wrong generation. `Recorder::attachment_moved`
currently ends the old client on any attachment change. These are not account
selection problems, and copying the existing bulk model move would create a new
seeded thread instead of continuing the native one.

A separate idle read before stopping the provider is insufficient: a native
client can submit a new turn or receive a request between the read and shutdown.
The same issue applies to a background inbox message. Recovery needs a provider
handoff that accounts for those arrivals, not an exception to native-input or
pending-request retention.

### Native observation

A disposable Codex 0.167.0 probe used a private HOME/CODEX_HOME, Unix endpoint,
PTY and a credential-free loopback Responses endpoint. External egress was denied.
It opened and named a native thread without a turn, typed a draft, stopped only
that private app-server, started its replacement at the same socket, edited the
still-running terminal's draft, and submitted it. One local synthetic response
completed. Results:

- draft rendered before replacement: true
- original client survived: true
- further editing rendered after replacement: true
- local endpoint received `DRAFT_BEFORE_RESTART_AFTER_RESTART` intact: true
- terminal displayed `RESTART_TURN_COMPLETED`: true
- local fixture requests: 1; every owned process reaped

Command: `uv run python scripts/test_network.py uv run --offline --script .lf/tmp/loo-811-protocol/restart_probe.py < /dev/null` — passed.
Probe and terminal/server artifacts are under `.lf/tmp/loo-811-protocol/`.
This proves native editor survival and continued input on an unauthenticated
custom provider. It does not prove managed-account switching, pending approval
survival, a turn racing shutdown, Loopflow receipt handoff, or multiple Sessions.

An earlier rendering-only run hit `PermissionError` in group cleanup after its
observations; a subsequent process inventory found no process carrying its
private endpoint/thread. The final probe uses direct owned-child termination,
checks every child exited, shuts down its local HTTP server, and completed cleanup.
An account-read experiment with synthetic ChatGPT credentials failed with
`workspace routing discovery failed` before account observation; it proves
nothing about hot account reload. No production credentials were read.

Claude's existing `claude_updates::deliver` uses its native messaging socket and
`priority: next`, keeping drafts and requests in their native client. Its socket
has no consumption acknowledgement: successful write remains uncertain. The
[Claude hooks reference](https://code.claude.com/docs/en/hooks#stopfailure), read
November 9, says StopFailure ignores normal output; it cannot directly retry a
turn. `asyncRewake` is a separate wake mechanism, not acknowledgement of recovery.

Codex 0.167.0's locally generated protocol includes `account/read` and
`account/login/start`. Its supplied-token login is marked unstable/internal;
using it would change credential ownership and introduce refresh callbacks.
Do not silently substitute that for the shared-home design. Upstream source read
November 9 (`codex-rs/login/src/auth/manager.rs`, `auth` and
`refresh_token_with_reason`) guards managed refresh against a different account;
this is a lead, not a native observation of the installed version.

### Native drain counterexample — November 9 implementation pass

The installed Codex 0.167.0 has provider-side admission: one SIGHUP begins a
graceful drain, rejects new work with `-32600` / `serverShuttingDown`, and waits
for admitted turns. This agrees with the [app-server errors reference](https://learn.chatgpt.com/docs/app-server#errors),
read November 9. An observer idle check is unnecessary for admission, but drain
alone does **not** satisfy native-client preservation.

The credential-free, externally network-denied probe in
`scratch/codex_drain_probe.py` runs a native PTY against a private app-server and
loopback Responses endpoint:

- **Racing input fails.** With one admitted model response held locally, SIGHUP
  kept the server alive and a side connection's new turn received the documented
  rejection. Typing `TYPED_DURING_DRAIN` and Enter in the original client submitted
  `turn/steer`; that client exited **1** with:
  `Error: turn/steer failed: Server is draining; retry after reconnecting (code -32600), data: {"reason":"serverShuttingDown"}`.
  The server later exited after releasing the fixture response, but no completed
  original turn was observed. The typed text never reached the model. Starting a
  successor at the same endpoint did not restore the terminated client.
- **Pending approval passes.** The original `item/commandExecution/requestApproval`
  stayed answerable during drain. Answering in the original PTY ran the fixture's
  `/bin/echo APPROVAL_EXECUTED`, completed the admitted turn, and allowed the
  server to exit. The same client survived replacement and completed another
  typed turn. Three local Responses requests occurred; this is no account-switch,
  Loopflow lifecycle, or multiple-Session proof.

Check: `uv run python scripts/test_network.py uv run --offline --script scratch/codex_drain_probe.py --codex /Users/jack/.local/bin/codex --output .lf/tmp/loo-811-protocol/final-drain [--approval] < /dev/null` — racing input FAIL (native client exit); approval PASS. Both runs reaped every owned child and closed their local HTTP server. Results and terminal/server logs: `.lf/tmp/loo-811-protocol/final-drain/`.

A repeat approval probe reached cleanup and hung in `waitpid` while the PTY
remained open. Terminating only that disposable probe closed its PTY; its Python
and remaining Codex PID were then absent. The saved probe closes the PTY before
waiting on children and bounds the post-kill wait. No production process was
signalled.

**Historical counterexample:** provider-generation replacement is not ready for integration.
It requires a native client that retains rejected input and stays connected/rejoins
when its server drains, or a demonstrated managed-account reload that needs no
replacement. Neither exists as a proven path here. Changing installed Codex,
putting lf in the input path, or reopening a dead client from history would not
meet this run's constraints. Loopflow receipt/lifeline changes cannot repair that
native loss. The automatic recovery and Desktop implementation remain dependent
on resolving this counterexample; publication cannot claim them complete.

### Managed-auth reload counterexample — November 9, pass 3

`scratch/codex_auth_reload_probe.py` runs Codex 0.167.0 with a private HOME,
file-backed synthetic managed ChatGPT logins, a private Unix socket and loopback
workspace discovery. External egress is denied; no model turn or quota rejection
is generated. Account reads succeed, eliminating the earlier discovery failure:

- Initial `account/read` reports `first@example.test` / workspace `first`.
- Replacing the file with changed tokens for **the same account**, then calling
  `account/read {refreshToken:true}`, reports `rotated@example.test` / `first`.
- Replacing the file with **another account**, `second`, leaves both ordinary
  and forced-refresh reads reporting the rotated **first** account.
- The second account's file remains byte-for-byte intact; the original server
  stays alive. The provider child and HTTP worker both exit during cleanup.

Check: `uv run python scripts/test_network.py uv run --offline --script scratch/codex_auth_reload_probe.py --codex /Users/jack/.local/bin/codex --output .lf/tmp/loo-811-protocol/auth-reload-control < /dev/null` — probe completed; same-account reload YES, cross-account reload NO. Results and server log are in that output directory.

The first attempt's HTTP workspace origin failed validation; using a synthetic
HTTPS routing origin made account reads succeed. No request is sent to that
origin. The same-account control distinguishes a working refresh path from a
broken fixture. This observation does not establish managed-token refresh or
native-client preservation after a switch: no switch occurred.

The [official auth reference](https://learn.chatgpt.com/docs/app-server#auth-endpoints),
read November 9, describes forced managed refresh and a separate externally owned
token mode. Upstream `AuthManager::refresh_token_with_reason` guards reload by
the cached account ID; `get_account` calls that refresh then reads cached auth.
These sources agree with the native result. Current source inventory:
`openai/codex@9f04d886f3663b584fba87d6fd71001fb6f9fcf3`; downloaded sources are
under `.lf/tmp/loo-811-protocol/`. The installed build's behavior is established
by the probe, not inferred from that upstream revision.

**Historical status before the supplied-token direction above:**
Neither tested path meets the design: replacement loses racing input, while
managed `account/read` refresh retains the old account. No alternative managed
reload operation has been demonstrated. Switching to external-token ownership,
modifying Codex, or reducing native preservation would change the accepted
approach; none is selected. Dependent lifecycle, receipt and Desktop integration
is stopped at this contradiction. Publication and the demo boundary remain
unreached; the Task outcome is not complete.

### Claude native correlation — November 9, pass 3

`scratch/claude_retry_correlation_probe.py` ran Claude **2.1.321** (the installed
version at this observation), in a disposable home and PTY with a synthetic API
key, local Messages responses and external egress denied. No quota exhaustion
or real login was used. Its SessionStart hook kept the native inbox token in
process memory and sent one ordinary `priority: next` message with a chosen UUID.

- Initial typed input completed through `UserPromptSubmit` and `Stop`.
- The injected message retained the chosen UUID in the transcript's `user` row.
  That row's **`promptId` differs from its UUID** and matches both the native
  `UserPromptSubmit.prompt_id` and successful `Stop.prompt_id` for that turn.
- A draft typed before dispatch remained editable. Appending `_AFTER_RETRY`
  and submitting yielded `DRAFT_BEFORE_RETRY_AFTER_RETRY` intact, followed by a
  third successful Stop with its own prompt ID. Exactly three local Messages
  requests occurred, all on `claude-sonnet-5-5` (the native `sonnet` selection).
- The original client stayed alive; it and the HTTP worker were reaped.
  A subsequent PID-only probe found no process matching either disposable
  probe or its `loo811-claude-` home prefix.

Check: `uv run python scripts/test_network.py uv run python scratch/claude_retry_correlation_probe.py --claude /Users/jack/.local/bin/claude --output .lf/tmp/loo-811-protocol/claude-correlation-final < /dev/null` — correlation, retained draft and subsequent completed turn observed. Native transcript, hook inputs, local requests and terminal bytes are in that output directory.

The [hooks reference](https://code.claude.com/docs/en/hooks#common-input-fields),
read November 9, says transcripts can lag hooks. The surviving design therefore
joins dispatch UUID → transcript user `promptId` → Stop `prompt_id`, retaining
uncertainty until both records arrive. A socket write or unrelated Stop cannot
mark continuation. Missing/ambiguous transcript rows remain uncertain, never
trigger a resend. Model changes need native model-switch observations, not the
launch default. This establishes receipt correlation, not rejected-turn safety,
managed-login reload, approvals, simultaneous Sessions or Graph non-replay.

Probe setup corrections: the private Claude config belongs inside
`CLAUDE_CONFIG_DIR`; the synthetic API-key prompt required an affirmative choice.
An initial cleanup tried to signal an already-exited process group, raised
PermissionError and left its HTTP worker alive. Only the two disposable Python
processes were terminated; the corrected probe polls the child before signalling.
The successful final run and the later PID-only inventory found no leftovers.

### Recovery owner

The Session's existing native observer owns recovery, alongside the provider.
The Session history owns the failed-turn identity and recovery receipts. Desktop
and a served bulk mutation request another attempt; they never start a second
observer, inject terminal keystrokes, select a model, or move a Graph cursor.

One attempt is identified by Session, native thread and failed native turn.
It records the reported model/account and the old provider generation. It keeps
rejected-before-work distinct from unknown delivery, produced output and tool
activity. A capacity snapshot alone cannot identify a rejected turn. A sparse or
gapped trace stays unknown. Reconnect cannot recreate a consumed retry request.

The observer keeps its launch's account restrictions and repository/Wave route.
Selection uses the model actually active on the native thread, including changes
made in its UI. An unresolvable model is a named wait; it is never routed using
an unspecified default. Account switches remain serialized by the existing
activation/refresh locks. A late report from the old generation cannot cool the
new active login. All affected Sessions independently settle their own turns even
when one shared activation suffices.

Claude continues through its existing native inbox after a definite rejection
and successful activation. A dispatch receipt precedes the write. The native
transcript joins its UUID to the Stop hook's prompt ID as observed above;
this correlation, not a successful socket write, advances recovery. No retry follows
an uncertain write; a later successful native turn may establish continuation.
A turn with earlier tool effects stays intact for review instead of resending its
original input. Pending requests remain with Claude.

Codex supplies the selected login's access token through its existing private
server and answers native refresh callbacks through the account broker. Its
native client stays directly connected. Re-read the full failed turn after
credential work; newer work or uncertainty prevents continuation. No provider
replacement, second OAuth store, or terminal input injector remains to build.
The earlier replacement probes remain counterexamples, not a second path.

### Desktop and receipts

Expose one recovery reading beside each affected Session: waiting for capacity
(with reset when known), waiting for a native request, uncertain delivery,
recovering, or continued. **Recover affected sessions** requests attempts for
those exact Sessions; it cannot override uncertainty or pending approvals.
Progress comes from the same Session receipts used by automatic recovery.
"Continued" requires a successful correlated native turn, not changed credentials,
process liveness, a connected socket or an accepted start request.

Extend the existing Session projection and served mutation paths, with Rust/Swift
DTO fixtures. Keep **Move Blocked Sessions** for intentional model/provider moves.
No account preferences, model fallback, Graph completion or Auto-Ship changes.

### Delete — do not maintain

- Replace `provider_account.rs::record_native_limit` with the Session observer's
  model-aware, turn-correlated decision. Cut its `record_limit` caller over in
  the same change. Keep the detached credential-work lifetime and existing
  activation/refresh locks, cooldown and
  quota-window behavior on that surviving path; migrate its exclusive recovery
  tests there. Keep ordinary manual account switching.
- No generation replacement is needed by the supplied-token path. The earlier
  proposed deletion of `Recorder::attachment_moved` is withdrawn; ordinary moves,
  closes, lost authority and native exit retain their existing owners.
- No deletion of model moves, native-input retention, pending-approval handling,
  Graph limit recovery, Keychain reconciliation, or #1863/#1839 reconnect/input
  behavior. They remain required behavior, not fallback implementations.

The account-only handler is removed; correlated recovery is its surviving path. The surviving Keychain writer,
activation and command tests need no further reduction: each remaining case covers
a distinct installation failure. Keep the drain probe as the failing acceptance
case; it is not a second recovery implementation. Claude's probe now joins both
retry and draft rows against one set of completed prompt IDs, requiring the
fixture response and a unique transcript row for each; missing or ambiguous
records cannot pass. It writes terminal bytes once, before cleanup.

Compression keeps one Claude inbox writer for Task updates and quota recovery;
each caller still owns its dispatch receipt and provider fence. The duplicate
`session_provider_thread` reader is removed in favor of `session_thread`.
Codex polls one recovery snapshot and shares retry eligibility with the served
request path. Rejected input borrows the native snapshot until serialization;
the two classifier test modules are combined. No further predecessor remains
to delete; native drain/reload probes retain their contrary results.

Compression checks, November 9 (inherited LF_* unset):

- `cargo nextest run -p loopflow --lib -E 'test(session_recovery::) | test(harness::claude_updates::)' < /dev/null` — 4 passed on the final code; existing large `__eh_frame` linker warning. Gate owns affected suites/clippy.
- `cargo build -p loopflow --bin lf` — passed in 1m 07s after the final reader cut.
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --codex /Users/jack/.local/bin/codex --output .lf/tmp/loo-811-protocol/product-codex-compress < /dev/null` — passed: both continuations and edited drafts, one rejection each, correlated completion, clients/owners reaped.
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --claude /Users/jack/.local/bin/claude --output .lf/tmp/loo-811-protocol/product-claude-compress < /dev/null` — passed with the same results; synthetic logins/local inference only. Gate/demo retain the Task-bound, approval and installed checks below.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; memory and scratch fit token and byte budgets.

### Remaining verification

The runtime, served bulk mutation and Desktop control are implemented. Unit and
native checks above establish the covered paths; the complete delivery outcome
still includes these cases:

1. Gate the affected lifecycle, transport, provider-account, native decoder,
   hook, Session command/projection, DTO and Swift view suites. Include rejected
   turns with earlier effects, missing events, lost replies, reconnect, stale
   attachment, `/new`, and repeated limits. Fixed dispatch receipts must prevent
   duplicates even when completion is unknown.
2. Exercise model changes, distinct models and restricted routes, simultaneous
   reports, failed/partial activation, no capacity and later reset. Preferred
   login capacity and model-specific holds have focused coverage; product probes
   cover simultaneous same-model rejection and one successful alternate.
3. Exercise bulk recovery through the served/Desktop entry with Task-bound
   Sessions, pending approvals and retained drafts. Keep Task, Session and native
   history identity; count completed Graph steps and tool effects once. The
   separate Codex native token probe preserves an approval, but the integrated
   two-client probes do not establish this complete combination.
4. Demo/review retains isolated real-login refresh/token rotation, installed
   two-item Keychain behavior and the other observations in live-recovery.md.
   No production logins or Sessions were changed. Publication waits for the
   whole recovery outcome; account switching alone does not finish it.

### Earlier reconciliation — superseded by integration above

The saved drain receipts confirm client exit 1 and lost racing input, separately
from the passing approval/subsequent-turn case. Both use synthetic responses;
managed-login continuation and multiple-Session recovery remain unproved.

Pass 3 tested managed-account reload: ordinary and forced-refresh account reads
retain the old account even though same-account rotation reloads correctly.
The Codex path now needs a different demonstrated mechanism or a revised design;
repeating either disproved path cannot finish it. Claude correlation is observed,
with failed-turn classification and product integration still open. No scope
reduction, credential-ownership change or provider modification has been chosen.
Full pre-curation notes and drain probe:
`f5b3539644f30b22842273bdba995077eb2bd905`.

Realign re-read the saved managed-auth and compressed Claude receipts; they agree
with the findings above, including child cleanup. Main remains `4848816f7`, with
zero commits outside this branch. Infrastructure memory now records both new
findings; its Release child's complete top-level Markdown added no recovery
mechanism. Documentation now distinguishes planned recovery from available account
switching and history-only Codex restart. No native probe or runtime suite was
rerun for this prose repair.

Check: `uv run python scripts/test_network.py uv run python scratch/claude_retry_correlation_probe.py --claude /Users/jack/.local/bin/claude --output .lf/tmp/loo-811-protocol/claude-correlation-compress < /dev/null` — passed after compression: retry correlation, intact draft and subsequent completed turn; client and HTTP worker reaped. Ruff check/format, `git diff --check` and `lf context` passed; both budgets fit. Product runtime is unchanged; gate owns affected runtime suites after implementation.

Realign check: `git diff --check` — passed; `lf context` — memory and scratch fit
both limits. Runtime checks were reused, not rerun.
