# Proposal: OpenCode decision authority

2026-09-29 · LOO-298 · Prepared for Jack Heart. Research only. **No executable
patch is proposed for application yet**: the existing protocol adapters do not
establish the native request/turn correlation and before-tool ordering that a
complete repair requires. A caller-token-only patch would hide a second failure.
Main owns all source/build/test/Git work. This contribution changes only this
artifact and `.lf/tmp/opencode-decision-proposal/`.

## System understanding

Inspected branch head `a866926d01d91d98b7e0593010d0cdc74617a186`. The accepted
design, remaining scope, import obligations, native retry options, guide, README
and Infrastructure memory govern this proposal. Existing Exec discovery edits
belong to main and were not changed. No build, test, provider execution, installed
Home access, process control, PM/Git mutation or delegation ran.

### Architecture and data flow

There are **two OpenCode execution paths**, neither currently supplying the
selected native history required by Flow navigation:

| Path | Present behavior | Missing owner conversion |
| --- | --- | --- |
| Ordinary/headless/taskless Flow → `engine/agent.rs::_launch_agent_once` | Only batch Codex takes the harness route. OpenCode spawns `opencode run`; capture claims a Session driver and records the actual provider PID. | No fresh Flow caller token, native start selection or exact native completion. General recorded output remains observation, not native settlement authority. |
| Managed Task → `controller/task/mod.rs::TaskLauncher` → `OpenCodeHarness` | Passes `session_driver`, `flow_selection` and resume identity in AgentConfig; SSE reader maps status/text/tools into ConversationEvents. | OpenCode never consumes the first two fields or writes native Session events. Its server PID registry is operational metadata, not the Session provider-generation record. |

`CaptureHandle::claim_conversation_driver` and `AgentCaller` already own admission
and child ancestry. `FlowTurnSelection` carries the reserved Session, Flow
version/claim, history lower bound and caller token. `store/sqlite/flows.rs`
`require_turn_authority` requires the child Exec's exact Session/provider
generation/token to match the selected native start, with no terminal receipt.
`select_flow_turn` permits replacement only after the previous selected turn
failed/interrupted, clears its candidate, and retains both histories.
`consume_selected_in` records the exact successful completion under the cursor
transaction. Preserve these checks; no second navigation writer is needed.

A related hole matters: `consume_selected_in` returns success when selection is
absent. Thus the fixture's first ordinary work step advancing does **not** prove
native completion ownership. The decision fails earlier because navigation does
require a selection. Adding a token cannot supply the missing history.

### Evidence and counterexamples

The retained hosted log `ci-a866926d0-rust.log:2444–2487` reports source3441,
two original-caller rejections, then a missing decision. Final count is
1,855 passed / 1 failed / 15 skipped / 137 unrun. No reproduction ran here.
`saved_flow_stand_in` at `session_cutover_tests.rs:3189` is a shell script: it
prints `message=created id=ses_$LF_RUN_ID`, optionally exits7, executes the nested
command, and ignores its failure. It emits **no native OpenCode message/turn or
completion**. Its manufactured session name and exit0 cannot prove real native
selection. Retain the intended successful-decision assertion; replace its
transport with OpenCode-shaped evidence, not Codex records or expected rejection.

Other source findings constrain the repair:

- `opencode_mapping.rs:190` creates `turn_<random UUID>` on busy status. It is a
  local display correlation, not an ID recoverable from OpenCode. Idle plus
  substantive content becomes Completed; stream loss creates a local Failed.
  Neither may be copied into authoritative native history without qualification.
- The SSE mapper ignores `message.updated` and step parts, and expects usage on
  status events (`:313`). Existing synthetic idle-usage tests validate that
  assumption; they do not establish native usage transport.
- Direct retry at `agent.rs:1258` only enables conversation continuation for
  Claude/Codex. `_provider_resume_token` omits OpenCode's `sessionID` spelling;
  `build_opencode_command` has no launch/resume argument. All three must agree.
- `engine/stream.rs:139` expects `skill_start/skill_finish` and its fixtures copy
  those names. Current upstream emits `step_start/step_finish`. Merely correcting
  spelling would still mark each step finish as Success, including tool-call
  continuation steps; this cannot serve as whole-request completion.
- `OpenCodeHarness::resume_provider_session` converts every probe failure into a
  fresh conversation. That is explicitly described as an optimization in code,
  contrary to the accepted same-conversation retry requirement. Missing history
  and transport failure must stay distinguishable; neither licenses replacement.
- `lf/commands/flow.rs::recover_native_flow:407` unconditionally constructs
  `CodexConnection`. Once OpenCode starts selecting history, interruption exposes
  this second broken path. Adding an HTTP endpoint to Session without provider
  dispatch would send the wrong protocol during recovery.

## Native protocol evidence

