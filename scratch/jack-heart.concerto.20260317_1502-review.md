# Review: jack-heart.concerto.20260317_1502

## What was implemented

- Added queued-reply management in Concerto's reply tray: users can tap queued text replies to edit them in place, drag to reorder entries, and delete entries without sending.
- Extended `ReplyQueue` with in-place update and move helpers, plus tests that prove edits preserve entry identity and still assemble the expected outbound message.
- Split the edit-composer presentation into platform-specific wrappers and tightened the composer UX by disabling submit when the draft is blank.
- Updated Concerto wave planning docs to pivot future auth work from manual API key entry to a secrets-provider approach, with Doppler as the first provider.

## Key choices

- Kept queue mutation logic in `ReplyQueue` instead of mutating `entries` directly from the tray so normalization and ID preservation stay in one place.
- Reused `ReplyComposerContent` for both new replies and edit-in-place flows to keep queueing and editing behavior aligned.
- Moved iOS size-class detection into `ReplyDraftTray` and passed a plain `isCompact` flag into `ReplyDraftEditPresentation`; this keeps the platform shell files thin and avoids cross-file visibility/property-wrapper issues.
- Left the secrets-provider work as design and roadmap updates only. The branch does not implement the Doppler flow yet.

## How it fits together

`MessageRow` still creates queued replies from text selection. `ReplyDraftTray` now owns edit state for queued entries and presents the shared composer in a platform-specific sheet/popover wrapper. When the user saves, the tray calls `ReplyQueue.update`/`move`/`remove`, and `ReplyQueue` remains the single source of truth for sanitizing content and assembling the final message.

## Risks and bottlenecks

- Reordering relies on SwiftUI `List` move behavior; manual validation on iPhone/iPad/macOS is still worth doing because drag affordances differ by platform.
- The tray row height is still fixed, so unusually large dynamic type settings may need a follow-up polish pass.
- The branch mixes shipped queue-management code with forward-looking secrets-provider docs, which can confuse a cold reviewer if they expect the new docs to correspond to implemented behavior.

## What's not included

- No secrets-provider runtime, OAuth flow, API endpoints, or Concerto settings UI yet.
- No protocol or backend changes for reply queues; all queue management remains client-side.
- No new manual verification script for the queue tray interactions.

## Validation

- `uv run ruff check scripts/concerto-dev.py scripts/install.py scripts/lib/trigger_scenarios.py`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
