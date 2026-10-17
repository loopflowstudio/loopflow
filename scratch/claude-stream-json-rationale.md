# Why Claude uses stream-json

October 16, 2026. Jack Heart requested the practical, empirical rationale because
this choice has changed over time. This is evidence for LOO-415's implementation,
not a new requirement to retain or remove the mode. The latest accepted resume
semantics are in [Work API direction](loo-415-work-api-direction.md).

## Finding

Stream-json currently earns its place as the programmatic input/result channel
for headless work. It does not itself supply independent lifetime, and it is not
needed merely to have a durable Session or native conversation history. Native
terminal sessions and structured headless sessions can share lifetime guarantees
without using identical provider I/O.

There are three separate choices often bundled together: structured output,
persistent structured input, and routing all headless launches through one harness.
The inspected changes do not establish a measured contest between a retained TUI
and stream-json, nor a requirement that every conversation use stream-json.

## History and what each change addressed

| Evidence | Actual change and motivation | Proof limits |
|---|---|---|
| `6f6590eb82` (#382), February 24 | Claude harness used one `claude -p --output-format stream-json` process per turn, with native resume for later turns. It supplied normalized messages/tools and provider identity to the then-existing session API. | Historical integration design; the old client architecture is not a current requirement. |
| `5e872f06e6`, August 29; design `f7b5ad4d75:scratch/chat-refactor.md` | Added persistent stream input. Follow-ups and steers could enter the same process; queued provider results were coalesced into one runner completion. | Commit reports real-CLI sequential-turn and steering checks. Warm-cache benefit was asserted, without a retained-TUI latency/token comparison. |
| `6016eca305` (#1522), October 11; implementation `5ce7cd5e46` | Routed all headless Claude work through the existing harness. The separate print/stream launch path bypassed correlated turn history, so Flows could not read completed decisions. Added native schema delivery and same-conversation correction. | Fixture proofs cover decision advancement/correction. Historical notes explicitly leave configured Task-to-publication acceptance unproved. This was not evidence that native TUI execution is impossible. |
| `21dae527b4` (#1524), October 11 | Added exact stop-and-native-resume for structured Claude/OpenCode runs. Interrupted turns stay unfinished; saved Session/history survives. | Provider stand-ins, not a configured live-continuity demonstration. Jack Heart's October 16 clarification permits necessary process restart for explicit resume. |

Historical subscription/OAuth constraints explain using the Claude CLI rather
than a different SDK path. Both native terminal and print modes use that CLI;
this does not choose between them. Old SDK/auth claims have not been revalidated.

## Current concrete consumers

- `agent/session.rs::build_session_command` launches ClaudeStream with print mode,
  stream input/output and user-message echoes. The shared options in `agent/mod.rs`
  pass turn budgets and native JSON schemas only to headless execution.
- `harness/claude.rs` writes request UUIDs, sends follow-ups/steers without terminal
  keystroke injection, and waits for the owed results. EOF while work is active is
  failure rather than successful completion. Interrupt currently kills the process;
  a later explicit input can resume native history.
- `harness/claude_history.rs` matches echoed UUIDs to admitted inputs, stores exact
  final or structured output, usage and success/failure, and prevents another input
  from borrowing that answer. `store/sqlite/session_events.rs::input_final_answer`
  reads this correlation for Flow decisions.
- `tests/linear/flow_tests.rs::claude_deciding_step_records_native_answer_and_advances`
  proves advancement and invalid-answer correction through a simulated provider.

The tangible advantages are writable structured input and unambiguous result
boundaries. They are not equivalent to the terminal parser, screen, scrollback,
or native UI. The current harness also carries parser/protocol maintenance,
steer-result accounting, lifecycle fences, and a native-UI transition cost.

## Alternatives and discriminating evidence

Independent October 16 probes now qualify the assumptions below. The installed
Claude CLI running its actual native TUI against a simulated API retained one
process across turns and an unanswered native question. `UserPromptSubmit` and
`Stop` exposed matching `prompt_id` and answer text; a blocking Stop hook also
corrected an invalid decision in the same prompt. Native completion need not
depend on screen scraping. However, `Stop` precedes possible hook continuation,
so it is not definitive completion, and native `--json-schema` did not deliver
the schema while stream-json supplied `StructuredOutput`. These are protocol
experiments, not live-model performance or complete lf reattachment acceptance.
See `scratch/claude-execution-tradeoffs.md` for the independent report and evidence.

Retaining a native Claude PTY would make zero-viewer native UI reattachment direct.
It still needs a trustworthy completion/answer channel for Flow steps, same-input
correlation, schema/correction behavior, usage, and steer delivery without corrupting
typed text. Native hooks/history are candidates, not proven replacements for the
current harness. Provider process exit alone does not identify a completed turn
while an interactive process remains open.

Claude 2.1.302's background attach is another concrete capability, but official
[agent-view documentation](https://code.claude.com/docs/en/agent-view) excludes
print mode and describes supervisor restart/idle-stop behavior. It is not an
automatic fit for the accepted lf owner. OpenCode's existing server has a native
[attach command](https://opencode.ai/docs/cli/#attach); do not generalize Claude's
print-mode limitation to all providers.

The existing skill-invocation benchmarks substitute print/exec for the terminal
case (`scripts/benchmarks/skill-invocation/README.md`), so they cannot settle the
real TUI comparison. No inspected evidence establishes lower tokens, better prompt
cache use, lower RSS, or faster readiness for stream-json versus retained native TUI.

An empirical replacement trial should compare the same initial request and one
follow-up, a steer during work, valid and invalid Flow decisions, explicit resume,
and viewer/creator death. Measure readiness/result latency, process identity,
duplicate submissions, completion attribution, RSS and provider-reported usage.
Preserve model, context, hooks, account and permission policy across the comparison.
Judge whether the replacement deletes enough machinery while preserving behavior;
do not infer model quality or performance from a different renderer.

## Implication for LOO-415

The evidence supports retaining the working stream harness for headless work while
moving its complete execution/capture ownership out of the viewer. Native Sessions
retain their actual terminal. Explicit resume can transition to native history
when necessary, as Jack Heart allowed. A universal mode switch is not required to
unblock LOO-416; it should follow evidence of a simpler working replacement.

Checks: `cargo nextest run -p loopflow --lib --test integration -E 'test(native_echo_and_result_retain_exact_output_without_borrowing_another_request) | test(claude_deciding_step_records_native_answer_and_advances) | test(agent_session_resumes_on_a_fresh_process)'` passed 3/3 with inherited `LF_*` unset and stdin closed; provider fixtures establish correlation, history resume, and Flow advancement/correction, not live-model or retained-TUI superiority. No provider-mode production edit.
