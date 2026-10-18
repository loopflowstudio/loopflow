# Pi compatibility against OpenCode 2 — 2026-10-17

Jack Heart requested comparing LOO-507 / PR #1591 with LOO-504 / PR #1590.
This comparison does not approve Pi's demo, shipping, or Task completion.
The full original investigation and counterexamples are retained at
`3a0b71d455:scratch/pi-opencode-compatibility.md`; the first published Pi
implementation was `806338d501e4d2ac088033764cc0edc1dd48074d`.

## Findings and disposition

- **Synthetic retries:** Real Pi 1.3.0 with native retries disabled made three
  requests after a controlled HTTP 400 `connection reset`, including Loopflow's
  synthetic continuation. This proved extra input, not duplicated side effects.
  Pi now bypasses the outer retry loop; only explicit later input continues after
  final failure, preserving Pi's internal retries and exact conversation.
- **Empty success:** Initial and resumed turns returned exit 0 with empty output
  against the local model. Settlement now requires nonblank text with `stop`;
  tool-only intermediate messages and recoverable internal retries remain valid.
- **Compaction:** Source review found automatic compaction could continue without
  `before_agent_start`. Request projection now refreshes context after compaction;
  the real overflow probe sees changed scratch on the first recovered request.
  Real manual and threshold compaction passed in the October 18 gate.
- **Steering:** The adapter previously inherited `NotSteerable`. Direct RPC now
  distinguishes queued, rejected and uncertain receipts with frozen turn identity.
  Public Work steering uses the launch extension and existing provider callback
  receipts because the shared lifecycle has no production `send_current` caller.
  A real local-model probe proves same-turn delivery with an independent emitted
  receipt. The October 18 gate continuation also proves real native viewer handoff
  during a model call, retaining provider ownership and same-turn public steering.
- **Partial usage:** Native callbacks previously normalized usage only at
  settlement; raw message observations survived crashes. Assistant-message
  checkpoints now retain cumulative usage before settlement. Focused fixtures
  cover a crash and duplicate observations; real RPC/native crash probes now
  cover two explicit resumptions preserving prior usage and conversation identity.

Protocol baseline: Pi 1.3.0, upstream
`5ba1cbaf3d96d204d847c84ce235c62dae23e0fd`; installed RPC, extension and session
format documentation and implementation supplied the evidence. No credential
or authenticated-provider proof is inferred from these isolated model scenarios.

## Shared integration and acceptance

OpenCode's shared `SessionCommand.launch` and before-identity startup recovery
landed in `3316a5061357d452c097a22cae15b8fccf1ef8d9`, consumed by merge
`614c23816b`. Pi retains its native launch arm and covers failed-start reopening
through that shared owner. The original merge-conflict warning is resolved.

`scripts/benchmarks/pi/README.md` owns current reproducible proofs and remaining gate/demo
checks. The acceptance boundaries carried from OpenCode remain:

- Native launch, configured model, editable prompt, exit/reconnect and exact history.
- Managed Task/Flow routing, selected branch only, nested `lf` Work identity/cwd.
- Large Unicode skills, changed scratch, manual/automatic compaction, child isolation.
- Active viewer handoff and independent steer receipts; later inputs own later results.
- Before-identity/tool crashes, child cleanup, explicit recovery without completing old work.
- Partial usage and tool-history recovery with original Process/Work ownership.
- Real 429/5xx/network/invalid-model errors, interruption and truthful attention.
- Unicode edit/resize, blocked dialogs, retries, native diagnostics and rendered review.

Pi keeps its own credentials and model selection. No provider-only sandbox,
managed account switching, subscription estimates, response schemas, turn limits,
or cross-harness history import is implied. Pi's RPC needs no OpenCode HTTP/SSE
transport. Personal account decisions remain in `pi-authentication.md`.
