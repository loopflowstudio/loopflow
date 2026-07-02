# Context generation review

Reworked prompt context gathering around explicit user inputs instead of the old
`lfdocs` product concept. Bare `lf <step>` includes loopflow guidance, native
agent instructions, ambient `scratch/`, and scoped wave docs/memory; repo docs
load only via `--docs`; context is measured, not trimmed. `--diff-files` was
restored as branch-file context (full bodies for files changed on the branch when
no explicit file list is given).

## Forward-looking notes

- **Ambient bloat is now a workflow responsibility.** Very large `scratch/` or
  wave memory stays in context. `measure_context()` makes the size visible, but
  cleanup is not automatic — the wave maintains its own memory.
- **`--diff-files` branch discovery mirrors raw diff tiering.** If a branch has
  committed changes, unstaged files outside that branch diff are not added by the
  automatic branch-file path list.
- **The explicit docs cap applies after resolution**, so broad globs/directories
  do their walk before failing over `MAX_EXPLICIT_DOC_FILES`.
- **Breaking config migration, by design.** `lfdocs`, `area`, and `budgets` were
  removed without compatibility shims.

## Validation

Passed: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --all`, `uv run python tests/goldens/update_goldens.py`,
`swift test --package-path swift`, `tests/e2e/test_smoke.sh`,
`uv run pytest python/tests/`,
`uv run python scripts/check_swift_multiplatform_boundaries.py`,
`uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py`,
`docker version && cargo test -p loopflow docker_ -- --nocapture`.

Not run (no rendering environment this headless run): `cd website && uv run
python dev.py test`; `cd swift && xcodegen generate && xcodebuild test ...`.
