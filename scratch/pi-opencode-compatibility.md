# Pi compatibility against OpenCode 2 — 2026-10-17

Jack Heart requested checking the Pi work against the outstanding OpenCode 2
Task so its compatibility requirements are accounted for. This is a comparison
and acceptance update, not approval of Pi's demo or shipping.

## Evidence and scope

- Pi: LOO-507, PR #1591, `806338d501e4d2ac088033764cc0edc1dd48074d`.
- OpenCode: LOO-504, PR #1590, published `285aadc9d46cd21b1e923532a39ca82b214408bc`.
  Its live gate checkout also contains uncommitted native startup-recovery,
  PTY/routing/failure probes, fixture repairs and audit updates. Those were read,
  not changed or treated as delivered. The Task was running `ship` / `gate`.
- Read LOO-504's current Task brief, `scratch/opencode-2-design.md` and
  `docs/reviews/opencode-2/README.md` in its existing checkout, plus the production
  diff and the live `ops/human_session.rs` recovery change. The audit's current
  evidence section supersedes its retained pre-cutover investigation.
- Pi protocol/source: isolated installed package
  `/tmp/loo507-pi/node_modules/@earendil-works/pi-coding-agent`, version 1.3.0;
  `docs/rpc.md`, `rpc-commands.md`, `json.md` and `dist/core/agent-session.js`.
  The previously recorded upstream reference is
  `5ba1cbaf3d96d204d847c84ce235c62dae23e0fd`.
- Prior Pi passing evidence remains valid for its tested paths. The comparison
  below supplies counterexamples and additional acceptance work; no production
  code changed during this investigation.

## Findings that need resolution

1. **Loopflow adds a retry prompt after Pi finishes its own retry policy.**
   `agent/mod.rs::_run_with_transient_retries` handles Pi with the generic outer
   retry loop. A classified transient failure reopens the saved conversation and
   sends `RETRY_PROMPT`. LOO-504 explicitly bypasses this loop for OpenCode because
   a new model-visible instruction is not transparent continuation. Pi already
   owns automatic model retries before `agent_settled`; its current adapter's
   additional retry semantics are neither documented nor verified for side effects.
   A real Pi/local-model probe with native retries disabled and one HTTP 400
   reporting `connection reset` produced **three model requests instead of the
   normal two**, preserved exact resume, and included the synthetic continuation
   prompt. This establishes extra input, not duplicated side effects. Choose and
   test the intended Pi policy explicitly; the conservative repair is to preserve
   final failed/interrupted outcomes and require explicit later input.

2. **An empty assistant response is recorded as successful work.**
   `pi_mapping.rs::Turn::event` checks the assistant-message count and error flag,
   not whether a useful terminal answer exists. Real Pi with an empty text delta
   and normal stop returned **exit 0 with empty output on both initial and resumed
   headless turns**. LOO-504's history reducer requires a nonempty successful
   answer before completing an execution. Add the Pi equivalent at the appropriate
   result boundary, while preserving valid tool-only intermediate messages and
   accounting for any explicitly supported non-text final result.

3. **Automatic compaction can bypass Loopflow context refresh.**
   `context_pi.mjs:30` only sets `compacted = true`. Refresh happens in
   `before_agent_start`. Pi 1.3.0 `_startRun` calls `_prepareRun` once, then uses
   `agent.continue()` after `_handlePostAgentRun`; `_runAutoCompaction` emits
   `session_compact` and can return directly into that continuation. Therefore the
   current hook does not refresh Loopflow context before the first automatic
   post-compaction request. It refreshes on a later new run instead. This is a
   source-confirmed missing boundary, not a completed compaction experiment.
   Exercise overflow and threshold compaction with changed scratch/steers and
   confirm what the next actual model request receives. The existing docs claim
   context delivery after compaction more broadly than this evidence supports.

4. **Live steering is missing from the Pi adapter.**
   Pi exposes `steer`, `follow_up` and prompt `streamingBehavior`, but PiHarness
   inherits `Harness::send_current` returning `NotSteerable`. Work input falls back
   to a later durable boundary. Native queued messages also span a single
   `agent_settled`; treating them as independent completed requests would be wrong.
   Implement and test same-turn steering with independent delivery receipts and
   frozen initiating ownership, or explicitly document this capability as absent.
   Do not infer receipt/result parity from a successful prompt response.

