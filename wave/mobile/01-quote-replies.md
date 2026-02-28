# 01: Quote Replies on iOS

**Finish line:** iPhone and iPad users can select assistant text, queue quote replies and emoji reactions, and send — producing the same blockquote format as macOS.

## Carried context

- Discovery is the default iOS entry point; mobile-first feedback loops matter.
- macOS quote replies are wired into `WaveSessionView` with structured message assembly and tests.
- `onQueueEntry` callback exists on iOS but nothing triggers it — assistant prose isn't selectable.

## Approach

`UITextView` (non-editable, selectable) as iOS equivalent of macOS's `NSTextView`-based `SelectableAssistantMessageTextView`. Custom edit menu actions for "Quote Reply" and emoji shortcuts. Sheet on iPhone, popover on iPad.

### Components

**`SelectableAssistantTextView`** (new, iOS `UIViewRepresentable`)
- Wraps `UITextView` configured as non-editable, selectable
- Renders via `NSAttributedString(markdown:)` post-processed to apply Lato 14pt and palette colors
- Adds "Quote Reply" + emoji shortcuts (👍 👎 ✏️ ❓) to edit menu via `buildMenu(with:)`
- Reports actions via `onQuoteAction: (String, QuoteAction) -> Void`
- Self-sizes using `intrinsicContentSize` pattern from macOS `AutosizingSelectableTextView`

**`ReplyComposerContent`** (refactored from `ReplyComposerPopover`, cross-platform)
- Extract shared layout: quoted preview, TextField (1–4 lines), emoji buttons, "Queue" button
- `ReplyComposerPopover` becomes thin wrapper presenting in `.popover` (macOS, unchanged)
- iOS presents in `.sheet(item:)` with `.presentationDetents([.medium])` on iPhone, `.popover` on iPad regular width via `horizontalSizeClass`

**iOS `MessageRow` changes**
- Replace `AssistantTextSegment` with `SelectableAssistantTextView` for assistant messages
- Emoji quick-actions queue immediately (skip sheet); only "Quote Reply" opens composer

### What stays the same

`ReplyQueue`, `ReplyEntry`, `assembleMessage()`, `WaveSessionView.sendMessage()`, wire protocol, macOS views, `ReplyDraftTray`.

## Key decisions

- **UITextView over SwiftUI Text.** `.textSelection(.enabled)` doesn't expose selected text. `UITextView` gives `selectedRange` and `buildMenu(with:)`. Same choice as macOS's `NSTextView`.
- **Sheet on iPhone, popover on iPad.** iPhone popovers auto-promote to sheets with no spatial anchor. Use `horizontalSizeClass` to branch.
- **Emoji reactions bypass the sheet.** Unambiguous intent — queue immediately.
- **Shared composer content, platform presentation.** One layout in `ReplyComposerContent`, platform wrappers for presentation.
- **Post-process NSAttributedString.** Foundation's markdown parsing + our styling. Paragraph spacing won't pixel-match SwiftUI `Text` — acceptable.
- **Reorder deferred.** Cross-platform, orthogonal. Follow-up PR.

## Risks

- NSAttributedString styling may produce visual gaps vs SwiftUI `Text`. Verify early.
- Self-sizing `UIViewRepresentable` wrappers are fiddly. macOS pattern exists as reference but UIKit layout differs.
- `buildMenu(with:)` requires iOS 16+. Verify against deployment target.

## Constraints

- No protocol changes.
- macOS behavior unchanged.
- Inline markdown only (block-level rendering, syntax highlighting out of scope).

## Done when

- `SelectableAssistantTextView` renders assistant prose with inline markdown, visually close to `AssistantTextSegment`
- Selecting text on iOS shows "Quote Reply" and emoji actions in the edit menu
- "Quote Reply" opens composer sheet (iPhone) or popover (iPad); emoji actions queue immediately
- `ReplyComposerPopover` refactored to shared `ReplyComposerContent`; macOS unchanged
- Sending from iOS produces same blockquote format as macOS
- Swift tests cover NSAttributedString post-processing and queue-then-assemble flow
- Manual verification on iPhone and iPad simulators
