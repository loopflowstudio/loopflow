# Open Task Sessions after a usage limit

November 9, 2026, demo conversation. Jack Heart:

> i have still had the issue that if i have a bunch of task sessions open and they all hit a usage limit i need to restart them all. can we automatically do soething or at least add something in hte desktop so you dont need to go to an obscure cli command

The reported outcome remains unsatisfied. No demonstration or approval of
LOO-811 occurred. During the first demo preparation its candidate binary was
absent and installed lf reported `0.15.2+7e90dafb1.dirty`. A later demo request
started a local candidate build; no installation, account switch or Session
restart was performed.

The first handoff went to LOO-813. Jack Heart then corrected the placement:
"813 was marked complete so move that" and "to 811", enclosing the three targets
below. LOO-811 now owns this recovery outcome. Leave LOO-813 complete; its merged
PR #1863 is prior work, not completion of automatic quota recovery. PR #1862's
two-item Keychain repair is only part of LOO-811's outcome.

- Automatically continue affected Sessions when another login has capacity.
- Show Recover affected sessions in Desktop, with progress and anything still waiting.
- Keep history, drafts and approvals intact.

Desired experience: recover the affected open conversations together when an
eligible login has room; otherwise expose a visible Desktop recovery action and
explain what is waiting. Retain each conversation's history, model selection,
drafts and pending requests. A shared credential switch alone does not establish
that a stopped message continued. Automatic retry must distinguish a rejected
turn from uncertain delivery or completed tool effects.

Jack Heart's November 9 continuation direction:

> Automatically continue affected open Sessions on another eligible login for the selected model; expose Recover affected sessions in Desktop with progress and waiting states. Preserve native history, drafts and pending approvals. Distinguish rejected turns from uncertain delivery/completed effects and never replay finished Graph steps. Account switching alone is not acceptance. Inspect Claude and Codex; the report's provider is unspecified.

The original provider remains unidentified; both providers are in scope. The
initial brief/title sync timed out. November 9 `lf task status LOO-811 --json`
now returns the expanded title and direction, open PR #1862 and the current code
Graph at realign; it does not report title/brief delivery status. The earlier
local candidate build (`cargo build -p loopflow --bin lf`, inherited LF_* unset)
passed in 1m 45s. No disposable demo was launched.

## Replacement design — November 9, draft

Implementation is incomplete. The November 9 documentation remains the intended
experience; it is not implemented by the account switch in #1862. The recovery
owner must change before automatic retry or Desktop can truthfully report success.

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

**Design revision:** provider-generation replacement is not ready for integration.
It requires a native client that retains rejected input and stays connected/rejoins
when its server drains, or a demonstrated managed-account reload that needs no
replacement. Neither exists as a proven path here. Changing installed Codex,
putting lf in the input path, or reopening a dead client from history would not
meet this run's constraints. Loopflow receipt/lifeline changes cannot repair that
native loss. The automatic recovery and Desktop implementation remain dependent
on resolving this counterexample; publication cannot claim them complete.

### One recovery owner

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
and successful activation. A dispatch receipt precedes the write. Native receipt
correlation, not a successful socket write, advances recovery. No retry follows
an uncertain write; a later successful native turn may establish continuation.
A turn with earlier tool effects stays intact for review instead of resending its
original input. Pending requests remain with Claude.

Codex keeps the native client, transport owner, selected thread and editor while
its private provider changes generation. The existing attachment/lifeline/exit
owners must perform this as one handoff, retaining the old provider's exit and
pointing quit at the successor before acknowledging recovery. Reuse the native
endpoint and saved thread configuration; do not rebuild settings from an old
launch or replace native history. Pending approvals/tools, unknown delivery and
unproven quiescence wait with their original provider. An idle observation alone
never authorizes shutdown. The native replacement probe makes this design worth
pursuing; it does not yet establish that handoff contract.

Native graceful drain supplies admission but the racing-input probe above
invalidates using it with the installed terminal client. Pending approval
survival alone does not close that gap. Claude also needs a correlation rule
linking its retry dispatch to the native successful turn; the existing inbox
write supplies no such acknowledgement. These are technical design gaps, not
a choice to narrow the recovery outcome.

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
- Replace `harness/codex_connection.rs::Recorder::attachment_moved`'s unconditional
  client termination only for an acknowledged same-Session provider-generation
  handoff. Keep real moves, closes, lost authority and native exit cleanup.
  Do not add a second provider
  registry or overwrite the prior AgentProcess's PID/birth history.
- No deletion of model moves, native-input retention, pending-approval handling,
  Graph limit recovery, Keychain reconciliation, or #1863/#1839 reconnect/input
  behavior. They remain required behavior, not fallback implementations.

These runtime cuts await their surviving recovery path; neither replacement is
implemented. Compression removed the command fixture's arbitrary `history` file:
keeping unrelated bytes unchanged cannot test native conversation continuity.
Credential retry, rotation, marker and third-item assertions remain. The recovery
guide lives in `docs/sessions.md`; subscriptions and Desktop docs link there.

### Preservation cases and remaining implementation

1. Resolve the demonstrated Codex drain/client incompatibility before the
   generation handoff. The prerequisite is native rejection handling that keeps
   input and the client alive, or proven managed-account reload without restart.
   Then implement the handoff through existing lifecycle owners and retain
   coverage of racing input and answerable approvals. Closing the original
   client must settle exactly the current provider. The probe is a failing
   acceptance case, not an implementation of recovery.
2. Add turn-correlated Session recovery observations and once-only dispatch.
   Cover rejected-before-work, earlier tool effects, missing events, lost reply,
   reconnection, stale attachment, `/new`, and a repeated limit. Never treat an
   empty or partial turn snapshot as proof no work occurred.
3. Move Claude and Codex detection, route selection and continuation onto that
   owner; cover distinct models and restricted logins, simultaneous reports,
   failed/partial activation, no capacity, capacity reset and model changes.
4. Add served bulk retry and Desktop progress from those observations. Validate
   several open Sessions, including drafts and approvals, through successful
   continuation and a subsequent typed turn. Keep completed Graph steps/effects
   counted once and Session/Task/native history identities unchanged.
5. Gate affected lifecycle, transport, provider-account, hook/decoder, Session
   command/projection, DTO and Swift view suites once after implementation.
   Native managed-token and installed two-item observations in live-recovery.md
   remain demo/review work. Publication waits for the whole recovery outcome.

This replaces the assumption that existing `resume --account` plus a Desktop
button can meet the expanded scope. The recovery implementation and the provider
handoff need review together; neither the current PR nor the native probe is
acceptance of the expanded outcome.
