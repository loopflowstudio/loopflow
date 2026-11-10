# Open Task Sessions after a usage limit

November 9, 2026, demo conversation. Jack Heart:

> i have still had the issue that if i have a bunch of task sessions open and they all hit a usage limit i need to restart them all. can we automatically do soething or at least add something in hte desktop so you dont need to go to an obscure cli command

The full reported outcome remains unverified. No demonstration or approval of
the expanded LOO-811 outcome occurred. During the first demo preparation its
candidate binary was absent and installed lf reported `0.15.2+7e90dafb1.dirty`. A later demo request
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

## Native admission investigation — November 9, latest implementation pass

Two different native mechanisms were exercised, without file-refresh, drain,
account handoff, a proxy or another history-read gate. Neither provides safe
automatic recovery. Production code remains on the truthful waiting path.

**Codex 0.167.0:** `turn/steer` with `expectedTurnId` rejects the quota-failed
turn with `-32600`, `no active turn to steer`. While a newer typed turn has a
held model response, the old ID is rejected as mismatched. The positive control
using the newer active ID succeeds. After that turn completes, the failed ID
again receives `no active turn to steer`. No stale retry reaches the model;
the edited draft and subsequent typed input complete. Thus the conditional
operation protects active work but cannot restart rejected work. No credential
preparation was attempted because even baseline admission is unavailable.

**Claude 2.1.321:** `StopFailure` with `asyncRewake: true` and exit 2 wakes the
native conversation after a synthetic quota error. Its idle baseline keeps the
draft and completes later input. Two race probes hold the hook at separate
preparation and final-dispatch barriers. In each, newer typed input completes
before the hook is released; releasing it still starts another model request
containing the old wake message. Requests increase from two to three. Drafts
remain editable and later input completes, but the stale wake violates the
required precedence. These barriers model timing around credential preparation;
they perform no credential writes and establish no account adoption.

Receipts under `.lf/tmp/loo-811-protocol/`: `conditional-steer-final/`,
`rewake-baseline/`, `rewake-preparation/`, `rewake-dispatch/`. Native transcripts,
turn results and hooks are retained there. All probes use private homes and
loopback inference with external egress denied; native children and HTTP workers
were reaped. These are mechanism counterexamples, not product acceptance.

Checks (inherited LF_* cleared by `scripts/test_network.py`):

- `uv run python scripts/test_network.py uv run --offline --script scratch/codex_drain_probe.py --codex /Users/jack/.local/bin/codex --conditional-steer --output .lf/tmp/loo-811-protocol/conditional-steer-final < /dev/null` — rejected-turn counterexample reproduced; active control, edited draft, later input and cleanup passed.
- `uv run python scripts/test_network.py uv run --offline python scratch/claude_retry_correlation_probe.py --claude /Users/jack/.local/bin/claude --async-rewake --race <none|preparation|dispatch> --output .lf/tmp/loo-811-protocol/rewake-<case> < /dev/null` — baseline passed; both races reproduced stale wake after newer completion. Cleanup passed in all three runs.
- `uv run ruff check scratch/codex_drain_probe.py scratch/claude_retry_correlation_probe.py`, Ruff formatting, `git diff --check` and `lf context` — passed; both context budgets fit. No Rust/Swift runtime changed; broader checks remain with gate.

