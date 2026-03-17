# Queue Management

## Problem

Users can queue multiple replies before sending, but once queued, entries are frozen. You can delete one (xmark button), but you can't fix a typo, rethink reply order, or remove entries with a natural gesture. The queue is append-only with a nuclear option (clear all on send). For anyone composing more than two replies, this is friction that undermines the whole batching concept.

Both platforms need this. The queue model (`ReplyQueue`) is shared; the tray (`ReplyDraftTray`) is shared; the composer presentation diverges per platform (popover on macOS, sheet/popover on iOS). The fix lives almost entirely in shared code.

## Approach

Three capabilities, one PR. All changes target `ReplyQueue` (model) and `ReplyDraftTray` (view). No protocol changes, no new files.

### 1. Reorder — drag to reorder in the tray

Add `move(fromOffsets:toOffset:)` to `ReplyQueue`. Wire `ForEach` with `.onMove` in `ReplyDraftTray`.

SwiftUI's `onMove` works on both platforms inside a `ForEach`. It does **not** require `List` — it works with `ForEach` in a `VStack` when combined with a `DragGesture` or `draggable`/`dropDestination` on each row. However, the cleanest cross-platform path is to switch the tray's inner container from `VStack` to `List` with `.listStyle(.plain)` and `.listRowBackground(Color.clear)` — this gives us `onMove` and `onDelete` for free on both platforms with native drag handles.

The tradeoff: `List` has its own styling opinions. We'll strip them with `.listRowInsets`, `.listRowSeparator(.hidden)`, and `.listRowBackground` to preserve the current visual design (muted rounded-rect cards). The tray is small (typically 2-5 entries) so `List` performance overhead is negligible.

### 2. Edit — tap entry to reopen composer, update in place

Add `update(id:newEntry:)` to `ReplyQueue` that replaces an entry by ID, preserving position. The tray row becomes tappable — tapping sets an `editingEntryID` state on the tray, which presents the composer pre-filled with that entry's content.

On submit, the composer calls `queue.update(id:newEntry:)` instead of `queue.add(_:)`. The editing state is cleared.

Platform-specific composer presentation is already handled (`ReplyComposerPopover` on macOS, `ReplyComposerPresentation` sheet/popover on iOS). We add a new shared `EditComposerContent` view (or reuse `ReplyComposerContent` with an edit mode) that shows the existing quoted text and pre-fills the reply draft.

Decision: **Reuse `ReplyComposerContent`**. It already takes `quoted` and `replyDraft` as bindings. The tray will present it in a popover (macOS) or sheet (iOS) with the entry's values pre-filled. The only difference from "new reply" mode is the submit action: update-in-place vs. append.

Emoji-react entries (`.emojiReact`) are not editable — tapping them does nothing. They're single-emoji responses; if you want to change the emoji, delete and re-react. Quote replies and free text are editable.

### 3. Delete — swipe on iOS, button on macOS (already exists)

The xmark button already works on macOS. With `List`, we get `.onDelete` which provides swipe-to-delete on iOS for free. Keep the xmark button as a visible affordance on both platforms.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `VStack` + manual `DragGesture` for reorder | Full visual control, no `List` styling fights | Manual drag implementation is ~80 lines of gesture + offset tracking per platform. `List` gives us native behavior for free. |
| Inline text editing (edit the entry text directly in the tray row) | No popover/sheet, feels fast | The quoted text needs to be visible during editing. Inline editing in a small row is cramped. The composer already handles this well. |
| Make emoji reacts editable | Completeness | No meaningful edit — it's one emoji. Delete + re-react is two taps. Not worth the UI complexity. |
| `EditMode` environment for the whole tray | SwiftUI-native edit mode with move handles | Forces an explicit "edit mode" toggle button, which is extra UI for a tray that's already compact. `onMove` works without `EditMode` on iOS 16+/macOS 13+. |

## Key decisions

**`List` over `VStack` for the tray body.** This is the central decision. `List` gives us `onMove` and `onDelete` with platform-native gestures. The styling cost is manageable — we strip defaults and apply our existing card design. The tray is never large enough for `List` performance to matter.

**Edit via popover/sheet, not inline.** The composer UI is already built and handles quoted text display + text input well. Reusing it keeps the interaction model consistent: select text -> composer appears -> submit. Edit is the same flow, just pre-filled.

**Emoji reacts are not editable.** They're atomic — one emoji, one gesture. Edit semantics don't apply.

**`update(id:newEntry:)` preserves position.** The entry is replaced in-place in the array. This is simpler than remove-and-insert and preserves the user's carefully chosen order.

**No `EditMode` toggle.** The tray is always "editable" — drag handles are always visible when expanded, entries are always tappable. No mode switching.

## Scope

**In scope:**
- `ReplyQueue.move(fromOffsets:toOffset:)` and `ReplyQueue.update(id:newEntry:)`
- `ReplyDraftTray` switches inner `VStack` to `List` with `onMove` and `onDelete`
- Tap-to-edit on quote reply and free text entries (presents composer pre-filled)
- Swipe-to-delete on iOS (via `List` + `onDelete`)
- Tests for move, update, and edit-round-trip
- Both platforms verified

**Out of scope:**
- Emoji react editing
- Selection reset on iOS after composer dismiss (tracked separately)
- Changes to `assembleMessage()` output format
- Protocol or server changes
- Keyboard shortcuts for reorder (future polish)

## Done when

- `swift test --package-path swift` passes with new tests for `move` and `update`
- Concerto UI tests pass: `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- Manual verification: on macOS, drag entries to reorder, tap to edit, xmark to delete. On iOS simulator, drag to reorder, tap to edit, swipe to delete.
