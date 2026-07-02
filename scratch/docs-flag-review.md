# Context generation review

## What was implemented

Reworked prompt context gathering around explicit user inputs instead of the old `lfdocs` product concept.

Bare `lf <step>` now includes loopflow guidance, native agent instructions, ambient `scratch/`, and scoped wave docs/memory. Repo docs load only when requested with `--docs`, and context is measured instead of trimmed. The prompt tags and session context metadata now use `docs` instead of `repo_doc` / `area`, and `OPERATE.md` became `LOOPFLOW.md`.

Gate polish restored `--diff-files` as branch-file context: when enabled with no explicit file list, it loads complete bodies for files changed on the branch. Explicit file lists still stay narrow.

## Key choices

- `--docs` is additive and explicit. It accepts files, globs, directories, and related-repo targets while leaving ambient scratch and wave context intact.
- Context over the explicit docs cap fails instead of trimming. The caller sees the size problem and narrows the request.
- Token accounting remains a visibility layer only. `measure_context()` reports source totals, document entries, diff tier, and total tokens without mutating gathered context.
- `--diff-files` follows existing diff-tiering branch detection: prefer the default remote branch diff, and fall back to `HEAD` when there are no committed branch changes or no remote.
- `--no-loopflow` is the opt-out. Loopflow operating guidance is default-on for launch prompts.

## How it fits together

`prepare_launch_prompt()` merges config `docs` with CLI `--docs`, then builds `GatherContextOpts` with explicit booleans for diff, diff files, clipboard, and loopflow guidance. `gather_context()` loads step/directions, calls `gather_documents()` for ambient and explicit documents, separately gathers raw diff and clipboard, and returns full `PromptComponents`. Launch formatting measures those components and renders the same untrimmed context into CLI, Claude system/task, and session snapshot surfaces.

## Risks and bottlenecks

- Config migration is breaking by design: `lfdocs`, `area`, and `budgets` are removed without compatibility shims.
- Very large ambient `scratch/` or wave memory now stays in context. Measurement makes the problem visible, but cleanup remains a workflow responsibility.
- `--diff-files` branch discovery mirrors raw diff tiering. If a branch has committed changes, unstaged files outside that branch diff are not added by the automatic branch-file path list.
- The explicit docs cap applies after resolution, so broad globs/directories can still do work before failing.

## What's not included

- No compatibility path for old config keys or removed CLI flags.
- No context trimming or budget override replacement.
- No website or Xcode project UI validation in this headless gate run; the run context said no rendering environment.

## Validation

Passed:

- `cargo fmt`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow`
- `cargo test --all`
- `uv run python tests/goldens/update_goldens.py`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest python/tests/`
- `uv run python scripts/check_swift_multiplatform_boundaries.py`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `docker version && cargo test -p loopflow docker_ -- --nocapture`

Not run:

- `cd website && uv run python dev.py test`
- `cd swift && xcodegen generate && xcodebuild test ...`