The [Codex app-server reference](https://learn.chatgpt.com/docs/app-server) and
installed generated schema describe an active-turn condition on `turn/steer`,
not a failed-turn condition on `turn/start` or account login. Its
[model-request hooks](https://learn.chatgpt.com/docs/hooks#premodelrequest-and-postmodelrequest)
are diagnostic, without retry control. The
[Claude hooks reference](https://code.claude.com/docs/en/hooks#command-hook-fields)
describes asynchronous wake on exit 2; the native probes above show its behavior
after newer input. Sources read November 9; installed behavior is measured,
not inferred from current documentation.

### Required native operation; dependent integration stopped

The next implementation needs a demonstrated provider operation that admits
recovery only if the rejected turn is still current, combines that admission
with credential adoption, and reports a correlated attempt. New input must
either win admission with no old retry, or follow an already-admitted recovery
without credential replacement failing its request. Existing lf locks and
observation cannot add this contract to independently connected native clients.
The two new candidates above do not supply it. No safe sending implementation
is available to integrate in this pass; reintroducing either would contradict
the preservation requirement. This is a native capability gap, not a request for
a different product outcome. Task-bound bulk recovery, pending-approval checks,
publication and demo remain incomplete.

### Delete — do not maintain

The earlier unconditional sends stay removed: Codex recovery's
`account/login/start` plus `turn/start`, and Claude recovery's shared activation,
cache-expiry sleep and inbox send. No disabled copy or second credential owner
replaces them. Retain broker refresh, normal Task inbox delivery, manual account
switches, Graph recovery and retained attempt/completion facts. Historical probes
remain counterexamples; their successful old sends are not acceptance checks.

Keep Claude's detached capacity work separate from its polling/completion watcher;
the former `Recovery.detached` mode and second transcript read are removed. Waiting
polls read history only when due; dispatched attempts still check completion every
tick. Both providers record capacity/admission waits through one write path.
Uncommitted notes before this compression are saved in
`.lf/tmp/compress-admission-before/`; the earlier backup below is separate.

Watcher compression checks, November 9 (inherited LF_* unset; logs
`.lf/tmp/loo-811-compress-watchers-*.log`):

- `cargo nextest run -p loopflow --lib -E 'test(session_recovery::) | test(harness::claude_recovery::) | test(harness::claude_updates::) | test(harness::codex_connection::tests::client_turns_keep_their_origin_during_a_recovery_read)' < /dev/null` — 6 passed; correlated Stop bypasses capacity backoff. Existing large `__eh_frame` linker warning; broader suites/clippy remain with gate.
- `cargo build -p loopflow --bin lf < /dev/null` — passed.
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --claude /Users/jack/.local/bin/claude --output .lf/tmp/loo-811-protocol/compress-watchers-claude < /dev/null` — passed: two waits, intact edited drafts, typed completion, no handoff/retry input, clients/owners reaped.
- Same command with `--codex /Users/jack/.local/bin/codex` and output `compress-watchers-codex` — passed with the same results. Both probes use synthetic logins/loopback inference, not real-login or automatic-recovery acceptance.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; memory and scratch fit both limits.

## Implement revision — November 9: native admission remains missing

The automatic outcome remains required and unfinished. The native APIs inspected
here cannot admit account handoff plus retry only while the rejected turn is
still current. Codex's generated 0.167.0 `TurnStartParams` has no such condition:
`parentTurnId` is attribution, explicitly ignored when adding input to an active
turn. `LoginAccountParams` has no thread/turn condition. Sources remain under
`.lf/tmp/loo-811-protocol/schema/v2/`; the saved `account_processor.rs` performs
server-wide external-auth replacement. The earlier active-response failure is
still contrary evidence. Claude's inbox queues `priority: next` without a
failed-prompt condition; the transcript read cannot reserve that queue position.

Revised implementation removes interactive recovery's Codex login/start sends
and Claude activation/cache-sleep/inbox send. Available capacity produces a named
native-admission wait; unavailable capacity retains its reason/reset. The broker,
Codex same-account refresh callback, Session owners, normal Task inbox messages,
manual account switching and Graph recovery remain. There is no speculative
second-read protection or disabled alternate sending path. Native admission is
an implementation limit, not a new product choice or a reduced acceptance bar.
A safe provider mechanism must be demonstrated before automatic sends return.

The reader now folds dispatch, correlated reply and completion for a failed turn;
a late wait cannot replace those facts. Latest failure means latest *first*
observation, so an older worker cannot hide a subsequent rejection. Existing
wire types stay unchanged. A reconnect continues observing an existing attempt
and never resends it. Earlier native product successes below describe the removed
unconditional sending path, not acceptance of this revision.

Verification, inherited LF_* unset (logs `.lf/tmp/loo-811-*-test.log`):

- `cargo test -p loopflow --lib session_recovery::` — 3 passed, including same-thread provider replacement; existing large `__eh_frame` linker warning.
- `cargo test -p loopflow --lib harness::claude_recovery::` — 1 passed: unrelated Stop stays uncertain; correlated Stop settles after late wait/reconnect.
- `cargo test -p loopflow --test served_reads served_bulk_recovery_keeps_task_identity_and_dispatched_attempts` — built candidate and passed; fixed the merged tree's missing `SessionRecover` CLI-spelling match arm. Task memberships/primary Sessions and Process count stay unchanged.
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --codex /Users/jack/.local/bin/codex --output .lf/tmp/loo-811-protocol/admission-wait-codex` — passed: two native waits, no retry input or credential handoff, edited drafts complete, clients/owners reaped.
- Same probe with `--claude /Users/jack/.local/bin/claude --output .lf/tmp/loo-811-protocol/admission-wait-claude` — passed with the same results; temporary scoped Keychain items removed. Both probes use synthetic logins/loopback inference and unbound Sessions.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; gate owns merged-tree suites/clippy.

Compression checks after the admission cut (inherited LF_* unset):

- `cargo nextest run -p loopflow --lib -E 'test(session_recovery::) | test(harness::claude_recovery::) | test(harness::claude_updates::) | test(harness::codex_connection::tests::client_turns_keep_their_origin_during_a_recovery_read)' < /dev/null` — 6 passed on the final formatted tree; existing large `__eh_frame` linker warning. Log: `.lf/tmp/loo-811-compress-admission-test.log`.
- `cargo build -p loopflow --bin lf < /dev/null` — passed; log `.lf/tmp/loo-811-compress-admission-build.log`.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; fixture formatting retained identical parsed JSON. Gate owns merged-tree suites/clippy; native admission remains unfinished.

Pre-compression uncommitted notes are saved under `.lf/tmp/compress-before/`.

## Current design and remaining work

The existing Session/native conversation, failed turn and derived recovery reading
remain the three concepts. Session history retains dispatch and correlated results;
changing capacity observations cannot erase them. The current-thread reader picks
the latest failed turn by its first observation, folds its facts and keeps them
across provider replacement. The account broker owns credentials. The served
`session.recover` mutation requests checks; Desktop projects the same reading.
Neither starts another owner nor moves a Graph cursor. Normal Task inbox messages,
manual account switching, model moves and Graph recovery keep their existing paths.

The concept review's delayed-worker counterexample is repaired: the focused
Session test covers dispatch → late wait → reopened reader → correlated completion,
then a separate rejection and late old-worker observation. The Claude test settles
only a matching inbox UUID → promptId → Stop after reconnect. The served fixture
uses two Task primary Sessions, retains their memberships and attempts, refuses
a second request for dispatched work, and starts no Process. These fixtures do
not exercise native automatic admission or pending approvals through Desktop.

### Implementation still required

1. Establish safe native admission for **both** providers, including credential
   handoff. Input arriving during credential preparation and immediately before
   dispatch must take precedence without an old retry queuing behind it or a
   handoff failing the new turn. The inspected APIs provide no conditional
   operation; an extra history read, receipt or lf lock cannot serialize a directly
   connected native client. The current native wait is truthful but does not
   satisfy automatic continuation. Server replacement remains allowed, with the
   preservation cases below; the disproven drain and file-refresh designs remain
   counterexamples. No alternative provider mechanism is yet demonstrated.
2. Integrate the demonstrated mechanism through the existing broker and Session
   owners. Keep native history, selected model, draft and pending approval, retain
   uncertain delivery, and execute completed tool effects and Graph steps once.
   Preserve retained attempt correlation and separate subsequent rejections.
3. Extend the Task-bound served/Desktop path to two simultaneously rejected native
   Sessions, including a draft and pending approval, successful continuations and
   subsequent typed input. Existing unbound product passes describe the removed
   unconditional sends; the new wait probes establish no automatic recovery.
4. Cover model changes, distinct models/restricted routes, simultaneous reports,
   partial credential preparation, no capacity and later reset. Preferred-login
   capacity and model-specific holds already have focused coverage. Gate owns
   affected lifecycle, transport, account, decoder/hook, Session/projection, DTO
   and Swift view suites, including missing history, lost replies, stale attachment,
   `/new`, repeated limits and earlier effects. The latest focused build/checks
   above passed; those do not replace the broader merged-tree gate.

The ordinary-typing incident remains unresolved. The LOO-813 handoff reported
physical x/z invisible in Desktop and a passive owner snapshot; pasting y then
revealed xzy, and both affected conversations subsequently worked after Reset.
Those observations do not establish the cause. A disposable reproduction must
exercise physical keyboard input after reopen; quota retry/draft fixtures and
the separate installation inventory do not establish its repair.

Installed/real-login refresh, token rotation, unsuffixed Keychain repair and the
other observations remain in [live-recovery.md](live-recovery.md). No isolated
real-login environment was available. Headless checks continue in disposable
environments; configured observations and judgment of the complete experience
remain for demo/review. No new product decision is needed for reversible work.

The review page, `scratch/concept-review.html`, records the pre-repair findings.
Its proposed usage wording is now in `docs/sessions.md`; those docs and the
harness/subscription/Desktop docs accurately state that automatic continuation
is unfinished. Remaining native admission is substantial implementation work,
not a documentation adjustment or a smaller acceptance bar.

## Native observations — November 9, before the admission cut

The results below retain the successful experiments and contrary findings from
the earlier sending implementation. They do not describe current automatic sends.

### Direction

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

**Earlier implementation:** supplied tokens used the existing Codex account broker
and native refresh. The observer sent `account/login/start` over its side
connection; the native client stayed directly connected. Native 401 callbacks
passed before integration. No second OAuth store or server replacement path was
added. Server replacement remains permitted; failed drain and file-refresh
observations below still apply.

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

### Earlier integration — November 9

The supplied-token path passed native 401 callbacks both on the observer's
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

The earlier integration supplied Codex tokens through the broker before re-reading
the failed thread, and sent Claude inbox continuations from detached workers.
Both sending paths were removed after the admission review. Session history,
completion observers, served checks and Desktop progress remain.

Both earlier two-client product probes passed on that candidate with native clients,
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
That wait held no credential lock; it established neither admission nor adoption
by itself, and was removed with the inbox recovery send.

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

The latest reconciliation is `e8c777f9c`, merging main `7cf2d25b3` after the
implementation/compression checks above. It includes #1863, #1858's runtime
ownership/native-exit changes, #1865's Desktop watches and #1854's submitted
creation recovery. Realign fetched main again at `7cf2d25b3`; no upstream
commits remain outside this branch. Earlier automatic-product results predate
that merge and the admission cut; the focused admission checks above are later.
Gate still owns broader merged-tree checks. The two-item Keychain repair remains.

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

Codex 0.167.0's supplied-token login is marked unstable/internal. The initial
credential-ownership objection is superseded by the continuation direction and
native callback results above. The implementation keeps the existing broker as
the credential authority.

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

The drain result rules out that replacement design with this native client.
The supplied-token implementation avoids drain; it does not make the lost-input
counterexample a pass.

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

### Earlier product compression checks

Before the admission cut, both `product-codex-compress/results.json` and
`product-claude-compress/results.json` recorded six requests, two recoveries,
correlated completion, intact drafts, one rejection per client and reaped
clients/owners. They used synthetic logins and loopback inference. The native
probe commands and earlier 43-test, DTO and Swift results above apply to that
sending implementation, not the current wait-only behavior.

Full pre-realignment plan and concept review text:
`65686dbf98654371040f100b383d742df0dc49bd:scratch/interactive-limit-recovery.md`.
Earlier investigation: `e8c777f9c:scratch/interactive-limit-recovery.md`,
“Earlier reconciliation”; pre-curation drain notes: `f5b3539644f30b22842273bdba995077eb2bd905`.

Realign reused the saved six-test/build logs and both `admission-wait-*/results.json`
receipts: two native waits, no credential handoff or retry input, intact edited
drafts, completed typed turns, clients/owners reaped. No runtime code changed or
native probe reran. All Infrastructure memory and its Release child's top-level
Markdown were read; no child context was omitted.

Realign check: `git diff --check` — passed; `lf context` — both budgets fit.
