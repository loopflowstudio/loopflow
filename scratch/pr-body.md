## Try it!

- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- Open Concerto, queue a few replies from a message selection, then:
  - tap a queued text reply to edit it in place
  - drag queued rows to reorder them
  - delete a row from the tray
- Expect the tray to preserve reply IDs/order updates locally and still assemble the same final blockquote format when sent.

Validation run on this branch:
- `uv run ruff check scripts/concerto-dev.py scripts/install.py scripts/lib/trigger_scenarios.py` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ✅

## Intent

Make queued replies feel like a real staging area instead of a dead-end list. This branch lets users reorder, edit, and delete queued replies before sending, while keeping reply normalization and final message assembly inside `ReplyQueue`. It also retargets the Concerto auth roadmap away from manual key paste and toward a future secrets-provider flow.

## Assumptions

- Queue management stays entirely client-side; no lfd protocol or backend changes are needed.
- SwiftUI `List` move/delete affordances are acceptable on both macOS and iOS for this first pass.
- The new secrets-provider scratch/wave docs are planning artifacts only, not shipped functionality in this PR.

## Key decisions

- `ReplyQueue` now owns `move`, `remove(atOffsets:)`, and `update` so entry sanitation and ID preservation happen in one place.
- The queued-reply edit sheet/popover reuses `ReplyComposerContent` instead of introducing a second editor UI.
- The platform-specific `ReplyDraftEditPresentation` files only handle presentation; `ReplyDraftTray` computes compact-vs-regular presentation state and passes it in.
- Save/Queue is disabled for blank drafts so edit-in-place and new reply flows both reject whitespace-only submissions at the UI layer.

## Not included

- No Doppler/secrets-provider implementation yet.
- No queue persistence across app launches.
- No extra manual demo script beyond the commands above.
