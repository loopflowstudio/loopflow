# Cross-harness resume

Status: implemented (2026-10-14); headless path lacks its own test. Jack requested it.

Decided 2026-10-14 (Jack): no `--fresh` flag. When the provider matches, resume stays native. Jack is fixing the Claude permission block separately. The log format is the designer's call: whatever helps the next agent.

Decided 2026-10-14 (Jack): the skill belongs in the log and is not re-sent with the continuation. The first turn quotes the log's beginning and end, size-limited (6k + 14k chars), rather than only pointing at it. `lf session connect ID -a codex` imports the same way as resume. Any direction works: Claude, Codex and OpenCode are each a source (OpenCode via `opencode export`), and any provider is a target. `resume_choice` takes the selected agent explicitly, matching the parameter shape of `resume_model` on `jack-heart/claude-permissions`.

## Problem

Jack had a Claude Session stop because it was blocked by permissions, then ran:

```
% lf resume -a codex
Error: This Session's native history belongs to claude; it cannot resume with codex. Start a new Session with lf -a codex : <message>.
```

Jack: "I wanted to basically import the session and restart it in claude", then: "in codex".
Jack: "simple and fast are important. we dont need to preserve like a turn by turn seuqnce. just the log is enough."

## What to build

When `-a` names a different provider, `lf resume` keeps the same LfSession and
starts a fresh AgentSession on the new provider. Its first turn points to a
plain-text log of the earlier conversation.

## The demo

```
% lf resume -a codex
```

Codex opens in the same Session, with the same name and Task. Its first turn
says the conversation began in Claude and gives the log path and the last
request. Codex reads the log and continues. `lf session history` shows both
AgentSessions under one Session. `lf resume` alone then resumes Codex natively.

## As built

- `config::resume_choice(provider, model) -> Resume { Native(model) | Import { agent } }`
  is the one decision point; the refusal is gone.
- `ops/human_session/conversation_log.rs`: `render(provider, transcript)` returns
  `ConversationLog { markdown, last_request }` (one transcript read). It writes
  `## user`/`## assistant` blocks and `- tool: <name> <input ≤200 chars>`; a failed
  call (Claude `is_error`, Codex nonzero exit) gets the last 400 chars of its output.
  It drops successful results, thinking, sidechains, meta lines and injected
  `<lf:` blocks (keeping the `<lf:message>` text). Claude rows reuse
  `provider_conversation::is_user_input`, extracted from `claude_input_time`.
- `conversation_log::continuation(store, session)` finds the transcript via
  `conversation_home` + `transcript`, writes `<logged capture>/conversation.md`,
  and returns the continuation turn (origin provider, log path, last request).
  The log sits beside the logged conversation's capture, so both paths know it
  before launch. A second switch logs only the newest AgentSession; its first
  turn names the earlier log.
- Interactive: `human_session::open` → `import`, which builds the turn, stops the
  agent, sets provider/model from `-a`, and runs `append_input` (shared with
  `open_waiting`) then `serve_locked`.
- Headless: `lf -b session resume ID MSG -a codex` prepends the continuation
  turn to MSG; `continue_with_context` now records the launching agent on the row.
- `replace_input_in` clears `agent_sessions.provider_thread` when the provider
  changes, so the new provider never inherits the old resume token.
- `util::resume_session_with_env` is unreachable with a mismatched provider; it
  errors and names `lf resume -a`.

## Constraints

- **Same LfSession, new AgentSession.** It does not create a successor
  Session (`lf session replace`). Name, Task binding and feedback stay.
- **The log is a file, not inline.** Claude transcripts run to megabytes.
  Only the last user message goes in the turn
  (`validate_terminal_turn` bounds it).
- **No turn replay.** Jack ruled it out.

## Forbidden outcomes

- A new LfSession per switch, or a "fork" command beside `resume`.
- Storing the log in SQLite, or a second transcript type beside
  `ConversationEvent`.
- An LLM summarization step.

## Done when

Met: the rewritten `task_flow_launch_tests.rs` case passes:
`session resume <id> -a codex` (with a fixture Claude transcript and a stand-in
codex) succeeds. The Session count stays the same, and the Session's provider is now
`codex`. The stand-in receives a first turn naming the log path. The log
contains the fixture's user and assistant text and the failed tool's output tail, and no successful tool output.

- Met: unit tests of `render` cover a Claude fixture and a Codex rollout fixture.
- Gate: `cargo test -p loopflow conversation_log task_flow_launch`.

## Remaining

- A headless import test (needs a Codex app-server stand-in plus a Codex account).
- Demo check: `lf session history` shows both AgentSessions; a following
  `lf resume` resumes Codex natively.

Check: `cargo test -p loopflow --lib -- conversation_log resume_stays_native` + `task_flow_launch_tests::explicit_agent_wins` + clippy pass (after compress; realign 2026-10-14 reran nothing).
