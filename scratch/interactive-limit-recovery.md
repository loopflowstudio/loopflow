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
a second request for dispatched work, and starts no Process. Separate native
fixtures now exercise automatic admission and pending approvals through the served
protocol. None of these fixtures mounts Desktop.

### Remaining verification

Both providers now use the guarded runtime envelope and broker integration.
Unsupported cases retain their wait or uncertainty. New input either wins with
no old retry or follows an admitted recovery. The two-Task served/native bulk,
held-lock cancellation and approval cases pass; they do not replace gate or demo.

1. Publication checks passed after the fixture repairs below. Runtime admission,
   restricted routes, distinct models, repeated reports, hold expiry, lost/late
   installation replies and retained attempts have focused coverage; the affected
   lifecycle, transport, hook, projection, DTO and Swift checks also ran.
2. Demo/review: rendered Desktop, physical keyboard input after reopen and the
   installed/real-login observations below. No production switch or installation
   occurred in the disposable checks.

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

### Publication review — November 9

Review repaired required nullable recovery decoding in Rust and Swift, including
encoding nulls and completing inline/screen/server fixtures. Added a disposable
Codex socket/store test for cancellation after login is sent: late success pins
the new account, error/lost replies retain uncertainty, and none releases or
resends the retry. The route test now covers repeated reports and hold expiry.
No retry algorithm changed. `lf sync` merged cleanly; remote main remained
`8155d1fcde597154ee9746ad72280bf6687a7da8` at the later publication read.

All commands used inherited LF_* unset and closed stdin; Rust used stable 1.99.1
and canonical TMPDIR. Full commands/filters are in the named local logs.

- `scripts/test.py --reuse-passing --rust-filter` (affected selection; `.lf/tmp/loo-811-expanded-publish-gate.log`) — architecture, 654 Python + one separate observation check and 73 website checks passed. Initial Rust compile caught the new test reaching a private type; moved it into that type's unit tests. The 801-test Desktop run found one absent recovery key in a screen fixture; completed screen, inline and server fixtures.
- `cargo nextest run -p loopflow` (same selection plus context/schema/screen callers; `.lf/tmp/loo-811-publish-rust.log`) — 869/870 passed; the only failure was generated schema example key order. Regenerated with `target/debug/lf server describe --json`; the focused schema rerun passed. Existing grace-expiry transport test passed in 63.394s; no timeout or leak. New cancellation, repeated-limit and expiry tests passed.
- `cargo clippy --all-targets --jobs 4 -- -D warnings` — passed; `.lf/tmp/loo-811-publish-final-checks.log`. Existing large `__eh_frame` linker warning occurred in test linking, not clippy.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'DTOFixtureTests|sessionRecovery|uncertainRecovery'` — 59 passed after fixture repair; same final-check log. The full build also reported main's existing nil-coalescing warning in TaskWorkView.
- `scripts/check_swift_multiplatform_boundaries.py`, `cargo fmt --check`, `git diff --check` — passed. `scripts/third_party.py refresh` updated the Cargo.lock input hash; Python inventory tests passed.

Prior six-case native QA remains applicable: production edits in this review
change DTO decoding/encoding, not native dispatch or credentials. Rendered
Desktop, ordinary physical typing after reopen, installed and real-login checks
remain for demo. This publication neither merges nor completes the Task.

### QA — November 9, merged candidate

[qa.html](qa.html) records the public CLI walkthrough, headless Desktop checks
and six supplemental native scenarios on `16efb4f8e.dirty`, SHA256
`473e1d7353201b8a6ca85633c056ac74ee29926fc608eedadb41481ab282f963`.
No runtime defect surfaced. The first private-Machine walkthrough omitted an
agent preference; adding `-a codex` corrected the probe. Empty-state/errors,
reopening a prepared Session created by installed `28b0af568`, and recovery
reads across a private-server restart passed without provider login or seeded rows.

- `cargo build -p loopflow --bin lf < /dev/null` (inherited LF_* unset) — passed.
- `uv run python scripts/test_network.py uv run --offline python .lf/tmp/loo-811-qa-expanded/public_cli.py < /dev/null` — passed; prepared metadata only, no native conversation.
- `scripts/test_desktop.sh -Xswiftc -gnone --filter 'sessionRecoveryControlKeepsUncertaintyVisible|DesktopHeadlessTests' < /dev/null` (inherited LF_* unset) — 17 passed, including recovery-button tap, uncertain state and Work navigation; automated checks, no mounted window.
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --<provider> /Users/jack/.local/bin/<provider> <case flags> --output .lf/tmp/loo-811-qa-expanded/<provider>-<case> < /dev/null` — six passed: both providers with no flag, `--approval`, and `--cancel`. Native versions: Codex 0.167.0 and Claude 2.1.322. Synthetic logins/local responses only; both bulk cases continued each Task Session once, kept drafts and completed later input. Approvals executed once after recovery; held-lock cancellation prevented retries and account changes.
- `uv run python .lf/tmp/loo-811-qa-expanded/check_receipts.py` — six matching candidate receipts, seven successful-scenario roots removed, zero remaining probe processes, nine scoped Keychain items absent. The failed setup's root is also absent.
- `git diff --check`, `lf context` — passed after the report and skill edits; both context budgets fit. Broader suites and clippy remain at gate.

