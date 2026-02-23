# Gate Review — Prompt Pipeline (stage 04)

## What was implemented

- Replaced stringly document categories with a typed `DocumentSource` enum and migrated `Document` to `source`.
- Replaced boolean-driven gather routing with `GatherSpec { sources, repo_root, files, area, wave }` and unified gathering through `gather_documents(&GatherSpec)`.
- Unified prompt assembly with `PromptFormatMode` and a single `format_prompt(mode, ...)` entry point, with `format_context_prompt` and `format_task_prompt` now thin wrappers.
- Moved token accounting to enum-keyed maps in `ContextBreakdown`.
- Updated all callsites (`lf run`, `lf-prompt`, ops helpers, lint/rebase flows, executor) to pass typed gather sources via `default_gather_sources(...)`.
- Migrated tests to typed sources and updated golden prompt coverage.
- Gate polish fix: files-only context no longer accidentally pulls branch diff; added regression test `gather_context_with_specific_files_does_not_pull_branch_diff`.

## Key choices

- **Single control plane**: source selection now flows through `sources: Vec<DocumentSource>` instead of multiple booleans; this removes split routing logic.
- **Parity-first ordering**: gather dispatch preserves legacy ordering (`scratch -> wave -> repo docs`, then area/files) to avoid prompt-content churn.
- **No compatibility shim**: old category strings and gather booleans were removed instead of maintained in parallel.
- **Diff/source decoupling for files-only requests**: branch diff inclusion now depends on explicit `DocumentSource::Diff` in opts; explicit file includes still work without auto-including branch diff text.

## How it fits together

`GatherContextOpts` now converts to a normalized `GatherSpec`, and `gather_documents` is the sole dispatcher for context documents. `gather_context` splits gathered documents by typed source into docs/summaries/area/diff files, then attaches runtime sections (step, directions, diff text, clipboard, loopflow docs). Formatting runs through one mode-based function (`Full`, `Context`, `Task`) so callsites don’t duplicate assembly logic.

## Risks and bottlenecks

- `engine/prompt.rs` is still large, so future edits can still be high-friction even with typed routing.
- `DocumentSource::Summary` is wired but summary gathering remains stubbed in core gather flow (`TODO`), so summary behavior is still mostly injected by higher layers.
- Local Concerto UI test run via xcodebuild can fail on machine auth state (`LocalAuthentication Code=-2`), which is environmental rather than tied to this Rust refactor.

## What's not included

- No store/backend/sql catalog changes.
- No executor decomposition work.
- No prompt content redesign beyond parity-preserving routing/typing refactor.
- No new user-facing CLI flags; source selection changes are internal API cleanup.

## Verification run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cargo test -p loopflow prompt`
- `cargo test -p loopflow golden_prompt`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` *(fails locally due LocalAuthentication cancel in UI runner; unit/package tests pass)*
