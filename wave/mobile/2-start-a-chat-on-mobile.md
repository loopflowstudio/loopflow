# Start a chat on mobile

**Finish line:** From the iOS app, start a new chat session with an agent — pick provider, optionally pick a wave for context, type a question, get streaming responses. Full native chat UX: markdown, code blocks, tool call cards, voice input.

**Needs:** `workflows/chat-session-api`

## Context

The phone becomes a real interactive surface when you can start a conversation from it — not just view what's happening. This depends on the chat session API backend; once that lands, mobile chat is a consumer of the same stream.

## Daily experience

On a walk, you have a design thought. Open the app, tap new chat, pick Claude, tap "use `wave/root` context," type the thought, hit send. Response streams in as you walk. The agent writes a proposal; you ask a follow-up. By the time you're back at your desk, the idea is half-formed with a written record.

## Done when

- New chat flow: pick provider, optional wave context, start
- Streaming responses with typed content (markdown / code / tool cards)
- Voice input works
- Session persists to the lfd store — survives app close
- Provider selection (Claude / Codex / OpenCode Zen) respects connected auth
