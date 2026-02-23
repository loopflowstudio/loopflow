# Prompt Pipeline (Stage 04) — Consolidated

## Goal
Finish stage 04 by keeping prompt assembly typed, parity-safe, and easy to extend.

## Current state
The branch has completed the stage 04 refactor:

- `DocumentSource` replaces string categories on `Document`.
- `GatherSpec { sources, repo_root, files, area, wave }` drives one gather path via `gather_documents(&GatherSpec)`.
- `PromptFormatMode` drives one formatting entrypoint (`format_prompt`) with thin wrappers for context/task.
- `ContextBreakdown` accounting is keyed by `DocumentSource`.
- Callsites now pass typed source lists (`default_gather_sources(...)`).
- Files-only context no longer auto-includes branch diff unless `DocumentSource::Diff` is requested.

## Design decisions to keep

- **Single control plane for gathering**: no dual boolean/string routing.
- **Parity-first ordering**: keep gather order stable (`scratch -> wave -> repo docs`, then area/files).
- **No compatibility shim**: remove replaced paths in the same change.
- **Diff/source decoupling**: explicit source choice controls diff inclusion.

## Verification already run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cargo test -p loopflow prompt`
- `cargo test -p loopflow golden_prompt`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` *(environmental LocalAuthentication cancel on local machine)*

## Remaining follow-ups

- `engine/prompt.rs` is still large; split for maintainability in a later stage.
- `DocumentSource::Summary` exists, but summary gathering in core flow remains TODO.

## Out of scope for this branch

- Store/backend/sql catalog changes
- Executor decomposition
- Prompt content redesign or new CLI surface
