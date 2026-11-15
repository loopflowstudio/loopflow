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

### November 14 revision against current main

Jack Heart:

> is this still relevant. we ve done other work in this area

After the comparison and recommendation to retain interactive recovery while
adapting the implementation to current architecture:

> soudns good. let me know when its ready for review

Continue this Task and PR #1862 to review, not merge. At the comparison,
origin/main was `114ff62176`; the PR head `edee35dd57` was 57 main commits
behind and GitHub reported conflicts. Its earlier demo readiness is stale.

Retain automatic continuation of quota-stopped interactive Sessions on an
eligible login for their selected model, Desktop Recover affected sessions,
truthful progress/waits, drafts, approvals and once-only effects. Main still has
Claude credential switching without continuation and no session.recover action.
The Keychain repair is also absent from main.

Reconcile first, then adapt the useful implementation and tests to current
owners. Do not restore removed per-Session owners/watchers: #1887 introduced
holders, #1890 moved wakeups, #1904 changed headless Work ownership and #1923
unified steers. Keep #1937's refreshed Claude model-capacity semantics; #1906,
#1908 and #1935 own headless agent fallback; #1900 owns sticky provider resume.
These are merged foundations, not duplicate work for this Task. Recheck the
ordinary-typing incident against current input paths rather than claiming the
old fixture established a repair. Existing native results guide regressions;
rerun relevant cases on the revised candidate. Refresh the PR description to
the final remaining scope and stop at demo with current checks and any limits.

The earlier design below describes the published November 9 implementation and
must be reconciled during this pass. Keep production conversations untouched;
use disposable validation and do not install, merge or enable Auto-Ship.

User docs for this outcome lead with what Desktop shows and what to do per
state: `docs/sessions.md#recover-after-a-usage-limit`,
`docs/subscriptions.md#when-an-account-runs-out` (open-Session paragraphs) and
`swift/README.md` (recovery row). Main rewrote all three files since the PR
head; keep these sections through the merge. They name the five Desktop state
labels and the row above the pane, so a relabel or move updates them.
`rust/loopflow/src/harness/README.md#quota-recovery` now describes the
Machine server's connections and holder-preserved terminals.

November 14 reversible choice: keep the bulk recovery button on the row a
person must resend by hand (now `manual_retry`). It can still recheck the other affected conversations;
the reason and guide explain manual continuation for an older client. The
client is never restarted just to install its guard.

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

### Concept review — November 14 (implemented)

Read against `7a67cd6eb6..3b96422baa`. The three concepts, the served
`session.recover` request and the Machine-server ownership hold. One mismatch,
in what the row tells a person to do.

`waiting_native` ("Waiting for native conversation") holds two opposite cases
under one label and one button:

- short waits that end by themselves: reading the turn's history, model not
  yet reported, an earlier attempt settling, a login that could not be prepared
  ("recovery will check again");
- a turn Loopflow will not send again: the client has no recovery guard. Its
  reason ends "continue from the conversation", under a label that says waiting.

The second case also arrives late. Both providers check capacity first, so an
unguarded conversation reads "Waiting for capacity · reset in 2 hours" while
`docs/sessions.md` says "Leave them open", and only when a login frees up does
the row say to continue by hand. Codex's "This Session has no managed login to
recover" stays `waiting_capacity` forever with the button; Claude records the
same missing login as `uncertain`. The stated requirement is to "Distinguish
rejected turns from uncertain delivery/completed effects": a turn known to be
rejected that Loopflow cannot resend is neither a wait nor uncertain.

Each state is now one thing the person does (labels and reasons are
agent-written; nobody chose them):

| State (wire) | Label | The person |
|---|---|---|
| `waiting` | Waiting | Leaves it open. The reason says for what: capacity (with reset), history, model, the earlier attempt, a login being prepared. Button shown. |
| `manual_retry` | Send it again yourself | Sends the message again. The turn was rejected before any work; this client cannot take Loopflow's resend. |
| `recovering` | Recovering | Nothing. |
| `continued` | Continued | Carries on. |
| `uncertain` | Review conversation | Reads the end and continues. Unchanged. |

Rename was in place, no alias: Rust `RecoveryState`, Swift
`SessionRecovery.State`, `server_schema.json`, `session.json` and the new
`session_manual_retry.json` fixture. `manual_retry` is decided at the
rejection, before capacity selection, and the spent limit is still recorded.
Claude's guard is a launch fact (`LF_RECOVERY_GUARD`); Codex reads `hooks/list`
live, so `can_retry` covers both retryable states and the button stays on both.
Claude's missing managed login after a complete rejection is `manual_retry`;
missing model or incomplete chain stays `uncertain`. Docs follow the table.