Official [server docs](https://opencode.ai/docs/server/) describe Session/message
APIs, caller-supplied messageID, noReply, asynchronous prompting and message
readback. [CLI docs](https://opencode.ai/docs/cli/#run) document `--session`, JSON
output and server attachment. These are current docs, **not pinned-binary proof**.

Read-only upstream source is pinned to
`7945de208964a49300d7f770d1a71d078db9a4c4`; copies and hashes are private:

- [CLI JSON loop](https://github.com/anomalyco/opencode/blob/7945de208964a49300d7f770d1a71d078db9a4c4/packages/opencode/src/cli/cmd/run.ts#L678)
  emits sessionID and step parts with messageID, but does not forward full
  assistant `message.updated` in JSON mode. It consumes idle internally.
- [Message schema](https://github.com/anomalyco/opencode/blob/7945de208964a49300d7f770d1a71d078db9a4c4/packages/schema/src/v1/session.ts#L454)
  retains assistant id, parentID, time.completed, error, finish and token/cost
  observations. A user message can have several assistant messages/model steps.
- [Prompt loop](https://github.com/anomalyco/opencode/blob/7945de208964a49300d7f770d1a71d078db9a4c4/packages/opencode/src/session/prompt.ts#L1052)
  stores user input before running; noReply returns without model execution.
  Ordinary completion checks the last user's parent relation, finish reason and
  pending tool parts. Busy/idle alone lacks this correlation.
- [Processor](https://github.com/anomalyco/opencode/blob/7945de208964a49300d7f770d1a71d078db9a4c4/packages/opencode/src/session/processor.ts#L424)
  persists step-start/finish and usage against assistant message identity.
  Provider retries inside that processor are distinct from lf relaunching it.
- [Async handler](https://github.com/anomalyco/opencode/blob/7945de208964a49300d7f770d1a71d078db9a4c4/packages/opencode/src/server/routes/instance/httpapi/handlers/session.ts#L311)
  forks prompt work and returns204. This is no proof that lf persisted selection
  before a fast shell tool starts. JSON stdout/SSE ordering likewise supplies no
  acknowledgement from lf's SQLite writer to the provider.

The native Session and message IDs are useful existing identities. A synthesized
busy-interval UUID is not. Whether to correlate the Flow boundary with its exact
user message and all assistant descendants, or a terminal assistant with an
explicit request link, must be established through the actual request/readback
path. Repeated model steps must not become unrelated Flow attempts.

Pinned local binary bytes are identified in `local-binary.json` (path, SHA256,
size); the executable was **not run**, so its version/protocol compatibility is
unknown. Two initial upstream paths returned404 after source layout changes;
the tree lookup located current handlers. No conclusion comes from those404s.

## Tensions

**Before-tool authority versus asynchronous observations.** Spawning a fresh
OpenCode process can carry a fresh immutable caller token, unlike Codex's retained
failed thread. But it does not establish a native start before the first nested
`lf` command. Writing Started before native acceptance, guessing a currently busy
message, sleeping, or accepting today's token would weaken the contract.

**Conversation continuation versus server ownership.** Direct launches and the
current harness spawn their own process; `stop` aborts and kills its owned group.
This is not evidence of Codex's shared-engine/systemError environment defect.
No OpenCode experiment here showed stale environment after same-thread resume.
Conversely, process respawn alone proves neither native history preservation nor
safe attachment to an externally shared server. Never extend group cleanup to a
shared engine to obtain new tool environment.

**Status observations versus native outcome.** Session idle, CLI exit, receiver
disconnection, a completed tool and an assistant finish answer different
questions. Lost transport leaves native completion unknown until exact readback.
Keep raw failure/usage evidence even if another turn later succeeds.

## Recommendations

### One provider adapter, then both launch consumers

The smallest complete direction is to evolve the **existing OpenCode harness**
into the Session-owned request/history adapter and route headless execution
through it too, using the current launcher/event loop. Do not add a parallel CLI
history reducer or substitute `RunCapture::finish` for native completion.
This is a concrete implementation boundary, **not an apply-ready patch**:

| Intended path | Required change |
| --- | --- |
| `harness/opencode.rs` | Consume existing driver/Flow selection; generate a fresh launch token before owned server spawn; publish exact process/Session connection under the driver fence; preserve saved native identity on errors; establish exact request correlation before tool admission; record history before reporting completion. |
| `harness/opencode_mapping.rs` | Separate native message/request history from display busy intervals. Preserve intermediate steps, errors, interruptions and usage; remove invented native-turn assumptions and status-usage dependence. |
| `harness/opencode_history.rs` (only if separation earns it), `harness/mod.rs` | One OpenCode receipt/readback reducer keyed by actual native IDs, used by live and recovery readers. Existing Session events and Flow selection/consumption are the durable owners. No attempt table, fake Exec or separate supervisor. |
| `engine/agent.rs` | Generalize the existing batch harness driver only enough to serve OpenCode while retaining Codex account semantics. Resume the saved OpenCode Session through retries; record each failed/successful native request separately. Direct interactive launch remains a caller to audit, not automatic coverage. |
| `lf/commands/flow.rs` | Dispatch selected-history recovery by recorded provider; feed the same reducer. Exact engine death permits current explicit retry behavior without fabricating the old turn's outcome. |
| `controller/task/mod.rs` | Audit teardown, native error and event delivery through the same adapter; no second Task-only settlement implementation. |
| `engine/stream.rs` | Correct retained direct-CLI wire parsing where still reachable, preserving per-step usage and never treating tool-calls as final conversation success. Delete that OpenCode path if consumer conversion makes it unused. |

Planning estimate only: roughly **+350–650 / −100–220 production lines** across
these surfaces, excluding tests/docs and any new provider hook. It is not a diff
measurement. Exact native admission/turn grouping may materially change it.
Known adjacent callers: direct interactive runs/replay, saved Ask/review connect,
TaskLauncher, shared Flow retry, telemetry/conclusions and process observation.
Their existing ownership must survive; `record_session_event`, driver generation,
`require_turn_authority` and consumed-completion transactions stay singular.

### Resolve one transport question before writing that patch

The smallest distinguishing experiment for main is a disposable, actual OpenCode
binary with a scripted model endpoint: submit a known message, force an immediate
shell decision, and identify a **native-supported acknowledgement** that lets lf
persist the exact request/start/selection before that shell can execute. Record
both event/readback identities and ordering. Repeat after native failure with a
delayed descendant from the old launch. Do not claim success from a fake provider
that voluntarily waits for lf while the real provider has no such contract.

Candidates to investigate, not selected repairs:

- Accepted noReply message followed by same-ID execution could supply identity
  before effects, but the inspected API does not establish an immutable two-phase
  launch contract. Reposting may rewrite/reprocess input; a queued user message
  alone is not evidence of a started agent. Do not mark it Started by convenience.
- Existing permission handling could provide a real before-tool acknowledgement
  if every relevant tool launch is guaranteed to wait and exact message identity
  is available. Prove per-session rule precedence and remembered permissions;
  an auto-allowed bash path would invalidate that guarantee.
- [Plugin docs](https://opencode.ai/docs/plugins/#inject-environment-variables)
  expose shell environment hooks, but the
  [current plugin interface](https://github.com/anomalyco/opencode/blob/7945de208964a49300d7f770d1a71d078db9a4c4/packages/plugin/src/index.ts#L266)
  supplies sessionID/callID, not a native turn. The inspected core bash source
  also marks hook support as TODO. This contradiction prevents treating a plugin
  as an available proven fix. A hook/proxy or changed decision result interface
  is additional machinery and must return to the existing scope discussion.

No narrow patch is justified until that experiment closes correlation/admission.
This is a bounded evidence gap, not a claim that OpenCode cannot support it.
The separate pending decision-interface question remains unanswered; this
research selects neither result-based navigation nor an upstream/provider fork.

## Smallest meaningful behavioral proof plan

Main owns execution; none of these proofs ran here.

1. Preserve the hosted red test's successful Advance, work-step refusal and
   final cursor assertions. Use a scripted OpenCode HTTP/SSE service with real
   Session/message shapes; require selected/consumed SQL references, native
   failures and zero retired Run table. Do not seed Codex history. Test both
   taskless launch and managed Task against that same adapter.
2. Use actual OpenCode plus scripted model for immediate valid decision, failed
   decision then successful replacement, missing successor decision, delayed
   old child, and XOR route. One Session/native history; failed candidate cannot
   survive reselection; only the successor completion is consumed once. Delayed
   child retains its original environment and is explicitly cleaned by fixture
   ownership. Native internal API retry is a separate case from lf retry.
3. Interrupt after native acceptance before delivery, then after native completion
   before SQL persistence. Read back exact messages; retain unknown command/turn
   evidence where appropriate. Repeat recovery without another consumption, no
   duplicate prompt, same title/feedback/native Session. Wrong generation and
   passive observers cannot select or settle. Test failed resume read without
   creating a replacement conversation.
4. Retain multiple assistant/tool steps and distinct failed/successful usage,
   missing versus zero, partial data, model/cost and original input attribution.
   Reordered/duplicated SSE and readback must be idempotent. Assert no successful
   native completion from hollow idle, tool-calls, CLI exit0 or disconnect.
5. Focused Rust suites: `session_cutover_tests::a_taskless_step_records_its_decision_on_the_invocation`,
   the neighboring Task failure/retry/review proof, `harness::opencode`, shared
   selected-history authority/recovery tests, and changed stream/launch tests.
   Then formatting/all-target Clippy. Retain paired Codex stale-child/valid-retry
   results unchanged; OpenCode success cannot discharge Codex's transport gap.

## Handback and limits

No patch, source edit or passing test is claimed. Private `sources.json` records
29 source/context/log hashes; `validation.json` records closing drift, upstream
file hashes and artifact digest. Private source copies are evidence, **never
integration replacements**. The hosted failure is established; direct retry,
SSE identity, completion/recovery and ordering gaps above are source observations.
Configured provider/account, pinned native compatibility, shared-engine behavior,
Desktop and installed acceptance remain unproved. Leave this artifact uncommitted
for main's reconciliation; no Flow edge or Task disposition follows.
