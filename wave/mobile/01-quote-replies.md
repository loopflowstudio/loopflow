# 05: Quote Replies on iOS

Bring the quote-reply workflow to iPhone and iPad so users can attach precise feedback to assistant output without switching to desktop.

## Carried context

- Multi-client session consistency is already shipped; iOS and macOS can co-view and co-edit the same session state.
- Discovery is now the default first-launch path on iOS, so mobile-first feedback loops matter more.
- macOS quote replies are already wired into `WaveSessionView` with structured message assembly and tests.

## What to build

1. **Touch selection for assistant output**
   - Enable selecting meaningful text spans in iOS session bubbles.
   - Surface a reply entry point from selection.

2. **Queued quote-reply editing on iOS**
   - Let users queue emoji reactions, quoted replies, and free-text notes.
   - Support reorder, edit, and delete before send.

3. **Single-send assembly parity**
   - Reuse the existing structured payload format so iOS and macOS produce equivalent messages.
   - Keep non-quote composer behavior unchanged.

4. **Readable selectable markdown on mobile**
   - Preserve markdown readability while allowing selection and quote extraction.

## Constraints

- No protocol changes unless existing structured reply payload is insufficient.
- Keep manual message send and suggested-action flows intact.
- Keep touch affordances accessible (thumb-reachable controls, clear feedback states).
- macOS behavior must remain unchanged.

## Done when

- iPhone and iPad users can select assistant text and create quote replies from that selection.
- Queue management (add/edit/reorder/delete) is available before sending.
- Sending from iOS emits the same structured reply shape used on macOS.
- Swift tests cover queue assembly behavior and iOS-specific edge cases where practical.
- Manual mobile verification confirms selection ergonomics and queue usability on iPhone and iPad simulators.