QA changed no runtime code. Publication checks are recorded above; rendered
Desktop, physical typing and installed/real-login recovery remain unverified.

### Concept review — November 9, after runtime integration

No design change or open product choice. Keep the Session, rejected native turn
and derived recovery reading; the attempt stays in Session history and credential
ownership stays with the broker. The Desktop action requests a check, while native
admission orders recovery against newer input. An account installation, RPC reply
or unrelated completed turn cannot establish continuation. Cancellation after an
installation was sent can change the login without continuing the request; retain
that distinction and uncertainty rather than offering another automatic send.

The earlier late-wait and unguarded-input mismatches have implementation and
focused checks. Remaining failures of acceptance are observations to obtain, not
new product decisions: gate covers the matrix above; demo covers rendered Desktop,
ordinary typing after reopen and configured live behavior. Loop-or-next can
advance. No runtime code changed in this review.

`scratch/concept-review.html` now shows the current interaction and those limits.
Usage wording in `docs/sessions.md` and the harness/subscription/Desktop docs
matches the guarded continuation design; no replacement usage or skill workflow
is proposed. The full preservation matrix remains necessary for acceptance.

## Runtime integration — November 9

The native submission guards now run through the existing Session owners and
account broker. A private, one-use socket binds Session/thread/rejected turn/
attempt. The native hook holds it open; cancellation or expiry stops subsequent
installation and retry release. An installation already sent to Codex can still
take effect; its confirmed reply updates the route without releasing cancelled
admission. Partial or unconfirmed installation stays uncertain. Broker
refresh/save-back runs outside the hook's process group and retains rotated
credentials. Native credential and Claude
refresh locks poll cancellation; a partial Keychain write is stopped and remains
uncertain. Ordinary hooks still fail open. Clients lacking the loaded recovery
guard wait rather than receiving unguarded input.

Codex prepares tokens beside its observer, then installs them on that observer's
connection. Its route follows a confirmed installation even if retry admission
closed meanwhile. A successful `turn/start` reply proves nothing: completion
requires the admitted native turn and exact envelope input. Claude expires its
30-second credential cache inside admission, activates through the shared broker,
and correlates inbox UUID, admitted prompt ID and Stop.

Checks (inherited LF_* unset; synthetic native endpoints only):

