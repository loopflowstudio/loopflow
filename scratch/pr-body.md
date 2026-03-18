## Try it!

- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- Open Concerto, queue a few replies from a message selection, then:
  - tap a queued text reply to edit it in place
  - drag queued rows to reorder them
  - delete a row from the tray
- Expect the tray to preserve reply IDs/order updates locally and still assemble the same final blockquote format when sent.

## Validation

- `uv run ruff check scripts/concerto-dev.py scripts/install.py scripts/lib/trigger_scenarios.py` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ✅
