# Claude execution modes: tested tradeoffs

October 16, 2026. Independent research requested by Jack Heart for LOO-415 and
LOO-416. No production changes or execution-policy decision. Claude Code 2.1.302.

## Finding

A native Claude terminal can work without a viewer and preserve an unanswered
question. Stream-json is not necessary for persistent conversation state, prompt
identity, saved usage, or obtaining exact response text. Those are stronger native
capabilities than the earlier rationale established.

Stream-json still supplies a materially simpler automation contract: caller-chosen
input UUIDs echoed back, explicit success/failure results after hooks finish, and
native schema output. Replacing it everywhere needs a real completion adapter,
not just a PTY launch flag. The evidence supports the existing per-conversation
owner topology and a native-from-start option for conversations intended to be
opened later. It does **not yet support switching every Flow step to native TUI**.
That launch-policy choice belongs with LOO-416's lifecycle work; this research
changes neither Task's scope.

## Experiments

[Probe script](evidence/claude-execution-tradeoffs/probe.py) runs the installed
Claude binary in disposable directories against a local fake Anthropic API.
Native cases use a real PTY and **never substitute print mode**. Independent
config, dummy local credential, empty external settings/MCP, and small simulated
responses avoid account changes and inference charges. Hooks and API bodies are
retained in each result; [summary](evidence/claude-execution-tradeoffs/summary.json)
includes binary hash, PIDs, history fields and final process observations.

Run from this checkout:

```sh
uv run python scratch/evidence/claude-execution-tradeoffs/probe.py native
uv run python scratch/evidence/claude-execution-tradeoffs/probe.py stream
uv run python scratch/evidence/claude-execution-tradeoffs/probe.py native-question
uv run python scratch/evidence/claude-execution-tradeoffs/probe.py native-schema
uv run python scratch/evidence/claude-execution-tradeoffs/probe.py stream-schema
uv run python scratch/evidence/claude-execution-tradeoffs/probe.py native-correction
```

| Probe | Observation |
|---|---|
| Native and stream, two sequential inputs | Each retained one PID and native Session. Second model response was deliberately held for three seconds; it completed when released. Native UserPromptSubmit and Stop matched per-prompt UUIDs and exposed exact response text. Stream additionally echoed the caller's input UUID; hook prompt_id was a different identifier. |
| Native unanswered question | Real AskUserQuestion UI waited about seven seconds with no viewer. Enter answered Blue; the next API request contained a tool_result for the **original tool_use_id**. Same PID, Session and prompt_id completed. [Evidence](evidence/claude-execution-tradeoffs/native-question-result.json). |
| Native `--json-schema` | Flag was accepted, but no schema or StructuredOutput tool reached either API request. Arbitrary prose completed. [Evidence](evidence/claude-execution-tradeoffs/native-schema-result.json). |
| Stream with identical schema | Added StructuredOutput, accepted its simulated tool result and emitted exact structured_output with success. [Evidence](evidence/claude-execution-tradeoffs/stream-schema-result.json). |
| Native correction hook | First Stop carried INVALID_DECISION. Its hook blocked stopping; Claude requested another response in the **same prompt**, then emitted valid JSON with stop_hook_active=true. [Evidence](evidence/claude-execution-tradeoffs/native-correction-result.json). |
| Native history | Saved user entries retain promptId; assistant entries retain token usage. Usage values here are fixture-supplied, not measured inference consumption. |

The native owner was this experiment's Python controller, continuously draining
the PTY; there was no viewer process. This proves no-viewer native execution and
preservation of the original question. It **does not prove client detach/reattach,
screen/mode/scrollback restoration, creator death, input fencing, SSH, or lf's
actual transport**. Enter was written directly to the retained PTY. The initial
native attempt hit its disposable API-key confirmation and produced an auth
failure after declining; the corrected probe explicitly accepted its dummy local
key. No actual credential was displayed or read.

## The remaining native automation problem

A Stop callback is a candidate response boundary. Another hook may continue work;
recording every Stop as successful completion would settle the correction above
too early. Interruptions do not emit Stop, and asynchronous transcript writes can
lag callbacks. Current official [hook documentation](https://code.claude.com/docs/en/hooks)
also describes prompt_id, StopFailure, exact last_assistant_message, continuation
caps and background work. A native adapter needs a trustworthy **post-hook**
completion/interrupt boundary and a mapping from lf's admitted input to the native
prompt UUID. Neither terminal quiet nor process exit supplies that boundary.

Native hook correction is possible, as tested, but is not the same as provider
schema enforcement. A replacement must validate, correct, bound failed correction,
and preserve other installed hooks without false completion. Stream results already
supply that boundary to `harness/claude_history.rs`. Hooks/history give a promising
alternative, rather than a proven drop-in replacement.

Steering also needs an honest input channel: structured stdin avoids editing the
person's terminal draft. Existing lf native context hooks at UserPromptSubmit and
PostToolUse offer another route; live steering during long work, queued inputs,
and interruption were **not** tested here. No conclusion that native steering is
impossible follows.

## History and competing approaches

`5e872f06e6` explicitly replaced per-turn print spawns with persistent stream input
for live send_current and coalesced results; its warm-cache claim had no retained-TUI
comparison. `f7b5ad4d75:scratch/chat-refactor.md` explains CLI subscription routing,
which does not distinguish native CLI from print CLI. `5ce7cd5e46` fixed headless
Flow decisions bypassing correlated native history. These are real requirements,
but not proof that every persistent conversation should start in print mode.
The [earlier rationale](claude-stream-json-rationale.md) retains the source map.

Claude's own `--bg`/`attach` is native attachment, not a print-mode conversion.
Its shared supervisor stops idle workers, can restart unexpected exits, and
has its own supervisor-loss behavior. Foreground-to-background migration can
interrupt/restart work. These [documented behaviors](https://code.claude.com/docs/en/agent-view)
conflict with the accepted lf lifecycle; no shared Claude supervisor was launched
or adopted in this experiment.

## Decision evidence still needed

For native-ready conversations, the next useful proof is the real lf owner:
viewer and creator loss while a native question is pending, fresh client attach,
restored terminal state, then answer that original request with fenced control.
For native-only Flows, add exact input attribution, post-hook completion, interrupt,
invalid-decision correction and live steering before deleting stream-json.

No live-model quality/token/cache comparison or restart-latency measurement was
performed. First-response timings include deliberate sleeps and cannot be quoted
as startup measurements. One-point CLI RSS samples were roughly 288 MiB native
and 196 MiB stream; differing default permission modes and tiny simulated turns
make these observations unsuitable for capacity forecasts. Real account, context,
skill and hook fidelity must be matched before a practical cost comparison.

Necessary restart remains a valid fallback under Jack Heart's direction. Retaining
native execution from launch is better for continuity where that UI is the desired
future surface; the outstanding question is how much automation machinery must be
replaced to make that the universal default.
