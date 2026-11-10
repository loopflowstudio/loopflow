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

## Guarded native submission — November 9, latest pass

Both native clients now demonstrate submission ordering in isolated probes.
Automatic product recovery remains unfinished; no runtime sending path was added.
The probes use synthetic logins, loopback inference and network isolation, not
installed-app or real-token acceptance. Earlier unsafe replacement/drain results
remain applicable; they no longer justify saying Codex has no admission mechanism.

**Claude 2.1.322:** a self-contained shell wrapper invokes the recovery gate only
for its envelope, with native `onFailure: "block"`. An independently failing
ordinary context hook does not stop typing. Gate exit 1 and native timeout both
block stale wake before model execution, retaining the edited draft and later
completion. Escape cancels the gate, but native async wake queues itself again:
claiming the attempt once and rejecting its duplicate preserves the draft and
prevents execution. Inbox cancellation also preserves the draft. The first
cancel-once fixture accidentally counted the observer as gate admission; its
receipt is superseded by `guard-cancel-consumed`, which enters the gate first.

Guarded inbox delivery works as well as async wake. With credentials prepared
inside native submission, input submitted at the preparation or dispatch barrier
follows the retry. Requests use first → second → second → second; the inbox UUID
joins to `promptId` and the matching Stop, then the edited draft completes.
Stale validation now reads the native transcript **inside** the held gate; it
rejects newer completed input before changing credentials even when ordinary
context hooks fail. This is no longer the external history-read admission scheme.
The synthetic Keychain write still does not exercise broker refresh/activation.

**Codex 0.167.0:** `UserPromptSubmit` runs for observer `turn/start` input. Supplied-
token login inside that hook completes the rejected turn on the second login;
the native TUI, thread/model and editable draft remain. Input submitted while
the hook is held before login or before release completes after the retry.
With newer work already active, native queues recovery until that work finishes;
the gate rejects stale input without installing credentials. A pending approval
remains answerable and executes its original command once before stale recovery
is rejected. The guarded approval case uses native transcript history with no
ordinary observer hook, and reports no interrupted turn.

A crucial receipt distinction: `turn/start` during existing work returns that
work's turn ID. Its later successful completion does **not** establish recovery.
The probe now requires the recovery input itself in the completed turn. Product
correlation must bind the admitted hook's turn/input, not just the RPC reply ID.

Codex native hook timeout allows recovery through. A recovery-only shell watchdog
with a shorter deadline can return a blocking JSON decision instead; the timeout
and cancellation probes retain drafts and complete subsequent input without a
retry request. Its first two versions failed because `status` is read-only in
zsh; `gate_status` fixes that measured wrapper error. The watchdog remains a
probe: cancellation/partial-write handling with the real broker is unimplemented.
A watchdog must not kill a worker between receiving and storing a rotated token.

Receipts: `.lf/tmp/loo-811-protocol/`:

- Claude: `guard-{failure,timeout,cancel}-consumed`, `guard-handoff-preparation-final`,
  `guard-native-history-stale`, `inbox-guard-preparation-final`, `inbox-guard-cancel`;
  the native-history inbox dispatch case is `inbox-guard-native-history`.
- Codex: `codex-submit-{baseline,active-final,approval,preparation-final,dispatch}`,
  `codex-submit-native-history-approval`, `codex-submit-native-timeout`,
  `codex-submit-watchdog-final`, `codex-submit-cancel`,
  `codex-submit-failure-final`, `codex-submit-guarded-preparation`. Failed setup/control
  receipts remain alongside them; they are not product passes.

Checks (inherited LF_* removed by `scripts/test_network.py`):

- `uv run python scripts/test_network.py uv run --offline python scratch/claude_retry_correlation_probe.py --claude /Users/jack/.local/bin/claude --admit-recovery --guard-recovery <case flags> --output <receipt> < /dev/null` — the eight retained gate/inbox cases above passed; Keychain items removed where used.
- `uv run python scripts/test_network.py uv run --offline --script scratch/codex_drain_probe.py --codex /Users/jack/.local/bin/codex --submission-gate <case flags> --output <receipt> < /dev/null` — eleven retained cases above passed, including the expected native-timeout counterexample. Initial fixture failures remain in their own receipt directories.
- Saved-receipt assertions — 19 cases checked; all native clients/servers and HTTP workers reaped, no processes referring to the 30 recorded private-root spellings. `.lf/tmp/loo-811-protocol/guarded-submission-checks.json` retains the result.
- Ruff check/format, `git diff --check`, `lf context` — passed. No Rust/Swift runtime changed; gate still owns broader suites and product acceptance.

