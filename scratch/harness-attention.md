# Harness attention evidence — October 4, 2026

Source audit only; no configured provider experiment. Jack Heart selected one
Waiting state with provider-specific signals and accepted false positives.
The implementation contract and 120-second fallback are in
[the design](focus-on-your-own-work.md) under "Later slices".
Original research, including excluded Claude hooks/SDK/permission-host paths,
remains at `9c6866e8e:scratch/harness-attention.md`.

Activity, mode and attention are independent. Yield does not prove a question;
native history is weaker than an owned protocol stream. Disconnects leave
outstanding activity uncertain. Test event correlation and state transitions,
not semantic certainty.

| Provider | Usable evidence and gaps from the audit |
| --- | --- |
| Claude binary stream-json | Message/tool blocks, correlated results and final `result`. A tool-use block can begin during argument generation, before execution. Current mapping has no shared pending-input lifecycle. No hooks, SDK or permission host in this cut. |
| Codex app-server | Turn/item start/completion plus approval, requestUserInput and elicitation requests; resolution includes cancellation. Item start can precede approval. Audit found generic accept replies inadequate for structured questions; handle distinct schemas. Passive native history cannot assume this stream. |
| OpenCode | Tool parts distinguish pending/running/completed/error. Question asked/replied/rejected events expose input; idle alone proves neither question nor success. Current mapping reconstructs turn completion from assistant receipts without pending tools, ignores idle/status as completion, handles permissions but lacks shared question attention. Automatic permission replies must not imply waiting for Jack. |

Inspect `harness/{claude,codex,opencode}.rs` and corresponding `_mapping.rs` and
`_history.rs` modules. `chat/types.rs::ConversationEvent` lacks first-class input
request/resolution events; `ops/human_session.rs::session_attention` infers Reply
from completed interactive turns and marks Flow reviews before preparation.
Attached-client Active is insufficient evidence for Waiting.

Original documentation sources (not fresh installed-version verification):
[Claude CLI](https://code.claude.com/docs/en/cli-reference),
[Codex app-server](https://learn.chatgpt.com/docs/app-server),
[OpenCode server](https://opencode.ai/docs/server/), and OpenCode's
[question](https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/opencode/src/question/index.ts)
and [status](https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/opencode/src/session/status.ts) implementations.
