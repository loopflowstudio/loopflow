# Context generation review

## What was implemented

Prompt context gathering now treats `scratch/`, scoped `wave/` docs, wave
memory, native agent guidance, and loopflow guidance as ambient working state.
Repo docs are explicit additive context via `--docs` or `docs:` config.
`lfdocs`, `area`, context budgets, `<lf:docs>`, and the old `OPERATE.md` naming
were removed; `LOOPFLOW.md` renders as `<lf:loopflow>` and is default-on with a
`--no-loopflow` opt-out.

Context is measured with `measure_context()` instead of trimmed. Session
snapshots and the Concerto context UI now report total size, source counts, and
document drill-down without a budget percentage.

## Key choices

- `--docs` accepts files, globs, directories, and related-repo targets. Broad
  targets fail over 100 resolved files instead of silently trimming.
- Directory docs now use the same gitignore, `.lf/`, lockfile, binary, and
  dedupe rules as file/glob targets.
- `lf-prompt` drops native instruction docs before rendering so prompt
  inspection matches normal launch behavior.
- `--diff-files` remains an explicit switch. When enabled without explicit
  files, it includes full bodies for files changed on the branch.
- Removed config keys are breaking by design because this is internal config.

## How it fits together

`GatherContextOpts` carries explicit switches (`docs`, diff, diff-files,
clipboard, loopflow). `gather_documents()` always loads ambient state first, then
explicit docs, then optional changed file bodies. `prepare_launch_prompt()` drops
native instruction files, adds summaries, measures the final components, and
renders system-safe sections separately from repo/user content.

## Risks and bottlenecks

- Ambient `scratch/` and wave memory can grow large; the new behavior surfaces
  size but does not trim. Cleanup remains a workflow responsibility.
- The explicit docs cap applies after target resolution, so very broad
  directories/globs still walk before failing.
- Branch-file discovery mirrors raw diff tiering: if committed branch changes
  exist, automatic `--diff-files` uses those files and does not add unrelated
  unstaged files outside the branch diff.

## What's not included

No compatibility shim for `lfdocs`, `--area`, `--operate`, or budget config.
Browser-facing website tests and generated Xcode project UI tests were not run
because this headless gate has no rendering environment.

## Validation

Passed:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run python tests/goldens/update_goldens.py`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `uv run python scripts/check_swift_multiplatform_boundaries.py`
- `docker version && cargo test -p loopflow docker_ -- --nocapture`
- `git diff --check`

Acceptance probes:

- Bare `lf-prompt --step gate --diff-files false --diff false` renders
  `<lf:loopflow>` and `<lf:scratch>`, with no root `README.md` file section and
  no rendered `<lf:docs>` section.
- `lf-prompt --docs README.md,swift/ --diff-files false --diff false` adds
  explicit README/Swift docs while preserving scratch context.
- `lf --help` and `lf-prompt --help` expose `--docs`, `--diff-files`, and
  `--no-loopflow`; removed `--area`, `--lfdocs`, `--no-lfdocs`, and `--operate`
  do not appear.
