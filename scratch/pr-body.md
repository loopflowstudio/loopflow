## Try it!
- `cargo fmt --check && cargo clippy -- -D warnings && cargo test --all`
- `swift test --package-path swift`
- Open Concerto without selecting a wave: the repo window now lands on the attention queue, with `Ship` for code review items and `Retry` for step failures.
- If you want the Xcode scheme run too: `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`

Validation on March 17, 2026:
- ✅ Rust fmt/clippy/test all passed
- ✅ `swift test --package-path swift` passed
- ⚠️ `xcodebuild test ...` ran the scheme's unit tests but `ConcertoUITests-Runner` exited early before bootstrapping on this machine (same failure twice)

## Intent
Make Concerto behave like a conductor's queue instead of an empty detail pane by introducing durable attention items end to end: storage, API, websocket events, shared Swift models/state, and the macOS queue view itself.

## Assumptions
- Attention items remain projections of underlying domain state; actions still go through existing wave/domain APIs.
- Queue failures should be stable per run, so repeated reconciliation should update one item instead of resurfacing a new one.
- Existing queue failures can be recomputed after upgrade, so historical `wave_queue_blocks` rows do not need to be migrated forward.

## Key decisions
- Use one `attention_items` table plus JSON context instead of per-kind tables.
- Replace the default repo-window empty state with `AttentionQueueView` instead of adding another navigation mode.
- Preserve queue-failure `surfaced_at` / `viewed_at` across repeated poll cycles and emit queue-failure lifecycle websocket events from poll, webhook, and run-completion paths so the queue stays live.

## Not included
- Executor hooks for `design_review` and `calibration` attention items.
- Historical backfill of legacy `wave_queue_blocks` rows during migration.
