# Research: native retry options

2026-09-29 · For Jack Heart and LOO-298. Read-only research; no executable edits, tests, native experiments, process control, shared-state mutation, or Git mutation. Main retains implementation ownership. Loopflow HEAD inspected: `24ea61517cc5cbcfc4cf29452364fe6efcd37123`; concurrent migration/scratch edits were not evaluated.

## System understanding

### Architecture and data flow

`engine/agent.rs::_launch_with_transient_retries` resumes the same native conversation after a transient failure. `harness/codex.rs::start_inner` creates a fresh caller token for each Flow launch and puts it in `LF_AGENT_CALLER` through `thread/start` or `thread/resume` configuration. `codex_history.rs::History::record` correlates a new native start notification with its reply, records its origin, and selects it on FlowSession. `store/sqlite/flows.rs::require_turn_authority` compares the child Exec's Session, provider generation and token with that exact selection, rejecting already-completed turns. Successful native completion is consumed separately.

`CodexHarness::stop` detaches a Session driver without killing its engine; it does not explicitly acknowledge `thread/unsubscribe`. `CodexConnection` fences writes, while passive resume strips configuration overrides. Flow and conversation claims remain separate. A passive subscriber can therefore affect native reload eligibility without possessing Loopflow write authority.

### Evidence and counterexamples

All local result paths below are under `.lf/tmp/execution-model/`; these are inspected retained results, not fresh runs. Real Codex 0.157.1 used synthetic Responses and private Homes.

| Evidence | Observation | Limit |
| --- | --- | --- |
| `supervisor-late-decision-retry-3/results.json` | Old delayed child returns 0 after retry selection; retry makes no decision, yet Flow completes. Candidate `33be09c8…`. | Demonstrated stale-child acceptance before token repair. |
| `native-turn-caller-late/results.json` | Old child returns 1; Flow waits without a verdict. Candidate `d2791ad2…`. | Rejection alone cannot establish working retry. |
| `native-turn-caller-replace/results.json` | Same candidate rejects legitimate successor Advance; provider succeeds, Flow remains unresolved. | Paired repair is red. |
| `native-idle-resume-config-1/results.json` | Failed thread reports `systemError`; unsubscribe/resume with generation 2 still executes tools with generation 1. | Filename does not establish idle state. |
| `native-interrupt-resume-config-1/results.json` and Cut log | Interrupting the completed failed turn returns `no active turn to interrupt`; probe stops before resume. | No evidence about a later resume. |
| `native-success-resume-config-1/results.json` | Successful thread reports `idle`; unsubscribe/resume retains thread/engine and tool environment changes 1→2. | No shared-sibling or failed-thread repair proof. |

The retained 0.157.1 schemas expose `ThreadResumeParams.config`, but no general environment/config override in `TurnStartParams`, `ThreadSettingsUpdateParams`, or `TurnSettingsUpdateParams`. `DynamicToolCallParams` does carry native `threadId`, `turnId`, and `callId`; `TurnStartParams.outputSchema` offers another transport. These fields establish API shape, not successful Loopflow integration.

