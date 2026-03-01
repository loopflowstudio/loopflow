# 03: Queue Management

**Finish line:** Users can reorder, edit, and delete queued replies before sending — on both iOS and macOS.

## Carried context

- Quote replies shipped on iOS (PR jack-heart.mobile.20260228_1157). iOS and macOS produce identical blockquote messages via `ReplyQueue.assembleMessage()`.
- `ReplyDraftTray` renders the queue but has no reorder/edit/delete controls.
- Emoji reactions bypass the composer and queue immediately — this flow stays unchanged.
- iOS selection persists after composer dismiss (macOS has `selectionResetToken`). Address as polish here or in a follow-up.
- Self-sizing `UIViewRepresentable` can trigger extra layout passes during fast streaming. Watch for jank — same pattern as macOS `NSTextView` but UIKit layout differs. Not blocking but worth monitoring.
- Discovery auth shipped (02): mobile connects via connection tokens, WS re-validation runs on a 60s interval in the select loop. Queue management is client-side only — no auth/protocol changes needed.

## What to build

1. **Reorder** — Drag-to-reorder in `ReplyDraftTray`. SwiftUI `onMove` on both platforms.
2. **Edit** — Tap a queued entry to reopen the composer with its content pre-filled. Update in place on submit.
3. **Delete** — Swipe-to-delete on iOS, delete button on macOS. Already partially wired via `ReplyQueue.remove`.

## Constraints

- Cross-platform: same behavior on iOS and macOS.
- No protocol changes — queue management is client-side only.
- `assembleMessage()` output format unchanged.

## Done when

- Queued replies can be reordered, edited, and deleted before sending on both platforms.
- Tests cover reorder and edit-in-place flows.
- Manual verification on iPhone, iPad, and macOS.
