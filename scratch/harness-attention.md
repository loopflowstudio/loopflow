# Harness attention evidence

October 4 source audit, no configured provider experiment; the full table,
excluded Claude hooks/SDK paths and documentation links are at
`4207263c5:scratch/harness-attention.md`. Jack Heart selected one Waiting
state with provider-specific signals and accepted false positives.

What the Waiting slice reads (`harness/attention.rs`), from streams `lf` owns:

| Provider | Signals | Gap |
| --- | --- | --- |
| Claude stream-json | tool_use blocks and their tool_result; `result` | A tool block can begin while its arguments are generated. No question signal. A native `claude` terminal has no stream. |
| Codex app-server | item start/completion, turn start/completion, approval, requestUserInput and elicitation requests and their answers | Headless `lf` answers every request with a generic accept, so a structured question is never left pending there. |
| OpenCode events | tool part states, question asked/replied/rejected, idle, error | Permissions are answered by the driver and are not questions. A native `opencode` terminal has no stream. The test trace is written from documented shapes, not captured. |

Yield does not prove a question; a disconnect leaves outstanding activity
unknown. The tests prove correlation and state transitions, not semantics.
