# Release system redesign review

## What was implemented

- Added release target config support in `.lf/config.yaml` (`release.targets`) with per-target area scope, tag prefix, manifest list, and workflow override.
- Reworked `lf ops release` into target-aware publishing:
  - resolves a release target (`--target`),
  - bumps inline manifest versions before release PR creation,
  - scopes tags/PR collection/release notes by target,
  - tags as `{prefix}vX.Y.Z`,
  - monitors GitHub workflow and release publication after tagging.
- Added bootstrap path when no tags/workflow are found for a target (interactive agent via `release_init` prompt).
- Added post-failure diagnosis path (`release_diagnose` prompt) with optional retag/retry loop.
- Added `lf ops release --status` to report latest scoped tag, workflow run state, and GitHub Release presence.
- Migrated loopflow workspace versioning to real inline version values (`Cargo.toml`), and removed CI `sed` version patching from `.github/workflows/release.yml`.
- Updated release docs (`docs/lfops.md`, README ops table) for target/status/bootstrap behavior.

## Key choices

- **Inline manifest bumping over CI mutation:** source now carries the real version at commit time; CI no longer rewrites manifests.
- **Scoped tags and notes for monorepos:** tag prefix + area filter keep release artifacts specific to the selected target.
- **Heuristic workflow detection for bootstrap gate:** checks workflow files for `tags:` + `{prefix}v*` instead of full YAML parsing (simpler, may need hardening later).
- **Failure recovery via agent session:** on workflow failure, user can opt into guided diagnosis/fix/retry instead of manual triage.

## How it fits together

`lf ops release` now resolves a target from config, checks bootstrap readiness, creates a release worktree, bumps manifest versions, generates scoped notes, lands the PR, tags and pushes, then monitors GitHub Actions + release existence. The status command reuses the same tag/run/release lookup path for quick visibility. Prompt templates (`release_notes`, `release_init`, `release_diagnose`) carry target context so agent output stays scoped.

## Risks and bottlenecks

- Workflow detection is string-based and can false-positive/false-negative on unusual YAML layouts.
- Manifest auto-detection may still pick more files than desired in mixed-language repos.
- `gh pr list --json files` fallback logic depends on GitHub CLI/API behavior and could require maintenance if output contracts change.
- UI test stability remains a CI/runtime risk: local `xcodebuild test -scheme Concerto` hit a `ConcertoUITests-Runner ... hung before establishing connection` failure during this gate run.

## What's not included

- No structured YAML parser for workflow trigger detection yet.
- No deeper heuristics to minimize auto-detected manifest set beyond known-manifest scanning at area roots.
- No extra CLI surface for selecting diagnosis retry policies; current retry loop is bounded in code.
- No additional end-to-end monorepo fixture test that drives full publish path with multiple real targets.
