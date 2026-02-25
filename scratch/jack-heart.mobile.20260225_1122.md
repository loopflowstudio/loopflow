# Mobile action buttons — review follow-ups

Design doc for changes discussed during review of `jack-heart.mobile.20260225_1122`.

## Done on branch

- `SyntheticTool` → `LoopflowTool` → `StructuredReply` (file, struct, functions, field, XML tag, docs)
- Dropped unused `schema` field from `StructuredReply`
- Stronger prompt guidance: REQUIRED/never-skip language, explains why (phone keyboard)
- Scroll-jacking fix: invisible bottom anchor, only auto-scroll when user is near bottom
- Composer always focusable: removed `.disabled(!state.canSend)` from TextField
- `getSession` uses long timeouts to reduce connection timeout frequency
- Flat interleaved layout: accent bars instead of left/right bubbles, full-width flow
- Hit-testing delay on action buttons (300ms before interactive)

## Remaining work

### ~~1. Rename `LoopflowTool` → `StructuredReply`~~ Done

### ~~2. Rename `Chat*` → `Session*` across Swift~~ Done

### ~~3. Collapse consecutive tool calls~~ Done

### ~~4. Investigate auto-send bug~~ Done

Root cause: `process_user_message()` in `claude_mapping.rs` emitted text blocks from Claude's "user" type events as `SessionItem::Message` with `phase: "user"`. These are Claude echoing back the input the client already displayed — not new user actions. Fix: drop text blocks from user message processing; only process `tool_result` blocks.

### 5. Full-screen interleaving polish

The flat layout (accent bars) is in place. Additional polish:
- Verify accent bar colors work in both light and dark mode
- Consider adding a subtle role label ("you" / agent name) above each message group
- Ensure tool cards blend with the flat layout (no competing visual weight)
- Test on compact/iPhone layout

## Out of scope

- Additional structured replies beyond `suggest_actions`
- Persistence/analytics for actions
- iOS MobileWaveDetailView wiring (separate follow-up)