- Candidate CLI build and eight focused guard/cancellation tests passed; the two additional hook-trust/Claude correlation tests passed. Logs: `.lf/tmp/loo-811-{guarded-runtime-build,guard-cancellation-tests,guard-correlation-tests}.log`. Existing large `__eh_frame` linker warning remains.
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --<provider> /Users/jack/.local/bin/<provider> <case flags> --output .lf/tmp/loo-811-protocol/<receipt>` — six passed: `runtime-{codex,claude}-served` (no flags), `runtime-codex-cancel-native` and `runtime-claude-cancel` (`--cancel`), `runtime-codex-approval` and `runtime-claude-approval-retained` (`--approval`). Both Task primary Sessions keep their owner, cwd, model and native history; one correlated retry each, editable drafts and later input. Rechecking through Desktop's server operation does not resend. Escape while the credential lock is held prevents both native retries and account changes; edited drafts complete on the original login. Each provider's pending approval survives the other Task's recovery, then executes its effect once. All fixture clients/owners reaped and scoped Keychain items removed.
- Earlier whole-fixture failures remain under `runtime-guard-*`: binding after launch generated Task-update turns; newer Task input correctly retained an uncertain Claude attempt; the next fixture lacked checkouts. `runtime-guard-claude-checkouts` continued both but wrongly assumed automatic primary selection. `runtime-codex-cancel` used an absent native executable. All cleaned up; later fixtures create linked checkouts and select their bound primary Sessions before quota errors.

The successful Claude bulk, both cancellation and Codex approval receipts name
candidate SHA256 `b71b3d61f4769cf7a672234a176b3808bcb5fb54fb40b48ce54180962b41c839`.
Codex bulk used the preceding candidate; subsequent changes close a consumed
listener, reap the watchdog, serialize pending attempts and join the broker on
observer exit. The final review also found a blocking Keychain stdin write before
its deadline loop; input now writes nonblocking under the same cancellation and
timeout, killing/reaping the writer on failure.

- `cargo test -p loopflow --lib cancelled_keychain_install < /dev/null` — both blocked-stdin cancellation and partial-write/retry tests passed; `.lf/tmp/loo-811-keychain-cancel-write-tests.log`. Final `cargo build -p loopflow --bin lf` passed; `.lf/tmp/loo-811-guarded-runtime-final-build.log`, SHA256 `19267dfe1becbf2b77db2d73d9b7aa2d995b95532623e1aee8318bc4a49f6add`.

Claude's first approval fixture (`runtime-claude-approval`) used native auto mode,
so its harmless command ran without an approval. Explicit default mode retained
the approval across recovery (`runtime-claude-approval-default`), but the fixture
matched an unrendered terminal string whose spaces were cursor movements. Neither
whole fixture passed; both cleaned up. The corrected check joins native tool-use
and tool-result IDs, then requires the answer to execute its effect exactly once;
`runtime-claude-approval-retained` passes on final candidate `19267dfe1bec…`.

- Saved-receipt/cleanup check — six native cases pass, zero remaining fixture processes and all six scoped Keychain items absent; `.lf/tmp/loo-811-protocol/runtime-cleanup-checks.json`.
- `cargo fmt --check`, `uv run ruff check scratch/native_recovery_probe.py`, `uv run ruff format --check scratch/native_recovery_probe.py`, `git diff --check`, `lf context` — passed; scratch and memory fit their targets. Broader checks/clippy remain at gate.

Desktop's served mutation/projection is exercised without claiming a rendered
Desktop demonstration. The ordinary-typing incident and installed/real-login
limits stay in `live-recovery.md`. No production accounts, Sessions or installed
application were changed by these checks. Infrastructure memory now reflects the
integrated guards, measured preservation cases and remaining acceptance limits.

## Guarded native submission — November 9, mechanism pass

This earlier pass demonstrated submission ordering in isolated probes.
It added no runtime sending path; the integration above is subsequent work.
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
zsh; `gate_status` fixes that measured wrapper error. At that pass the watchdog
remained a probe; broker cancellation/partial-write handling came in the integration above.
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

## Compression checks — November 9

The unconditional sending paths are removed. Native submission owns admission;
the existing broker owns credentials. Historical probes retain counterexamples,
including successful unconditional sends that did not protect racing input.

Runtime compression separates Claude capacity selection from guarded dispatch,
and Codex broker preparation from its observer loop. One transcript decoder
serves both providers; the test-only Keychain writer wrapper is removed and its
tests use the production login writer. Completion still requires the admitted
input, and failed admission still retains dispatch. Pre-pass uncommitted sources:
`.lf/tmp/loo-811-before-runtime-compress/`.

- `cargo test -p loopflow --lib recovery < /dev/null` (inherited LF_* unset) — 28 passed; `.lf/tmp/loo-811-compress-runtime-tests.log`. Existing large `__eh_frame` linker warning remains.
- `cargo test -p loopflow --lib keychain_ < /dev/null` (inherited LF_* unset) — 7 passed, including blocked stdin, partial installation/retry and both-item switching; `.lf/tmp/loo-811-compress-keychain-tests.log`.
- `cargo build -p loopflow --bin lf < /dev/null` (inherited LF_* unset) — passed; `.lf/tmp/loo-811-compress-runtime-build.log`, SHA256 `21dd464633268c528d094ca7ef2d3180ff195e56b05d42b9cae6b8cb6aa263b1`.
- `uv run python scripts/test_network.py uv run --offline --script scratch/native_recovery_probe.py --lf target/debug/lf --<provider> /Users/jack/.local/bin/<provider> --output .lf/tmp/loo-811-protocol/compress-runtime-<provider> < /dev/null` — both providers passed: two Task primary Sessions, one retry each, retained identity/model/drafts and subsequent input. Synthetic logins and served Desktop protocol only; approval/cancellation cases retain the earlier integration receipts.
- Saved-receipt/cleanup check — both cases pass; zero fixture processes, temporary roots removed and all three scoped Claude Keychain items absent; `.lf/tmp/loo-811-protocol/compress-runtime-cleanup.json`.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; memory and scratch fit both targets. Gate owns broader suites and clippy; rendered Desktop, ordinary typing and installed/real-login checks remain open.

Compression checks, November 9 (all native commands below use
`uv run python scripts/test_network.py`, then `uv run --offline`, stdin closed):

- `python scratch/claude_retry_correlation_probe.py --claude /Users/jack/.local/bin/claude --admit-recovery <case flags> --output .lf/tmp/loo-811-protocol/<receipt>` — five passed: `--async-rewake --guard-recovery --handoff --gate-race preparation` (`compress-guard-preparation`); `--quota --guard-recovery --handoff --gate-race dispatch` (`compress-inbox-dispatch`); `--async-rewake --guard-recovery --gate-cancel` (`compress-guard-cancel`); `--quota --guard-recovery --gate-timeout` (`compress-inbox-timeout`); `--async-rewake --gate-failure --race dispatch` (`compress-unguarded-failure`). The last intentionally reproduced stale execution without a guard. Drafts and later typed input survived all five; both valid handoffs ordered retry before queued input on the second login.
- `--script scratch/codex_drain_probe.py --codex /Users/jack/.local/bin/codex <case flags> --output .lf/tmp/loo-811-protocol/<receipt>` — three passed: `--submission-gate` (`compress-submit-baseline`); `--submission-gate --guard-recovery --approval` (`compress-submit-approval`); `--conditional-steer` (`compress-conditional-steer`). Baseline completed the hook-correlated retry; approval executed once, kept its login and rejected stale recovery; conditional steer retained its failed-turn counterexample and passing active control.
- Saved-receipt/cleanup check — eight cases, zero processes referencing their private roots, native clients/servers and HTTP workers reaped; scoped Keychain items removed. The first tally counted `/tmp` and `/private/tmp` aliases twice; normalizing them yielded eight roots. Receipt: `.lf/tmp/loo-811-protocol/compress-probe-checks.json`.
- `uv run python -m py_compile scratch/{claude_retry_correlation_probe,codex_drain_probe}.py`, Ruff check/format, `git diff --check`, `lf context` — passed. That earlier probe-only pass changed no runtime Rust/Swift; the subsequent integration and runtime compression checks above establish the narrower product results.

### Retained implementation checks — November 9

Full commands/results: `c5b22af319508841fe822609ad4dd6bae8efa64f:scratch/interactive-limit-recovery.md`,
“Compression checks for this revision” and “Retained implementation checks”.
All Rust commands ran with inherited LF_* unset.

- Focused Session/Claude/observer checks: six passed in each compression; candidate builds passed. The final parser test was marked leaky once, then passed alone without that report; no subprocess path was found. Retain the unexplained report for gate. Logs: `.lf/tmp/loo-811-compress-{final,admission,watchers}-test.log`, `loo-811-compress-final-leak-check.log` and corresponding build logs.
- Task-bound `served_bulk_recovery_keeps_task_identity_and_dispatched_attempts`: passed after repairing the missing CLI-spelling match arm; memberships, primary Sessions and Process count stayed unchanged.
- Two-client native wait probes passed for both providers at `admission-wait-{codex,claude}/` and `compress-watchers-{codex,claude}/` under the protocol directory. Drafts remained editable and typed input completed, with no retry input or credential handoff; clients/owners were reaped and scoped Keychain items removed. These were unbound synthetic Sessions, not automatic or real-login recovery.
- Formatting, diff and context checks passed; DTO fixtures were unchanged. Gate owns the broader merged-tree suites/clippy.

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
round trip and Desktop recovery control; full commands and limits are in the git
source above. They predate guarded runtime integration. The latest pre-sync
candidate is `21dd464633268c528d094ca7ef2d3180ff195e56b05d42b9cae6b8cb6aa263b1`;
its recovery/Keychain tests and two-provider bulk checks are recorded above.

The November 9 realign read of `git ls-remote origin refs/heads/main` returned
`8155d1fcde597154ee9746ad72280bf6687a7da8`, merged at `da782f592`.
It adds #1868's observer changes, #1869's typed board/Task/Session-card reads and
#1872's late Task-creation reply repair to the earlier merged work. Sync repaired
the observer-measurement fixture's recovery callback; its focused test passed
(command in `live-recovery.md`). No broader merged-tree check has run. Gate covers
these changed subscriptions and observer paths alongside the recovery matrix;
reconnect retains the selected model and service tier.

`git diff --check`, `lf context` — passed after realign; both budgets fit.
Documentation only; existing results reused, broader checks remain at gate.
