# Research: live input and blocking questions in agent runtimes

## System understanding

### Codex

Codex app-server makes the conversation boundary explicit: a Thread contains
Turns, and Turns contain streamed Items. `turn/steer` appends input to the exact
active Turn; queued follow-up input starts later. `turn/interrupt` is separate.
Detached review forks a Thread and runs an independent Turn rather than
injecting review work into the main conversation.

App-server also supports server-initiated dynamic tool requests. The active
tool call waits for the client response, then completes as an Item. This proves
the useful boundary for Ask: request and response belong to one tool call, not
to the conversation's unsolicited-input queue.

Sources:

- https://learn.chatgpt.com/docs/app-server
- https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md

### Pi

Pi names two input queues. Steering is read after the current assistant
response has finished its tool calls and before the next model call. Follow-up
input is read only after the agent would otherwise stop. Its agent loop mirrors
that distinction with an inner tool/steering loop and an outer follow-up loop.

Pi RPC extensions use a separate request/response protocol for dialog UI. A
dialog emits an `extension_ui_request` and blocks until the client returns the
matching `extension_ui_response`. Print mode has no such UI, which reinforces
that interaction is a property of the execution surface, not the skill.

Sources:

- https://github.com/badlogic/pi-mono/blob/main/packages/agent/src/agent-loop.ts
- https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/rpc.md
- https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/sdk.md

### OpenCode

OpenCode separates its server from its clients. HTTP starts or asynchronously
prompts Sessions; SSE carries events. Its built-in `question` tool is the
closest existing implementation to Loopflow Ask:

1. allocate a question id;
2. add a pending entry containing a deferred result;
3. publish `question.asked`;
4. block the tool on the deferred result;
5. expose list, reply, and reject endpoints;
6. return the reply as the tool result.

The pending map is process memory. Shutdown rejects the deferred requests, so
OpenCode supplies the live protocol but not Loopflow's durability or recovery.
OpenCode's independent Sessions and asynchronous prompt endpoint also provide
a natural shape for detached one-question agents.

Sources:

- https://opencode.ai/docs/server/
- https://opencode.ai/docs/tools/
- https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/question/index.ts
- https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/tool/question.ts
- https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/server/routes/question.ts

## Shared pattern

All three systems separate at least two of these concepts:

- input that changes an active conversation;
- input queued for a later conversation boundary;
- a tool request whose matching response unblocks that tool;
- detached work that should not disturb the main conversation.

The portable boundary is therefore above each provider:

- Steer remains durable unsolicited input to the core Project conversation.
- Ask/Answer remains a targeted request/result exchange.
- A fresh one-shot answer agent handles a child Ask without sharing or steering
  the Project provider session.
- Provider-specific live steering is only a delivery optimization. Durable
  boundary delivery remains the semantic fallback.

## Consequences for Loopflow

Do not model child questions as Steers to the Project. Doing so would advance
the Project's input Basis, replay an already-answered question, and couple
response latency to the active Project turn.

Do not inject a provider-specific Ask tool. `lf ask` can be an ordinary blocking
shell command for every harness. Persist the exchange before waiting so shell,
harness, or process failure cannot lose it.

Keep one active Project Run as authority, but give its process two execution
lanes: the serial clarify/pursue/mutate lane and one detached answer worker. The
answer worker receives no Run lease; the Project runner captures its final text
and performs the authorized Answer write.

No durable answer-attempt state is needed initially. One active Project Run
ensures one supervisor, and that supervisor starts at most one answer worker.
If it dies, the unanswered Ask is simply selected again by the replacement Run.