References read November 9: [Claude hooks](https://code.claude.com/docs/en/hooks#userpromptsubmit)
(describes prompt blocking and `onFailure`) and [Codex hooks](https://learn.chatgpt.com/docs/hooks)
(describes prompt blocking; no documented `onFailure` field). Native behavior
above was measured, not inferred from those references. Pre-pass uncommitted notes
are saved in `.lf/tmp/loo-811-before-guarded-submission/`.

## Earlier admission counterexamples — November 9

The guarded probes above supersede earlier implementation directions, not these
measured failures. Full commands, setup failures and observations remain at
`c5b22af319508841fe822609ad4dd6bae8efa64f:scratch/interactive-limit-recovery.md`,
under “Native replacement and submission checks” and “Prior native admission
investigation”. Pre-compression uncommitted sources are saved in
`.lf/tmp/loo-811-before-probe-compression/`. Receipts below are under
`.lf/tmp/loo-811-protocol/`; all used private homes, synthetic credentials and
loopback inference with external egress denied. Native children and HTTP workers
were reaped; scoped Keychain items were removed.

- Claude 2.1.321 `StopFailure.asyncRewake` retained drafts but woke stale input
  after newer typed input completed at both held barriers: requests rose from
  two to three. No credentials changed. The original observer missed retry
  prompt IDs and reported no correlated Stop; the later submission hook supplies
  that correlation. Receipts: `rewake-{baseline,preparation,dispatch}/`.
- Claude 2.1.322 submission hooks rejected stale wakes before credential writes
  and ordered valid retries before input submitted at preparation/dispatch.
  Valid handoffs used first → second → second (plus a second-login queued turn
  in race cases). **Without the recovery guard**, hook exit 1 admitted a stale
  wake after newer input: requests rose from two to three and a correlated Stop
  followed. Correlation alone establishes execution. Receipts:
  `rewake-submit-preservation/`, `submit-handoff-{baseline,preparation,dispatch}/`,
  `submit-stale-dispatch/`, `submit-failure-dispatch/`. Failed fixture controls
  remain at `rewake-submit-block/` and `rewake-submit-block-final/`; one restored
  its deliberately blocked text, another waited for a Stop that blocking prevents.
- Codex 0.167.0 `turn/steer.expectedTurnId` rejected the quota-failed turn with
  `-32600`, `no active turn to steer`; rejected the old ID during newer active
  work; accepted the newer active ID; then rejected the failed ID again after
  completion. Drafts and later typed input completed; no credential work was
  attempted. Receipt: `conditional-steer-final/`.
- Direct Codex server replacement adopted the second managed login in the idle
  rejected-turn case, preserving thread/model/draft and completing retry plus
  edited input (first → second → second). A newer held response was interrupted
  and its connection cut. A pending approval was lost: `y` became new input,
  executing no original command. Input submitted during downtime stayed editable
  and completed only after resubmission. Receipts:
  `replace-{baseline,preparation,approval,disconnected}/`. The disconnected
  receipt's `draft_submitted_intact: false` checks another case's marker; its
  completed input is `TYPED_WHILE_DISCONNECTED_AFTER_REPLACEMENT`. This proves
  editor retention, not transparent delivery.

The [Codex app-server reference](https://learn.chatgpt.com/docs/app-server) and
installed generated schema describe an active-turn condition on `turn/steer`,
not a failed-turn condition on `turn/start` or login. Its
[model-request hooks](https://learn.chatgpt.com/docs/hooks#premodelrequest-and-postmodelrequest)
are diagnostic. The [Claude hooks reference](https://code.claude.com/docs/en/hooks#command-hook-fields)
describes async wake on exit 2. Sources read November 9; native behavior above
was measured, not inferred from documentation. Earlier Ruff, saved-receipt and
context checks passed; no runtime sends were integrated by those passes.

## Delete — do not maintain

The unsafe sending paths are removed: Codex recovery's `account/login/start`
and `turn/start`; Claude recovery's shared activation, cache-expiry sleep and
inbox send. Keep unguarded sends removed. Native submission ordering is now demonstrated;
only a guarded path with bounded broker work and cancellation handling can replace
them. No disabled copy, second credential owner, outside history read or lf lock
substitutes for native admission.
Historical probes stay as counterexamples, including successful unconditional
sends that did not protect racing input.

The old Claude `Recovery.detached` mode and duplicate transcript read are gone.
Detached capacity work remains separate from polling/completion; a capacity wait
cannot delay observing a dispatched attempt's completion. Retain broker refresh,
normal Task inbox delivery, manual account switching, Graph recovery and all
attempt/completion facts.

The remaining implementation has no predecessor deletion targets. This pass
removes duplicate submission-gate drivers in the Claude probe and duplicate
wait loops in the Codex probe. Codex completion now joins the native submission
hook's turn to its exact recovery input; a successful RPC reply's turn alone
cannot establish continuation. Keep one gate driver for inbox and async wake,
including failure, timeout, cancellation and both input-ordering barriers.

Compression checks, November 9 (all native commands below use
`uv run python scripts/test_network.py`, then `uv run --offline`, stdin closed):

- `python scratch/claude_retry_correlation_probe.py --claude /Users/jack/.local/bin/claude --admit-recovery <case flags> --output .lf/tmp/loo-811-protocol/<receipt>` — five passed: `--async-rewake --guard-recovery --handoff --gate-race preparation` (`compress-guard-preparation`); `--quota --guard-recovery --handoff --gate-race dispatch` (`compress-inbox-dispatch`); `--async-rewake --guard-recovery --gate-cancel` (`compress-guard-cancel`); `--quota --guard-recovery --gate-timeout` (`compress-inbox-timeout`); `--async-rewake --gate-failure --race dispatch` (`compress-unguarded-failure`). The last intentionally reproduced stale execution without a guard. Drafts and later typed input survived all five; both valid handoffs ordered retry before queued input on the second login.
- `--script scratch/codex_drain_probe.py --codex /Users/jack/.local/bin/codex <case flags> --output .lf/tmp/loo-811-protocol/<receipt>` — three passed: `--submission-gate` (`compress-submit-baseline`); `--submission-gate --guard-recovery --approval` (`compress-submit-approval`); `--conditional-steer` (`compress-conditional-steer`). Baseline completed the hook-correlated retry; approval executed once, kept its login and rejected stale recovery; conditional steer retained its failed-turn counterexample and passing active control.
- Saved-receipt/cleanup check — eight cases, zero processes referencing their private roots, native clients/servers and HTTP workers reaped; scoped Keychain items removed. The first tally counted `/tmp` and `/private/tmp` aliases twice; normalizing them yielded eight roots. Receipt: `.lf/tmp/loo-811-protocol/compress-probe-checks.json`.
- `uv run python -m py_compile scratch/{claude_retry_correlation_probe,codex_drain_probe}.py`, Ruff check/format, `git diff --check`, `lf context` — passed. Runtime Rust/Swift is unchanged; broader checks remain at gate. These probe passes do not complete broker integration or Task-bound native/Desktop recovery.

### Retained implementation checks — November 9

Full commands/results: `c5b22af319508841fe822609ad4dd6bae8efa64f:scratch/interactive-limit-recovery.md`,
“Compression checks for this revision” and “Retained implementation checks”.
All Rust commands ran with inherited LF_* unset.

- Focused Session/Claude/observer checks: six passed in each compression; candidate builds passed. The final parser test was marked leaky once, then passed alone without that report; no subprocess path was found. Retain the unexplained report for gate. Logs: `.lf/tmp/loo-811-compress-{final,admission,watchers}-test.log`, `loo-811-compress-final-leak-check.log` and corresponding build logs.
- Task-bound `served_bulk_recovery_keeps_task_identity_and_dispatched_attempts`: passed after repairing the missing CLI-spelling match arm; memberships, primary Sessions and Process count stayed unchanged.
- Two-client native wait probes passed for both providers at `admission-wait-{codex,claude}/` and `compress-watchers-{codex,claude}/` under the protocol directory. Drafts remained editable and typed input completed, with no retry input or credential handoff; clients/owners were reaped and scoped Keychain items removed. These were unbound synthetic Sessions, not automatic or real-login recovery.
- Formatting, diff and context checks passed; DTO fixtures were unchanged. Gate owns the broader merged-tree suites/clippy.

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

The outcome still requires **both** providers. A demonstrated provider mechanism
can be integrated while the other remains unresolved; unsupported cases keep
their wait or uncertainty. New input either wins with no old retry or follows an
admitted recovery without credential replacement failing its request.

1. Claude: integrate the demonstrated recovery-only submission wrapper, one-use
   attempt and inbox correlation through the existing watcher and Session facts.
   Credential activation belongs inside admission. Broker work needs bounded,
   cancellable lock acquisition and a token writer that survives cancellation
   long enough to store any received refresh grant. Failure or partial native
   installation blocks the prompt and keeps a truthful wait/uncertainty. Current
   native failure/timeout/cancel tests cover the gate, not broker integration.
2. Codex: integrate native `UserPromptSubmit` ordering through the existing
   observer/broker, with recovery-only failure rejection. The native deadline
   itself fails open; the measured shorter watchdog rejects its child timeout.
   Broker refresh cannot be that killable child. No login happens before native
   admission; stale work keeps the original login, active request and approval.
   Correlation comes from the admitted native hook, since the `turn/start` reply
   can name earlier work. Cancellation during credential preparation and final
   dispatch, partial writes and missing history still need integration checks.
3. Retain dispatch/completion facts across reconnects and subsequent rejections.
   Keep history, selected model, drafts and pending approvals; leave uncertain
   delivery intact and execute completed tool effects and Graph steps once.
   Extend the Task-bound served/Desktop path to two simultaneously rejected native
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
is unfinished. Remaining guarded broker and Session-owner integration is substantial work,
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
plan cover the retained waiting implementation, not the latest merged tree.
Main `6f7a6c9ca` is merged at `c5b22af31`: it retains #1863, #1858, #1865 and
#1854 and adds #1866's Desktop Task launches, #1871's observed live reads and
#1870's Codex service tiers. Recovery must retain reconnect's current tier and
use the existing served reads/Session owners. Gate owns checks against this merged
tree, including Desktop recovery updates through the changed subscriptions.
