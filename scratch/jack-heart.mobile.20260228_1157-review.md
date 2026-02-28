# Quote Replies on iOS — Review

## What was implemented

iOS users can now select assistant text, trigger quote replies and emoji reactions from the edit menu, and send them — producing the same blockquote format as macOS.

Four changes:

1. **`SelectableAssistantTextView`** — `UIViewRepresentable` wrapping a non-editable, selectable `UITextView`. Renders inline markdown via `NSAttributedString(markdown:)` post-processed to apply Lato 14pt, JetBrains Mono 13pt for code, and bold/italic variants. Adds "Quote Reply" + emoji shortcuts (👍 👎 ✏️ ❓) to the edit menu via `buildMenu(with:)`.

2. **`ReplyComposerContent` refactor** — Extracted shared composer layout from `ReplyComposerPopover`. macOS wrapper adds `.frame(width: 320)`. iOS `MessageRow` presents via sheet (iPhone compact) or popover (iPad regular) using `horizontalSizeClass`.

3. **iOS `MessageRow`** — Replaces `AssistantTextSegment` (SwiftUI `Text`) with `SelectableAssistantTextView`. Emoji reactions queue immediately; "Quote Reply" opens the composer.

4. **`AssistantTextSegment` deleted** — No longer needed. The `SelectableAssistantTextView` serves the same purpose with selection support.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| `UITextView` over SwiftUI `Text` | `.textSelection(.enabled)` doesn't expose selected text or custom menu actions. `UITextView` gives `selectedRange` and `buildMenu(with:)`. Same choice as macOS's `NSTextView`. | SwiftUI `Text` — no selection access |
| Sheet on iPhone, popover on iPad | iPhone popovers auto-promote to full sheets with no spatial anchor. `horizontalSizeClass` branches cleanly. | Always sheet — loses spatial context on iPad |
| Emoji reactions bypass composer | Unambiguous intent. Tap emoji → queue immediately. Fewer taps. | Always open composer — unnecessary friction |
| Post-process `NSAttributedString` | Foundation handles markdown parsing; we restyle with Lato/JetBrains Mono. Won't pixel-match SwiftUI `Text` — acceptable. | Manual regex parsing — fragile |

## How it fits together

```
User selects text in SelectableAssistantTextView
  → buildMenu adds "Quote Reply" + emoji actions
  → QuoteAction callback to MessageRow
    → .emojiReact: queues ReplyEntry directly
    → .quoteReply: opens ReplyComposerContent (sheet/popover)
      → "Queue" or emoji: queues ReplyEntry
  → ReplyDraftTray shows queue
  → Send: ReplyQueue.assembleMessage() → same blockquote format as macOS
```

`ReplyQueue`, `ReplyEntry`, `assembleMessage()`, and wire protocol are untouched. iOS and macOS produce identical message shapes.

## Risks and bottlenecks

- **Self-sizing `UIViewRepresentable`**: `intrinsicContentSize` + `layoutSubviews` invalidation works but can trigger extra layout passes on rapid content updates (streaming). The macOS `NSTextView` uses the same pattern. Watch for jank during fast streaming.
- **Font availability**: Falls back to system fonts if Lato/JetBrains Mono aren't bundled. Tests verify font family name but CI may not have the fonts installed — tests check with `||` alternatives.
- **`NSAttributedString(markdown:)` styling gaps**: Won't match SwiftUI `Text`'s line spacing and paragraph spacing exactly. Visually close but not identical. Noted in design doc as acceptable.

## What's not included

- **Reorder/edit queued replies** — Cross-platform feature, orthogonal. Follow-up PR.
- **Selection reset on composer dismiss** — macOS has `selectionResetToken`; iOS selection persists. Not disruptive, can add later.
- **Block-level markdown** — Code blocks already handled by `CodeBlockView`. This view handles inline markdown only (bold, italic, inline code, links, strikethrough).
