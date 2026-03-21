# Review: actionable roadmap in the Concerto workspace

## What was implemented

This branch closes the loop between planning and execution inside Concerto's workspace.

- `lf ops ingest` now accepts `--item <filename-or-slug>` so the backend can ingest a specific roadmap file instead of always auto-picking the highest-priority item.
- Concerto's roadmap pane now makes roadmap cards actionable: each non-shipped item shows an inline summary, a priority picker, and a play button that ingests that exact item and starts the wave's configured flow.
- Roadmap priority changes are expressed as file renames (`1-` through `4-`) so the filesystem remains the source of truth.
- The workspace and daemon plumbing in this branch support that loop end-to-end: run overrides carry the selected roadmap item through `lfd`, wave content parsing understands priority-prefixed roadmap files, and roadmap state refreshes after ingest or reprioritization.

## Key choices

- **Targeted ingest stays in Rust.** Concerto does not copy roadmap files itself; it asks `lfd`/`lf ops ingest` to do the same move-and-delete flow the CLI already owns.
- **Priority stays encoded in filenames.** Reprioritization renames the file instead of introducing metadata or sidecars, which keeps Git history legible and wave directories simple.
- **Workspace cards show content by default.** The first few lines are always visible, with expansion for the larger preview, so roadmap scanning does not require opening every item.
- **Configured flow remains authoritative.** The roadmap play button runs the wave's configured flow when present, falling back to `build` only when the wave has no explicit flow.

## How it fits together

The Rust side extends ingest and run plumbing so a specific roadmap item can be selected, validated, and moved into `scratch/`, then executed through the normal wave-run path. The Swift side parses the roadmap files directly from `wave/<name>/`, renders them in the multiplexer roadmap pane, and uses `RepoState.ingestAndBuild` / `updateRoadmapPriority` to bridge UI actions back to the backend and local filesystem.

## Risks and bottlenecks

- Reprioritization is local-only because it renames files in the checked-out repo; remote repos still need a separate edit path.
- The roadmap pane assumes one stable slug per wave item. Filename collisions on rename are handled as an error, but duplicate slugs across priorities would still be a workflow problem.
- Full `cargo test --all` could not complete inside this sandbox because several HTTP/socket tests need to bind listeners or Unix sockets, which the environment denies.
- `xcodebuild test` could not complete in this sandbox because package resolution needs network access and Xcode cache paths outside writable roots.

## What's not included

- No drag-to-reorder or arbitrary total ordering beyond the four priority buckets.
- No Swift-side reimplementation of ingest semantics.
- No broader multiplexer interaction follow-ups like directional focus, named layouts, or richer markdown/diff browsing.

## Validation

Passed locally in this environment:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `.venv/bin/pytest python/tests/`
- `cargo test -p loopflow ops::ingest::tests -- --nocapture`
- `swift test --package-path swift`
- `swift test --package-path swift --filter WaveContentParser`
- `swift test --package-path swift --filter Multiplexer`

Environment-limited here, not completed:

- `cargo test --all` — sandbox blocks listener / Unix socket binding used by several networked tests
- `xcodebuild test -project swift/LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` — sandbox blocks cache writes and package fetching
