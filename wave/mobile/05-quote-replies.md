# 05: Quote Replies

Attach feedback to exact assistant spans instead of replying to whole messages.

**Status: macOS milestone shipped** (branch `jack-heart.replies.20260225_2033`)

## What shipped

- `ReplyEntry` + `ReplyQueue` model/assembly flow for quote replies, emoji reactions, and free-text entries.
- `ReplyDraftTray` for reviewing and deleting queued items (appears when queue non-empty; free-text goes through the existing composer).
- `ReplyComposerPopover` for text reply + fixed quick-react emoji buttons on selected quote text.
- `SelectableAssistantMessageTextView` (macOS) so assistant text selection can trigger quote-reply actions.
- `WaveSessionView` wiring:
  - selection opens reply composer
  - queued entries render in the draft tray
  - send assembles one structured message via `ReplyQueue.assembleMessage(...)` and dispatches through `SessionState.send`
- Debug surface: `ReplyDemoView` + Debug menu entry (`Reply Demo`, ⇧⌘R) for option comparison and screenshots.
- Unit coverage: `ReplyQueueTests` verifies assembly/sanitization, including multi-line quote formatting (`>` on every line).

## Open follow-up work

- Decide capture scope: keep assistant-only quote selection or include user/system/error messages.
- Add queued-entry reorder and edit controls (current UI supports add + delete).
- Restore markdown fidelity in selectable assistant bubbles (current selectable renderer is plain text).
- Add iOS quote selection gesture + compose flow (long-press/select equivalent).
- Revisit optional UX polish after dogfooding:
  - fixed emoji palette vs. customizable picker
  - inline “already replied” annotations in long responses

## Done when

- macOS and iOS both support quote selection + reply/react queueing.
- Queue entries can be edited/reordered/deleted before send.
- Selectable assistant rendering preserves markdown readability.
- Structured assembled message output remains deterministic and covered by tests.
