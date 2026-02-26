# Review: jack-heart.replies.20260225_2033

## What was implemented

- Added quote-reply infrastructure for Concerto sessions:
  - `ReplyEntry` model + `ReplyQueue` assembly/sanitization logic.
  - `ReplyDraftTray` UI for queued replies and free-text additions.
  - `ReplyComposerPopover` for text reply + quick emoji reaction on selected quote.
- Added macOS selectable assistant renderer (`SelectableAssistantMessageTextView`) so text selection can trigger quote-reply actions.
- Wired quote-reply into `WaveSessionView`:
  - Assistant text selection opens popover.
  - Queued entries flow into draft tray.
  - `Send` assembles one structured message and dispatches through `SessionState.send`.
- Added `ReplyDemoView` and a new Debug menu entry (`Reply Demo`, ⇧⌘R) to compare interaction variants and tray states with mock data.
- Added unit coverage for reply assembly behavior in `ReplyQueueTests`.
- Polish pass updates:
  - Draft tray send now respects `state.canSend`.
  - `sendMessage()` now guards `state.canSend`.
  - Multi-line quotes are rendered with `>` on every line for cleaner markdown quoting.
  - Added test for multi-line quote formatting.

## Key choices

- **Assistant-only quote selection (macOS first):** quote capture is enabled for assistant bubbles only, aligning with milestone scope and reducing complexity.
- **Single assembled outbound message:** queued entries are flattened into one user message block, preserving existing session send semantics.
- **NSViewRepresentable text selection path:** assistant selection uses a native `NSTextView` wrapper to support precise selection and popover triggering.
- **Demo surface included in-app:** design option review is available from Debug menu instead of relying on Xcode previews.

## How it fits together

`MessageRow` detects assistant text selection (macOS) and emits `ReplyEntry` values. `WaveSessionView` stores these in `ReplyQueue`, renders queue state through `ReplyDraftTray`, and on send calls `ReplyQueue.assembleMessage(extraFreeText:)` before dispatching via `SessionState.send`. `ReplyDemoView` reuses the same queue/tray components with mock data to evaluate UX variants.

## Risks and bottlenecks

- Assistant bubbles in quote-reply mode currently render as plain selectable text (no rich markdown styling).
- Queue management supports add/delete, but not reorder/edit yet.
- Full `xcodebuild test` still includes UI-test runner instability in this environment (early runner exit); unit-only `-only-testing:ConcertoTests` is green.

## What's not included

- iOS quote-reply selection gesture support.
- Reorder/edit controls for queued entries.
- Visual badges/annotations marking already-replied regions in-message.
- Customizable emoji picker behavior beyond fixed quick-react set.