Current [official app-server documentation](https://learn.chatgpt.com/docs/app-server#unsubscribe-from-a-loaded-thread) describes delayed unloading after the last subscriber and an inactivity period; unsubscribe alone is not immediate reload. Its dynamic-tool section documents `thread/start.dynamicTools` and `item/tool/call` requests. Current docs are not a pinned-binary guarantee.

Fresh read-only upstream inspection at commit `c248f6d48b97eb4a2aa56147a0b11b7d763278b9` corroborates the earlier source explanation: [thread_processor.rs:4336–4385](https://github.com/openai/codex/blob/c248f6d48b97eb4a2aa56147a0b11b7d763278b9/codex-rs/app-server/src/request_processors/thread_processor.rs#L4336) compares resume overrides, requires direct-input eligibility, no subscribers, `ThreadStatus::Idle`, and non-Running agent status, then calls `wait_for_thread_shutdown`, removes only that thread, and cold-resumes after confirmed shutdown. Otherwise overrides are ignored. This is current upstream source, **not** verified source for the installed 0.157.1 bytes. An attempted `rust-v0.157.1` source URL returned 404; no conclusion about release availability follows.

## Tensions

- Caller evidence must identify the originating turn, rather than whichever turn is current when a child finally starts. Looking up today's token, accepting either token, or minting another driver generation without changing inherited evidence restores the original defect.
- The existing shell command interface needs fresh per-turn provenance; the native protocol already knows turn identity, but exposing another decision transport changes integration and prompts.
- Thread-local reload preserves unrelated engine siblings in design, but another subscriber to the **same** thread blocks the current reload path. Silently detaching passive clients is an ownership change.

## Options and implementation cost

Estimates below count implementation surfaces, not measured effort or promised completion dates. Every option retains Exec/AgentSession/FlowSession and their existing histories; none is selected here.

### A. Extend upstream failed-thread reload eligibility

**Change:** In the linked resume-overrides branch, permit terminal `SystemError` alongside `Idle`, retaining direct-input eligibility, no subscribers, non-Running status, and successful `wait_for_thread_shutdown` before removal. Add failed/successful/subscribed/running cases to `app-server/tests/suite/v2/thread_resume.rs`. Loopflow would explicitly acknowledge unsubscribe on its owned connection before reconnect/resume with the new token; retain `require_turn_authority` unchanged.
**Cost/ownership:** Small upstream predicate change plus focused native tests; small Loopflow detach sequencing change. Requires upstream acceptance and a usable provider release, not a local provider fork. No engine replacement or new product owner. Still conditional on no other subscriber to this conversation; it is not a universal connect-with-observers repair.
**Proof:** Failed-thread reload must change tools' token, retain both turns/thread/Session and usage, reject the old child, accept successor once, and preserve an active sibling on the same engine. Same-thread subscribers and shutdown timeout must not start a duplicate conversation. Current evidence supports this candidate but proves none of those repaired behaviors.

### B. Use native dynamic-tool identity for Flow navigation

**Change:** Register a narrow decision/router tool in `build_thread_request` via `dynamicTools`; enable the experimental API; dispatch `item/tool/call` before codex.rs's current catch-all approval response. Validate its exact thread/turn against recorded selection under the Session driver fence. A real child `lf flow decide/route` can receive that selected turn's token and reuse the existing CLI/store check; retain its actual Exec and parent. Never select a token from Session identity alone.
**Cost/ownership:** Medium: request dispatch, narrow child launch, registration/resume behavior, prompt adaptation and native tests. No new daemon or generic command proxy is necessary, but this is a new provider-specific tool interface requiring scope approval. Existing resumed conversations cannot be assumed to gain tools: pinned `ThreadResumeParams` has no `dynamicTools` field. The current shell path remains insufficient for legitimate retries unless callers switch to this interface.
**Proof:** Old-turn tool requests and delayed shell children reject; successor native tool succeeds; contradictory choices, router behavior, duplicate requests, driver handoff and sibling dispatch retain existing semantics. Test registration persistence across retry/restart and previously created threads. This is a feasible alternative transport, not a proven drop-in repair or authorization for a tool proxy.

### C. Consume a structured verdict from the selected successful turn

**Change:** Supply a decision/router `outputSchema` in `CodexHarness::send_input`; persist the turn-correlated final agent message in Session history; consume its validated verdict/path with exact successful completion in the existing Flow settlement transaction. Extend native history recovery to retrieve the selected turn's output after driver loss. Refactor the existing decision validation rather than adding a parallel navigation reducer.
**Cost/ownership:** Medium to large: harness, history/recovery, reducer, prompts and provider parity. It avoids new shell provenance and leaves sibling engines untouched. It changes the decision contract from an in-turn CLI write to a completion result; it needs an explicit product decision. Disabling shell navigation for that boundary must be deliberate, or two writers remain. It does not repair original-turn attribution for arbitrary shell descendants.
**Proof:** Failed and earlier outputs never settle; successful retry output settles once; missing/malformed/contradictory output stays unresolved; driver-loss readback retains exact output, attribution and usage. `outputSchema` availability alone is not a result-path proof.

### D. Replace only an exclusively owned engine

**Change:** Wire automatic retry to the existing exact process evidence, graceful-stop/confirmed-exit path and provider-generation replacement, then resume the same native thread/account with fresh configuration. `prepare_native_retry` currently recovers/releases selected work after proven engine exit; it does not authorize killing a live engine. `CodexHarness::stop` deliberately detaches instead.
**Cost/ownership:** Medium for a proven exclusive engine, larger for trustworthy exclusivity. Retained both-dead recovery supports conversation restoration, not this live replacement. A shared engine cannot be killed; keeping the failed loaded thread while opening the same thread elsewhere risks competing writers. Making all engines private would change accepted sharing/ownership and does not repair existing shared engines.
**Proof/verdict:** Pair the original tests with exact shutdown and generation attribution, plus an unrelated live sibling that remains untouched. Conditional fallback only, not a complete repair under the current shared-engine contract; not authorized by this research.

A further upstream alternative is native per-command turn provenance: export a native turn ID when spawning tools and compare it with `selected_start.provider_turn` at Exec admission/navigation. It avoids reload and same-thread subscriber restrictions. Unlike a snapshot of thread config, it must cover every shell/tool launch and retained terminal reuse; the recorded environment probe did not expose `CODEX_TURN_ID`. Exact provider implementation sites and coverage were not established here. This is a broader upstream API change, not an available local fix.

## Quality, potential, and open questions

The existing selection/history check is the reusable core; no new attempt object is needed. The main hotspot is the mismatch between per-turn authority and per-thread tool configuration. The harness also currently treats every server request as approval, so dynamic tools require real dispatch work. Direct native identity is latent protocol capability, while structured results trade interface compatibility for fewer child-control dependencies.

No inspected evidence proves a purely local, unchanged-interface repair that works with shared engines and same-thread observers. That is an evidence gap, not impossibility. Archive/unarchive, rollback/revert, dummy successful turns, elapsed waiting, explicit `--retry` without changed evidence, and accepting today's token are not selected repairs: they respectively alter history/visibility, add work, or fail to establish origin. All-provider parity and configured account continuity remain unproved.

## Smallest next action

Main should present **A's exact upstream predicate/test change as an upstream-only repair proposal**, explicitly retaining its no-other-subscriber limitation, before approving more Loopflow machinery. Review the paired stale-child/valid-retry test plus one shared-engine sibling as the single acceptance gate. Do not repeat the already-negative unsubscribe experiment on unchanged provider bytes. If an upstream dependency or same-thread observer limitation is unacceptable, the concrete decision is whether to authorize B's native tool interface or C's completion-result contract; neither is silently selected. No upstream issue or patch was sent here.

## Inspected source fingerprints

SHA-256; Rust paths relative to `rust/loopflow/src`, schemas relative to `.lf/tmp/execution-model/supervisor-codex-schema`. Hashes describe inspected bytes, not tests or a whole-tree receipt.

```text
956525be82bc13072d9c08c4c86fe5ebc797a54fb0b4b451f03a438efff37a9c  harness/codex.rs
1ffc08be0df486462814a695286590bde8caacb182ee86f81d17077b309ce8e6  harness/codex_history.rs
11df1ea1103ccf86259206e2b500d0c681612540d698fccccad7f79ee3c6a80d  harness/codex_connection.rs
95697244c3fcd157340fae889f10b9478620e8159218924f525adc0487d73402  engine/agent.rs
3a9890c92ed3c3b1dec9b7b1eaf1f681541e75388f70c5d5c04f81cbc123ebfb  store/sqlite/flows.rs
c2b001c1443d12a8c3dd9bd0405f37efe8efabb20efd634d068e29648b6eb8ed  exec.rs
401bba20cfbd95762bef0467d840430c46be53369093ad9f26425ba757e34efc  DynamicToolCallParams.json
07771223642e1b61bd9aac0069fc0f98143a1c047724ca02c7ceb13653442738  v2/TurnStartParams.json
cc5bb3b25f82073d24af5b6c09e4804ac307467f8400ba5f263fa86f9f0349e6  v2/ThreadResumeParams.json
786c0549e5c1b57421e03472f3b8c3519090849fb8401163f3da5cc2c55b66be  upstream thread_processor.rs
ca67555394168f6b5b1164290907d0539b0d857974b802e75d79615a21ccb5cf  upstream tests/suite/v2/thread_resume.rs
```
