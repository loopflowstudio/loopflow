# Review: `.agents/skills` injection + `npx:` skill source

## What was implemented

- Moved skill injection output from flat `.claude/commands/*.md` files to `.agents/skills/<name>/SKILL.md` directories.
- Extended repo-local step injection to include namespaced `.lf/steps/**` markdown files, flattening names like `scan/scan-report` to `scan-scan-report`.
- Added frontmatter projection when injecting loopflow steps/directions into Agent Skills format:
  - always sets `name`, `description`, and `loopflow: true`
  - maps non-interactive steps to `disable-model-invocation: true`
  - marks directions as `user-invocable: false`
  - passes through supported SKILL.md-native fields (for example `model`, `allowed-tools`, `context`, `agent`, `argument-hint`).
- Updated cleanup to remove injected directories (not just files).
- Expanded CLI/daemon/wave injection callsites to treat `.agents/skills` as the auto-discovery surface.
- Added `npx` skill source support in discovery:
  - repo-local cache at `.agents/skills/`
  - cache-first lookup
  - exact `npx skills add <name>` fallback
  - `npx skills find <name>` + follow-up add fallback
  - skip loopflow-injected skills via `loopflow: true` marker.
- Updated docs to include `npx:` examples and call out `.agents/skills` cache behavior.

## Key choices

- **Directory-level no-clobbering**: injection skips if `.agents/skills/<name>/` already exists, preserving user-installed/custom skills.
- **Projection over raw copy**: loopflow step frontmatter is projected to SKILL.md-compatible fields so all supported agents can auto-discover skills consistently.
- **Single cache path**: reused `.agents/skills/` as both runtime discovery path and npx cache to avoid parallel cache state.
- **Marker-based loop avoidance**: `loopflow: true` marker prevents re-discovery loops between injected built-ins and npx-sourced skills.

Alternatives rejected:
- Keeping `.claude/commands` injection as primary output (would remain Claude-specific and miss the open standard path).
- Separate cache directory for npx (would add invalidation/sync complexity with no clear gain).

## How it fits together

`inject_skills()` now materializes built-ins and repo-local prompts as Agent Skills in `.agents/skills`. Discovery adds an `npx` source that checks that same directory first, then fetches on miss with `npx skills` commands. Runtime launch paths (`lf`, `lfd` sessions, wave executor) inject and later clean up the directories they created.

## Risks and bottlenecks

- `npx skills find` output is parsed heuristically; upstream output format changes could reduce fallback reliability.
- Fetch-on-miss depends on local Node/npx availability and network access; failures degrade to “skill not found”.
- Repo-local directions are still injected from top-level `.lf/directions/*.md` files only (group subdirectories are not projected yet).

## What's not included

- No Gemini TOML command projection.
- No changes to the existing step resolution chain (`.lf/steps` → `.claude/commands` → global → built-ins).
- No migration/removal logic for pre-existing `.claude/commands` skills.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`

All passed.
