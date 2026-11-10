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

## Native replacement and submission checks — November 9, latest pass

Automatic continuation remains unfinished. No production recovery sends were
added. The probes now separate a promising Claude submission mechanism from
its failure case, and directly test managed-account Codex replacement.

**Claude 2.1.322** (the binary reported this version during these runs): native
`UserPromptSubmit` runs for `StopFailure.asyncRewake` messages. Its `prompt_id`
correlates to the retry's successful Stop; the earlier claim that hook-wake
completion cannot be correlated is superseded by this observation.

- A blocking submission hook rejects a stale wake after newer typed input has
  completed. Requests stay at two; the existing draft remains editable and its
  later submission completes. A separately typed blocking control also passes.
- A valid wake can prepare the scoped native Keychain login inside that hook,
  wait for the measured cache lifetime, then proceed. Requests use first → second
  → second; retry completion, edited draft and later input all pass.
- Input typed while the submission hook is held before credential preparation,
  or immediately before final release, stays in native order: the admitted retry
  completes first, then the queued edited input completes on the second login.
  Both runs have first → second → second → second request accounts, correlated
  completions and intact drafts. These observations use a native submission hook,
  not an observer history read or an lf lock as admission.
- A stale wake reaching the same conditional hook after newer input completed
  is rejected **before** any credential write. All three model requests remain
  on the first synthetic login (initial rejection, newer input, later draft).
- **Counterexample:** make that submission hook exit 1 before its decision.
  Native Claude allows the stale wake through: requests rise from two to three
  after newer input completed. The stale retry even reports a correlated Stop.
  Correlation establishes execution, not permission to execute.

