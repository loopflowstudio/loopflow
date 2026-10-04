# Harness attention evidence — October 4, 2026

Source audit for Jack Heart's Session/Run and needs-me design. No live provider
experiment was run; current documentation is not proof of installed-version support.

Product decision after this audit: Jack Heart chose one visible **Waiting** state
with provider-specific detection. The fallback is no outstanding tool calls and
N minutes without updates; occasional false positives are accepted. `--waiting`
selects Waiting Sessions. No confidence tiers or separate yielded/question labels
are required. Technical limitations below describe evidence, not product blockers.
Claude integration is restricted to the binary's stream-json path; hooks, SDK and
permission hosts described below are outside the selected implementation scope.

## Meaning

Activity, interaction mode and attention are separate. A Session can have background
tools running and an outstanding question simultaneously. Keep request identity and
resolution; a single busy/idle flag loses that fact. A turn ending proves that the
agent yielded, not that its last message requires an answer. Silence, low CPU,
process existence, tool-call text and historical running items are insufficient to
prove a live human wait. Native terminal observation has less evidence than an owned
protocol connection. A disconnect makes outstanding activity uncertain.

## Claude Code

Owned stream-json exposes message/tool blocks, correlated tool results and final
`result`; Loopflow maps these to turn and item lifecycles. A streamed tool-use block
begins while arguments are being generated: it does not prove tool execution started.
Loopflow launches the `claude` binary, not Agent SDK. SDK `canUseTool` is therefore
not an available callback in this integration. The binary's documented hooks and
permission-host path are the relevant extension points; using SDK is not proposed.
For `-p`, interactive tools require a supported permission host. CLI hooks can
collect answers through a host UI and return updated input. Current Loopflow mapping
does not expose a shared pending-question lifecycle or establish this host path.

Native terminal hooks can observe UserPromptSubmit, tool boundaries, Stop and
permission requests. Stop is yielding, not semantic question detection. Notification
idle_prompt is delayed (about 60 seconds under documented conditions), not an
immediate authoritative waiting-for-reply signal. PermissionRequest also fires for
some unpromptable calls that are subsequently denied, and excludes sandbox network
prompts; track actual resolution rather than latching every hook as a human wait.

Sources: https://code.claude.com/docs/en/hooks and
https://code.claude.com/docs/en/cli-reference . SDK documentation was initially
consulted but does not establish capabilities of Loopflow's binary integration.
Local: harness/claude.rs, claude_mapping.rs, claude_history.rs.

## Codex

App-server exposes turn/started and turn/completed with completed/failed/interrupted
status, plus item/started and item/completed for commands, edits and tools. Explicit
command/file/permission approval requests, item/tool/requestUserInput and MCP
elicitation identify pending interactions. serverRequest/resolved clears requests,
including cancellation; requests need not mean the entire turn has stopped.
Item start can precede approval, so it is not proof of a running command.

Loopflow records turn/item boundaries and retrying errors. The inspected codex.rs
server-request handler treats every request with an ID as an approval and replies
with decision: accept. That is not a valid general implementation for structured
questions/elicitation; their distinct request/reply schemas need handling. Passive
terminal/history observation must not assume access to the app-server request stream.

Source: https://learn.chatgpt.com/docs/app-server .
Local: harness/codex.rs, codex_mapping.rs, codex_history.rs.

## OpenCode

Server status and events expose activity; tool parts distinguish pending/running/
completed/error. Question service maintains pending requests and publishes asked,
replied and rejected events. Status idle alone does not imply a question or success.
The live server can expose pending questions; retained messages alone are weaker.

Loopflow normalizes running/completed/error tool parts and reconstructs turn
completion from assistant receipts with no pending tools. Its mapping intentionally
does not treat session.status or session.idle as turn completion. permission.asked
is handled and answered once automatically; question events are not mapped into
shared attention. Implement pending request reads/events and resolution without
mistaking an automatic permission response for a request to Jack.

Sources: https://opencode.ai/docs/server/ and
https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/opencode/src/question/index.ts
and https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/opencode/src/session/status.ts .
Local: harness/opencode.rs, opencode_mapping.rs, opencode_history.rs.

## Common implementation gap

`chat/types.rs::ConversationEvent` has turn/item lifecycles but no first-class
outstanding-input/request-resolved events. `ops/human_session.rs::session_attention`
currently infers Reply from a completed interactive turn and marks current Flow
reviews even before preparation completes. `Active` means an attached client.
These are insufficient for strict current `--waiting` semantics.

Earlier proposal to separate yielded answers from questions is superseded by Jack's
single Waiting state. Available provider events and the quiet-time/no-open-tools
heuristic drive that state; new activity clears it. No inspected turn-completion
event proves semantic need for a reply, and Jack accepts that limitation. Tests
should cover event correlation and state transitions, not require semantic certainty.