## Acceptance carried over from the OpenCode audit

| Boundary | Pi evidence now | Required next proof |
|---|---|---|
| Launch, identity, exact resume | Unit/launch fixtures; real Pi RPC and non-TTY native probes | Real PTY, configured model, editable prompt, exit/reconnect, same history; supported installed binary |
| Task/Flow and routing | Managed admission and public skill fixtures | Actual composed managed Task/Flow, JSON router, selected branch only, nested `lf` retaining Work/cwd |
| Context and skills | Small Unicode/context fixtures and local-model requests | Large skill/Unicode endpoint markers; scratch changes; manual and automatic compaction; child isolation |
| Ownership and queued input | Frozen turn-origin calls and stale-attachment unit test | Live viewer handoff while tool/turn is active, independent steer receipt, queued next result attributed to its own input |
| Crash, failed-start and subsequent input | RPC crash/auth/missing-binary fixtures | Kill real provider before identity and during tools; preserve failed/unknown capture; explicit later input must not complete old work |
| Usage and history | Cumulative usage tests, recovery test, native settlement fixture | Native crash before settlement; partial usage recovery and repeated observations; tool event recovery with original Process/Work owner |
| Errors and permissions | Fixture auth/model/protocol/extension failures; no provider-only sandbox documented | Real 429/5xx/network/invalid-model cases, native diagnostics off composer, interruption and child cleanup; preserve caller-owned OS boundary |
| Terminal and attention | Native callback fixture and shared Desktop selector | Actual resize with a Unicode draft, edit/submit, blocked dialogs, retries, idle/error state; live handoff without renderer disruption |

Native usage deserves special attention: `pi_history::observe_native` retains raw
`message_end` observations but only creates normalized usage/output at settlement.
A killed native process can have completed paid model steps without any normalized
usage receipt. Raw observations survive; whether recovery restores that partial
usage is unproved. Do not report it as zero or claim that all evidence is lost.

Already explicit limitations need no imitation of OpenCode's architecture: Pi
owns credentials/model selection, has no provider-only sandbox, and lacks managed
account switching, subscription estimates, native output-schema enforcement,
turn limits and cross-harness history import. Pi's stdin/stdout RPC does not need
OpenCode's authenticated HTTP server or SSE parser. Same-Pi continuation remains
the scope; cross-provider parity is not silently added.

## Integration with the outstanding branch

`git merge-tree --write-tree --name-only HEAD 285aadc9d46cd21b1e923532a39ca82b214408bc`
reported conflicts in `rust/loopflow/src/agent/session.rs` and `scratch/questions.md`.
No branch or checkout was merged. LOO-504 generalizes `SessionCommand.codex` to
`SessionCommand.launch`; Pi's native arm must survive that refactor. Shared context,
capture and launch-test files merge textually, which does not establish semantic
compatibility. After LOO-504 lands, sync and run the combined affected suites.

The live LOO-504 gate repair also settles a confirmed-dead provider attachment
before requiring native identity when reopening failed startup. It is shared
Session behavior relevant to Pi; consume the delivered repair and cover Pi's
before-identity failure, rather than duplicating it here.

## Probe receipts and recommendation

`uv run python -I /tmp/loo507-retry-audit.py`: PASS, real Pi 1.3.0/local model,
3 model requests, context/exact resume true, 6 usage receipts,
`synthetic_continue_prompt=true`. This extends the retained RPC probe with one
controlled model error and disables Pi-native retries in the disposable settings.

`uv run python -I /tmp/loo507-empty-audit.py`: PASS as a counterexample, real Pi
1.3.0/local model, 2 requests, both `{exit_code: 0, output_empty: true}`, exact
resume true and 4 usage receipts. No account credentials were used. These temporary
probes should become maintained regressions with their corresponding repairs.

Recommendation: another bounded Pi implementation pass should resolve findings
1–4, retain the relevant runtime proofs, and consume LOO-504's shared launch and
recovery changes once available. Gate must cover the matrix above; authenticated
terminal review and the full affected suites still remain. LOO-507 stays at demo
review with PR #1591 open; this comparison does not select `ship` or approve a demo.
