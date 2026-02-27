# Quote Replies on iOS

## Problem

iOS users can view sessions but can't select assistant text or give precise feedback. They have to switch to desktop or type vague references like "that part about indexes." macOS quote replies work well — this brings the same power to touch. Discovery is now the default first-launch path on iOS, so mobile feedback loops matter.

## Approach

Mirror the macOS selection→compose→queue→send flow, adapted for touch conventions.

**Selection**: `UITextView` wrapped in `UIViewRepresentable`, just like macOS uses `NSTextView`. Same delegate pattern (`textViewDidChangeSelection`), same trimming/cleaning. Add "Quote Reply" to the UITextView's edit menu via `UIEditMenuInteraction` — it appears alongside Copy/Select All, right where iOS users expect actions on selected text.

**Composer**: When the user taps "Quote Reply", capture the selection and present a partial sheet (`.presentationDetents([.medium])`). The sheet contains the quoted text preview, a text field, the same 4 emoji quick-reacts, and a "Queue" button. This is the iOS equivalent of the macOS popover — thumb-reachable, keyboard-aware, dismissable by swipe.

**Queue + Send**: `ReplyQueue` and `ReplyDraftTray` are already platform-agnostic SwiftUI. They work on iOS as-is. `assembleMessage()` produces identical markdown on both platforms. No changes to the send path.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Custom gesture-based selection (build our own long-press → handles → range tracking) | Full control over UX | Enormous implementation effort reinventing what UITextView does natively. Fragile across iOS versions. |
| SwiftUI `.textSelection(.enabled)` on `Text` views | Zero new code for selection | No callback for selected text — SwiftUI doesn't expose what the user selected. Can't wire it to reply. |
| Floating "Reply" button near selection (instead of edit menu) | More discoverable than menu | Competes with system selection handles, hard to position on small screens, keyboard pushes it around. |
| Inline composer (transform main composer area) | No modal to manage | Overloads the composer — confusing when user wants to type a normal message vs. annotate a quote. iMessage's reply bar works because it's 1:1 message threading; our queue model is N quotes in one send. |

## Key decisions

**UITextView over SwiftUI Text.** SwiftUI's `Text` view with `.textSelection(.enabled)` doesn't expose the selected string — there's no callback. `UITextView` gives us `textViewDidChangeSelection` and the full edit menu API. Since we target iOS 18+, `UIEditMenuInteraction` is the right surface.

**Edit menu "Quote Reply" over floating UI.** Adding to the system edit menu is the most iOS-native affordance. Users already know to select → tap menu action. No custom floating buttons to position, no z-index fights with selection handles. Slack, Notion, and Bear all use this pattern.

**Partial sheet over popover.** iOS `.popover()` on iPhone presents as a full sheet, which is too heavy. A `.medium` detent sheet shows the composer at thumb height, stays visible alongside the keyboard, and dismisses with a swipe. This is the iOS equivalent of the macOS popover — contextual and lightweight.

**Markdown rendering in UITextView.** `AssistantTextSegment` currently uses `AttributedString(markdown:)` for inline markdown. The iOS UITextView wrapper does the same conversion to `NSAttributedString`, preserving bold/italic/code rendering while enabling selection. Font and color match the design system (`Typography.body()`, `palette.text`).

**No reorder or inline edit.** macOS doesn't have these either. Delete-and-re-add is sufficient for a queue of 2-5 items. Adding drag-to-reorder or edit-in-place would increase complexity without clear user benefit.

## Scope

**In scope:**
- `SelectableAssistantTextView` — UIViewRepresentable wrapping UITextView with selection tracking and "Quote Reply" edit menu action
- `MobileReplyComposerSheet` — partial sheet with quoted text, text field, emoji row, Queue button
- `MessageRow` iOS branch — replace `AssistantTextSegment` with `SelectableAssistantTextView`, present reply composer sheet, wire `onQueueEntry`
- Tests for selection tracking, edit menu action, and sheet→queue integration

**Out of scope:**
- Drag-to-reorder queue items
- Inline editing of queued items
- Full emoji picker (keep the 4 fixed emojis)
- Selection in code blocks (not on macOS either — `CodeBlockView` has `.textSelection(.enabled)` for copy, but no quote-reply path)
- Protocol or payload format changes
- macOS behavior changes

## Implementation

### New files

**`swift/Concerto/Platform/iOS/Views/SelectableAssistantTextView.swift`**

UIViewRepresentable wrapping a non-editable, selectable UITextView. Renders markdown via `NSAttributedString`. Tracks selection changes via `UITextViewDelegate.textViewDidChangeSelection`. Adds "Quote Reply" to the edit menu via `UIEditMenuInteraction`. When the user taps "Quote Reply", fires `onQuoteReply(selectedText)` and clears the selection.

Key details:
- `isEditable = false`, `isSelectable = true`, `backgroundColor = .clear`
- Font: `UIFont(name: "Lato", size: 14)` matching macOS `AutosizingSelectableTextView`
- Text color: adapts to `palette.text` via environment
- Intrinsic content size calculated from `layoutManager.usedRect` (same pattern as macOS)
- Selection reset via token mechanism (same as macOS `selectionResetToken`)

**`swift/Concerto/Platform/iOS/Views/MobileReplyComposerSheet.swift`**

Sheet content for composing a reply to selected text. Contains:
- Quoted text preview (truncated to 4 lines, gray)
- TextField for typing a reply (multiline, 1-4 lines)
- Emoji row: 4 buttons (same as macOS)
- "Queue" button (dark style, disabled when reply text is empty for text replies)

Callbacks: `onQueueTextReply(quoted:reply:)`, `onQueueEmoji(quoted:emoji:)`. Both dismiss the sheet.

### Modified files

**`swift/Concerto/Views/MessageRow.swift`** — iOS `#else` branch:
- Replace `AssistantTextSegment(text:)` with `SelectableAssistantTextView(text:, onQuoteReply:)`
- Add `@State private var activeQuote: String?` to track the selected quote for sheet presentation
- Add `.sheet(item: $activeQuote)` presenting `MobileReplyComposerSheet`
- Wire sheet callbacks to `onQueueEntry`

**`swift/ConcertoTests/ReplyQueueTests.swift`** — existing tests already cover queue behavior. Add a test verifying the `onQueueEntry` callback signature works for both platforms (it already does — `ReplyEntry` has no platform deps).

### Data flow

```
Long-press assistant text
  → UITextView native selection (loupe, handles)
  → System edit menu appears with "Quote Reply"
  → User taps "Quote Reply"
  → onQuoteReply(trimmedText) fires
  → MessageRow sets activeQuote = trimmedText
  → .sheet presents MobileReplyComposerSheet
  → User types reply + taps "Queue" (or taps emoji)
  → onQueueEntry(.quoteReply(...)) fires
  → ReplyQueue.add(entry) in WaveSessionView
  → ReplyDraftTray renders (shared, no changes)
  → User taps Send
  → assembleMessage() → identical markdown
  → SessionState.send(text)
```

## Done when

- `swift test --package-path swift` passes with new tests
- `xcodebuild test ... -destination 'platform=iOS Simulator,name=iPhone 16'` passes
- iPhone simulator: long-press assistant text → select → "Quote Reply" in menu → sheet → type reply → Queue → tray shows entry → Send → message sent with `> quoted\n\nreply` format
- iPad simulator: same flow works in split-view layout
- macOS: zero behavior change (verified by existing tests passing)
