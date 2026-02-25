# Wave Memory — consolidated status (2026-02-25)

## Goal

Phase 02 makes wave-scoped agents carry forward durable learning by reading and updating `wave/<wave>/MEMORY.md` through normal prompt context.

## Current implementation state

Implemented on this branch:

- Added `DocumentSource::WaveMemory` and wired it through gather, prompt assembly, token accounting, and trimming.
- Wave-scoped runs now auto-include `wave/<wave>/MEMORY.md` when repo docs are enabled.
- `MEMORY.md` is excluded from regular wave docs and handled as its own source.
- `<lf:wave>` now includes memory guidance plus current memory content (or an explicit empty marker).
- Trimming drops wave memory before summaries/docs under pressure.
- Context header now reports memory tokens separately.
- Ops prompts (`update-wave`, `add-to-wave`) now require memory distillation into canonical docs and duplicate trimming.
- Tests cover memory load, formatting, trim order, and wave-doc separation.

## What still needs to be built

1. **Persistence hardening across execution surfaces**
   - Confirm no headless/session path can lose `MEMORY.md` edits due to detached/ephemeral workspaces.
   - If any path can lose edits, add explicit post-step/session flush behavior.

2. **Read-failure visibility**
   - `MEMORY.md` read errors currently soft-fail.
   - Decide whether to add warning/logging so silent misses are diagnosable.

3. **Memory size discipline**
   - Single-file memory can grow quickly.
   - Keep distilling durable content into canonical docs and trimming duplicated long-form memory.

## Non-goals for this phase

- Cross-wave/shared memory behavior.
- Automated aging/inheritance.
- New memory-specific APIs or tools.

## Validation status

Validated in this branch:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow wave_memory -- --nocapture`
- `cargo test -p loopflow --test context_tests`
- `cargo test -p loopflow golden_prompt`
- `uv run pytest python/tests/ -q`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`

Known environment caveat:

- `cargo test --all` fails locally on Docker recovery tests when `/var/run/docker.sock` is unavailable.
