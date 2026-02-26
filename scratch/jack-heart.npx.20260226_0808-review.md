# Review: `.agents/skills` injection + `npx:` skill source

## What was implemented

- Moved skill injection output from flat `.claude/commands/*.md` files to `.agents/skills/<name>/SKILL.md` directories.
- Added frontmatter projection from loopflow step/direction frontmatter to SKILL.md-compatible fields:
  - always emits `name`, `description`, `loopflow: true`
  - maps non-interactive steps to `disable-model-invocation: true`
  - marks injected directions as `user-invocable: false`
  - passes through supported SKILL-native fields (for example `model`, `allowed-tools`, `context`, `agent`, `argument-hint`).
- Kept no-clobber behavior by skipping existing skill directories and updated cleanup to remove injected directories.
- Added `npx` discovery + fetch support:
  - cache-first from `.agents/skills/`
  - fallback to `npx skills add <name>`
  - fallback to `npx skills find <name>` then add.
- Hardened `npx` fallback parsing:
  - passes `--yes` through to the skills CLI to avoid interactive hangs
  - handles ANSI-colored `skills find` output
  - handles `owner/repo@skill` and resolves installed cache directory names.
- Ensured runtime step loading can consume resolved external skills without re-looking up by name:
  - `LaunchPromptInput` now accepts `resolved_step`
  - prompt prep merges step directions and uses resolved step content/model directly.
- Updated step argument splitting so source-prefixed steps like `npx:explain-code` and `sp:brainstorm` are preserved.
- Added fallback step/direction loading from `.agents/skills/<name>/SKILL.md` in flow loading paths.

## Key choices

- **Single canonical skill path**: use `.agents/skills/` for both injection and npx cache to align Claude/Codex/OpenCode discovery and avoid cache divergence.
- **Projection, not raw copy**: injected files are normalized to SKILL.md expectations instead of preserving loopflow-only frontmatter.
- **Explicit resolved-step plumbing**: when discovery already resolved a step (for example `npx:`), prompt assembly uses that object directly instead of a second lookup path.
- **Parser robustness over strict format assumptions**: `npx skills find` parsing strips ANSI and accepts qualified `owner/repo@skill` hints.

## How it fits together

Loopflow now injects built-ins and repo prompts into `.agents/skills/*/SKILL.md`, and runtime discovery can load skills from that same location directly or via `npx:` fetch-on-miss. CLI prompt preparation can take a pre-resolved skill step, merge its directions, and render prompt context without relying on legacy step-path lookup.

## Risks and bottlenecks

- `npx skills find` parsing remains heuristic; upstream CLI output changes could still reduce fallback reliability.
- Fetch-on-miss depends on local Node/npx availability and network access.
- macOS UI tests are still environment-sensitive (see validation notes).

## What's not included

- No Gemini TOML command projection.
- No migration/removal path for existing `.claude/commands` content.
- No changes to non-`npx` external providers beyond compatibility with source-prefixed step parsing.

## Validation run

Passed:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`

Attempted but failed due UI runner crash (not compile/test assertion failures in the changed Rust paths):
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- Failure: `ConcertoUITests-Runner ... Early unexpected exit, operation never finished bootstrapping`.