The [Claude hooks reference](https://code.claude.com/docs/en/hooks#userpromptsubmit),
read November 9, documents timeout continuation and `onFailure: "block"`.
`UserPromptSubmit` has no native matcher. Applying blocking failure behavior to
all lf prompt hooks would violate ordinary input's best-effort contract. The next
Claude implementation must isolate the recovery-only gate from the ordinary
context hook, demonstrate rejection on gate failure/timeout and cancellation,
then integrate broker activation and correlated attempts. A candidate is a
self-contained native hook wrapper which invokes the recovery gate only for its
own envelope; normal input must not depend on that gate or lf state. This wrapper
has **not** been implemented or demonstrated. Do not reintroduce sends with the
current fail-open hook. The synchronous credential wait also needs bounded broker
work and partial-write handling; a synthetic Keychain write is not that integration.

**Codex 0.167.0:** direct server replacement, without SIGHUP, adopts the second
synthetic managed login. The idle rejected-turn baseline retains the native
client, same thread/model and draft; retry and subsequent edited input complete
with request accounts first → second → second.

Two preservation cases invalidate unconditional replacement. A newer native
request held before shutdown becomes `interrupted` in the replacement's history;
its response connection is cut. With a pending approval, the original turn also
becomes `interrupted`; typing `y` after replacement creates a new user turn,
executes no original command and no longer answers the approval. Keeping the
TUI alive does not retain its server-side work. Input submitted while the server
is down remains in the editor; after reconnect, appending text and submitting
again completes it. That last result is draft retention, not transparent delivery.
No replacement runtime path was integrated. The earlier drain counterexample
and active supplied-token-handoff failure remain applicable.

Receipts under `.lf/tmp/loo-811-protocol/`:
`replace-{baseline,preparation,approval,disconnected}/`,
`rewake-submit-preservation/`, `submit-handoff-{baseline,preparation,dispatch}/`,
`submit-stale-dispatch/`, `submit-failure-dispatch/`. Each has native transcript,
request/turn or hook ordering and cleanup results. Initial submission-control
fixtures restored their blocked typed text, and a second fixture incorrectly
waited for Stop after deliberately blocking wake; those failed setup runs are
retained as `rewake-submit-block/` and `rewake-submit-block-final/`. Corrected
controls clear only that fixture input and wait for native submission. No
production credentials, Sessions or installation were changed. All owned native
clients, servers and HTTP workers were reaped; scoped Keychain items were removed.

Checks (all native probes use `scripts/test_network.py`, private homes and
loopback synthetic inference; external egress denied):

- `uv run --offline --script scratch/codex_drain_probe.py --codex /Users/jack/.local/bin/codex --replace-server [--race preparation|disconnected | --approval] --output <receipt> < /dev/null` — idle continuation passed; active-turn and approval counterexamples recorded; disconnected editor/input observed. Saved-receipt assertions check each outcome.
- `uv run --offline python scratch/claude_retry_correlation_probe.py --claude /Users/jack/.local/bin/claude --async-rewake --admit-recovery --handoff [--gate-race preparation|dispatch | --race dispatch] --output <receipt> < /dev/null` — valid admission, native ordering, credential adoption, stale rejection and cleanup passed. The same probe with `--gate-failure --race dispatch` (without handoff) reproduced fail-open stale execution.
- Ruff check/format for both probes, saved native receipt assertions and `git diff --check` — passed. `lf context` — scratch and memory fit both budgets. Runtime source is unchanged; earlier focused Rust/build results remain applicable. Gate owns broader suites.

## Prior native admission investigation — November 9

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
All three wake receipts also report `retry_prompt_id: null` and
`retry_stop_correlated: false`: a model request occurred, but the existing inbox
UUID → promptId → Stop completion contract was not established for hook wake.

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

## Delete — do not maintain

The unsafe sending paths are removed: Codex recovery's `account/login/start`
and `turn/start`; Claude recovery's shared activation, cache-expiry sleep and
inbox send. Keep them removed until native admission is demonstrated. No disabled
copy, second credential owner, extra history read or lf lock substitutes for it.
Historical probes stay as counterexamples, including successful unconditional
sends that did not protect racing input.

The old Claude `Recovery.detached` mode and duplicate transcript read are gone.
Detached capacity work remains separate from polling/completion; a capacity wait
cannot delay observing a dispatched attempt's completion. Retain broker refresh,
normal Task inbox delivery, manual account switching, Graph recovery and all
attempt/completion facts.

This compression removes Codex's unused replay-payload return: the history
validator answers only whether rejection preceded work. Pending history reads
own their timeout instead of leaving a separate timestamp after completion.
Claude's single-match checks use iterators, retaining rejection of duplicate
rows without allocating collections. No remaining predecessor deletion targets.

Compression checks for this revision (inherited LF_* unset):

- `cargo nextest run -p loopflow --lib -E 'test(session_recovery::) | test(harness::claude_recovery::) | test(harness::claude_updates::) | test(harness::codex_connection::tests::client_turns_keep_their_origin_during_a_recovery_read)' < /dev/null` — 6 passed; nextest marked the pure Claude history-parser test leaky. Existing large `__eh_frame` linker warning. Log: `.lf/tmp/loo-811-compress-final-test.log`.
- `cargo nextest run -p loopflow --lib -E 'test(session_recovery::tests::claude_requires_the_entire_rejected_prompt_chain_and_its_actual_model)' < /dev/null` — 1 passed without a leak report; no subprocess path found in that parser test. The first report remains unexplained; gate retains the broader check. Log: `.lf/tmp/loo-811-compress-final-leak-check.log`.
- `cargo build -p loopflow --bin lf < /dev/null` — passed; `.lf/tmp/loo-811-compress-final-build.log`.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; both context budgets fit. No new native recovery claim; gate owns broader suites/clippy.

### Retained implementation checks — November 9

All Rust commands below ran with inherited LF_* unset. Earlier uncommitted notes
are in `.lf/tmp/compress-before/` and `.lf/tmp/compress-admission-before/`;
the full notes before this pass are in `.lf/tmp/loo-811-compress-final-before/`.

- `cargo test -p loopflow --lib session_recovery::` — 3 passed, including same-thread provider replacement; existing large `__eh_frame` linker warning.
- `cargo test -p loopflow --lib harness::claude_recovery::` — 1 passed: unrelated Stop stays uncertain; correlated Stop settles after late wait/reconnect.
- `cargo test -p loopflow --test served_reads served_bulk_recovery_keeps_task_identity_and_dispatched_attempts` — built candidate and passed; repaired the merged tree's missing `SessionRecover` CLI-spelling match arm. Task memberships, primary Sessions and Process count stayed unchanged.
- `cargo nextest run -p loopflow --lib -E 'test(session_recovery::) | test(harness::claude_recovery::) | test(harness::claude_updates::) | test(harness::codex_connection::tests::client_turns_keep_their_origin_during_a_recovery_read)' < /dev/null` — 6 passed in both admission and watcher compression; correlated Stop bypassed capacity backoff. Logs: `.lf/tmp/loo-811-compress-{admission,watchers}-test.log`.
- `cargo build -p loopflow --bin lf < /dev/null` — passed after both compressions; logs `.lf/tmp/loo-811-compress-{admission,watchers}-build.log`.
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --codex /Users/jack/.local/bin/codex --output <receipt> < /dev/null` — passed at `admission-wait-codex` and `compress-watchers-codex` under `.lf/tmp/loo-811-protocol/`. Two waits, intact edited drafts, typed completion, no retry input or credential handoff; clients/owners reaped.
- Same probe with `--claude /Users/jack/.local/bin/claude` — passed at `admission-wait-claude` and `compress-watchers-claude`; scoped Keychain items removed. These are unbound Sessions with synthetic logins/loopback inference, not automatic recovery or real-login acceptance.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; parsed DTO fixtures unchanged, memory and scratch fit both limits. Gate owns broader merged-tree suites/clippy.

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
   handoff. New input either wins with no old retry or follows an admitted recovery
   without credential replacement failing its request. Claude needs failure/timeout
   rejection confined to recovery; ordinary context delivery stays best-effort.
   Codex needs an operation that retains active work and approvals. The native
   counterexamples above rule out unconditional handoff/replacement and history-read
   gates; they do not rule out every mechanism. The current wait is truthful but
   does not satisfy automatic continuation.
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

## Earlier native results and retained counterexamples

Full sources and detailed check history remain at
`98c32372889f39819bcaccd512f1849ea5446d55:scratch/interactive-limit-recovery.md`.
Pre-curation uncommitted notes are saved under
`.lf/tmp/loo-811-before-submission-probes/`. The current plan above supersedes
historical implementation instructions, not their measured failures.

Jack Heart's November 9 direction remains:

> ok figure out ow to make progress

> do we need another strategy? it is ok if we need to restart the codex servers to switch
> or look online for how people handle

Server replacement is allowed; retaining the server PID is not an outcome.
Native history, drafts, approvals, newer input and uncertain work still matter.

Retained observations (synthetic credentials, loopback only):

- Codex 0.167.0's generated `TurnStartParams` has no rejected-turn condition;
  `parentTurnId` is attribution and is ignored when adding input to an active
  turn. `LoginAccountParams` has no thread/turn condition. Schema and saved
  `account_processor.rs` under `.lf/tmp/loo-811-protocol/` show server-wide
  external-auth replacement. Claude's inbox `priority: next` likewise has no
  failed-prompt condition.
- Codex 0.167.0 managed file reload follows token rotation for the same account,
  but forced refresh still reports the old identity after a cross-account file
  replacement. Receipts: `auth-reload-control/`. Upstream source inventory:
  `openai/codex@9f04d886f3663b584fba87d6fd71001fb6f9fcf3` in the protocol directory.
- Supplied-token login changes identity without rewriting auth.json; native 401
  callbacks on observer and later TUI requests use the broker-supplied refresh.
  `token-native-refresh/` and `token-native-typed-refresh/` passed. It is still an
  unstable/internal protocol. A handoff during a held active response fails with
  “Fatal error: application network permission was revoked”; its no-handoff
  control completes. Never apply that success claim to unrelated active work.
- SIGHUP rejects new work while draining but racing TUI input exits the client
  with `turn/steer failed: Server is draining`. Separately, an admitted pending
  approval survives drain, executes once and permits replacement. Receipts:
  `final-drain/`. A separate idle unauthenticated restart retained an edited draft;
  the newer managed-account replacement probe above adds account adoption but
  shows pending-approval and active-turn loss.
- Claude's inbox UUID differs from its transcript `promptId`, which matches
  `Stop.prompt_id`. A socket write or unrelated Stop is not completion. Native
  transcripts can lag hooks. `claude-correlation-final/` and `claude-quota/`
  established draft/typed-input survival and correlation. The latest submission
  probes additionally correlate hook-wake retries directly by their prompt IDs.
- Earlier two-client product probes completed retries and edited drafts on second
  logins, but lacked conditional admission, Task binding and pending approvals.
  They exercised the now-removed unconditional sends. Immediate Claude retry used
  cached credentials; expiry after 31 seconds permitted adoption. Cache expiry
  alone reserves no input position. Receipts: `product-codex-structured/`,
  `product-claude-reload/` and `product-*-compress/`.
- Initial product cleanup left 28 detached disposable owners. Those exact owners
  were stopped; corrected cleanup and `product-codex-cleanup/` reaped clients and
  owners. No production process was signalled. The mixed HTTP fixture that counted
  plugin/MCP POSTs as inference is invalid; later probes count `/v1/responses` only.

Earlier focused checks covered 43 selected recovery/routing/CLI tests, the DTO
round trip and Desktop recovery control. Their full commands and limits are in
the git source above. The later six-test focused run and candidate build in this
plan cover the current waiting implementation. Merged main `7cf2d25b3` includes
#1863, #1858, #1865 and #1854; broader merged-tree checks remain with gate.