Counterexamples kept: an unguarded conversation with no capacity anywhere
resends by hand and is rejected again; that is a new failed turn with its own
row. A `manual_retry` row followed by typed input follows the existing rule:
newer input wins and the old turn is dropped. A late wait observation must
still not replace dispatch (`SessionRecovery::observe` treats `waiting` and
`manual_retry` as it treats today's two waits).

Left alone: `lf session recover` takes ids and Desktop chooses them from the
Sessions it has loaded with its own copy of `can_retry`. The served request
could own "every waiting conversation"; no failure was observed, so this
review does not direct it.

### Concept review — November 14, second pass (implemented)

Read against `3b96422baa..0db7a9ea5d`. The five states hold. The row has no
end: `session_recovery` returns the conversation's latest failed turn for as
long as the native thread lives, and Desktop shows a row whenever it is there.

- A person who does what **Send it again yourself** asks gets, within a minute,
  **Review conversation** · "Newer input or incomplete history; continue from
  the conversation", and it stays. Claude's `check_capacity` and Codex's
  `turns.last() != Some(turn)` branch both record `uncertain` for newer input;
  `claude_recovery`'s classification test asserts ManualRetry → Uncertain after
  a new user row. Every conversation open at install is unguarded, so this is
  the first thing the row does.
- Typing over a **Waiting** row does the same. `docs/sessions.md` says "your
  message wins and the old turn is dropped".
- **Continued** stays above the pane through every later turn.
- **Review conversation** after a lost reply stays after the person has read
  the end and carried on.

Direction (agent-written; nobody chose it): the row ends at the person's next
message in that conversation. Each state names one thing to do, and a later
message is that thing done.

- The reading is absent once a turn the person started follows the failed turn.
  Loopflow's own resend (the recovery envelope the gate already classifies, or
  `retry_turn`) is not the person's message.
- Newer input stops being a reason for `uncertain`. That state keeps earlier
  work, incomplete history, and unconfirmed or cancelled dispatch. Split the
  two reasons that currently say "Newer input or …".
- A row that ended is not requested or checked again; nothing is resent for it.
  Dispatch stays in Session history as today.
- `docs/sessions.md` (proposed): after the table, "The row goes away at your
  next message in that conversation." The "old turn is dropped" sentence then
  matches what Desktop shows. `swift/README.md` follows.

Counterexamples to hold:

- An unconfirmed resend that arrives late must not clear its own **Review
  conversation** row; only a turn that is not this attempt's resend does.
- A message queued behind a running resend ("runs after the retried turn")
  starts after `retry_turn`: the row reads Recovering until the resend ends,
  then ends with the queued turn. It must not end while the resend runs.
- A hand resend that is rejected again is a new failed turn with its own row
  (unchanged).
- An older Claude client may report no `started` boundary; the transcript's
  newer user row is the fallback already read by `claude_rejected_model`.

Also settled: one wording for one condition. Claude says "No eligible login is
ready for this model", Codex and `docs/sessions.md` say "has capacity". Use
"is ready" for both: a model resting after its failure has capacity by the
poll and is still not selectable. Update the docs example and `session.json`.

Left alone: the button on a `manual_retry` row, and Desktop's own copy of
`can_retry`, as in the first pass.

### Delete — do not maintain

- Delete both provider branches that classify newer input as uncertain. Keep
  incomplete-history and earlier-work uncertainty; keep dispatch and correlation.
- Delete the server reader's exclusion of continued/undispatched uncertain rows:
  native-history fallback must still notice the next message. All row states
  share the existing check backoff; absent rows schedule no further checks.
- Replace Codex's capacity wording and its fixture. No state alias or migration.

The projection ends rows on a recorded later start when the resend ID is known
(or no dispatch exists). Unconfirmed dispatch without an ID waits for native
history, where the exact envelope or Claude inbox UUID identifies the resend.
A native-history end marker outlives late worker observations. A new failed turn
still owns a new row.

Compression keeps envelope matching on native message content: Claude needs no
synthetic Codex item to identify its resend. The projection returns at an end
marker instead of folding later facts into a hidden row. A recorded start with
an unconfirmed resend ID still retains uncertainty until native history can
identify the input.

### Holder adaptation — November 14 (implementation)

Integrated pinned main `114ff621767dea4525c9f89d638f82567ccab33e`, then
`5fa27860813be006b01212d8bfc703c914388c49`, including #1939's canonical
Claude capacity readings and #1898's planning repair. Both syncs stayed local.


The Machine server owns recovery checks and native side connections; holders
continue to own PTYs. Native boundaries retain the launch's account selection,
guard availability, native home and transcript location. They open no new watcher. One server
reader schedules recoverable conversations; a single attempt per conversation
owns admission and broker work. Codex retains its authenticated side connection
for native refresh callbacks after switching. The same connection follows native
thread changes; a late old-thread login confirmation cannot release its retry.
The shared reader considers live interactive AgentProcesses, not headless workers.
No process per Session returns.
Dispatch remains in Session history, so server restart never resends uncertain
input. Missing launch metadata waits rather than using the server's account
restrictions. Credential preparation must finish save-back even after cancellation.

Deleted with commit 7a67cd6eb6: `harness/claude_updates.rs`,
`harness/observer_measurement.rs` and their fixtures; `CodexObserver` and the
Task-update polling loop; `task_updates::watch` and Pi watcher dispatch;
`record_native_limit`'s unconditional shared-login switch; environment-dependent
interactive route selection. The `printed`/`SessionBoundaryAnswer` TODOs in
`server/wire.rs` are main's LOO-838 compatibility, not leftovers here.

### State replacement — November 14 (done)

The capacity-first no-guard branches and the two old waiting labels are gone.
The Machine server keeps launch metadata for later checks. A failed Codex
`hooks/list` read waits; a confirmed absent hook is manual retry.

Compress pass: Claude's guard is classified once, by `record_limit`. The server
check records the spent limit and returns for a `manual_retry` row instead of
re-deriving it from `client.native`; a missing native home now waits like a
missing inbox. A failed spent-limit write returns the error to the server log
instead of reading "The chosen login could not be signed in". Both providers
write one `wait:` phase; the two new tests use `test_ambient::EnvGuard`.

### Remaining verification

The November 9 native and publication receipts below establish the earlier
candidate, not the holder adaptation. Gate owns the revised candidate's
served/DTO/Swift matrix and disposable native bulk, approval, cancellation and
continued-input cases. Include a Machine-server restart while Codex is using
supplied-token login: retention of dispatch does not establish transfer of
its native token-refresh callback connection. Do not infer that from the
metadata-only reopen test.

PR #1862's body still describes the November 9 candidate ("Existing Session
owners guard submission", 870 selected Rust tests); its checks and wording
predate the holder adaptation. The publishing step rewrites it from the gate's
receipts.

Demo/review retains rendered Desktop, physical keyboard input after reopen,
and installed/real-login behavior. No production switch or installation has
occurred. The reported x/z invisibility, paste revealing xzy, and later Reset
success from LOO-813 still establish no cause or physical-keyboard repair.

Current focused checks (inherited LF_* unset, closed stdin):

- `cargo test -p loopflow --lib --jobs 4 -- harness::claude_recovery harness::codex_recovery session_recovery server::recovery harness::recovery_gate` — 15 passed on the final compress tree, including the five-state lifecycle, later rejection, queued-turn ordering, native-history fallback, an unconfirmed resend start, late dispatch and indexed plans with Machine statistics. Existing large `__eh_frame` linker warning.
- `cargo test -p loopflow --lib --jobs 4 -- harness::claude_recovery harness::codex_recovery` — 6 passed on the final provider tree, including Claude tool-result/meta rows. Continued rows check for later input without repeated settlement writes. Existing large `__eh_frame` linker warning.

Earlier state/Swift/schema checks remain at
`0db7a9ea5dc8801b6c72f9a33f5ee9561a8b6e99:scratch/interactive-limit-recovery.md`,
“Remaining verification”. Gate owns current static analysis, served/DTO/schema,
documentation and broader Swift/iOS checks. The served bulk fixture includes a
manual retry row beside dispatched work; both languages read its DTO fixture.

The gate selection reaches the recovery modules, provider-account/auth, native
hook/context and fact writer, server boundary/wake/recovery, Session projection,
CLI parsing/mutations, DTO/served reads, and their transport/lifecycle callers.
Native bulk, held-lock cancellation, approval and continued-input cases need fresh
receipts on this holder-based implementation; the November 9 passes below are
not reused for them.


November 9 publication, QA and concept-review commands and receipts: `253a13a2580c36539fffabe04f2139e6ebcfa33f:scratch/interactive-limit-recovery.md`, corresponding dated headings. They predate the holder adaptation and require renewed focused checks.

## Earlier native admission and runtime receipts

Detailed November 9 runtime integration, guarded submission, counterexamples and
compression receipts remain at
`3b96422baa6acbe9899cfe350c8e12b4997f9df7:scratch/interactive-limit-recovery.md`,
headings “Runtime integration — November 9” through “Retained implementation
checks — November 9”. The pre-curation working note is also saved under
`.lf/tmp/loo-811-state-curation/interactive-limit-recovery.md`.

Those native checks used synthetic logins and loopback inference. They retain
proof that hook admission can reject stale input and preserve drafts/approvals;
they do not establish the holder adaptation, installed acceptance or real-token
rotation. Notable counterexamples: unguarded Claude wake can run stale input;
Codex `turn/start` can return an existing active turn ID; native hook timeout
allows input unless the shorter recovery watchdog blocks it. The latest guarded
runtime retains one-use admission, broker save-back, cancellation and exact
input-to-completion correlation. Revalidate on the revised candidate at gate.

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
