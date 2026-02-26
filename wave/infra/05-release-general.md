# 05: Release Ops Generalization

Status: **shipped** (branch `jack-heart.release-general.20260226_0637`)

## What shipped

- Added target-aware release config in `.lf/config.yaml` (`release.targets`) with per-target area scope, tag prefix, optional manifest overrides, and optional workflow override.
- Reworked `lf ops release` to resolve a target (`--target`), generate scoped tags/notes, and handle monorepos with independently versioned subprojects.
- Added inline manifest version bumping before release-note/PR generation so source control carries the real released version.
- Added automatic bootstrap mode when release infrastructure is missing, using an interactive agent session (`release_init`) instead of a separate `--init` command.
- Added post-tag workflow monitoring and release verification, with optional diagnosis flow (`release_diagnose`) on failures.
- Added `lf ops release --status` for quick release health checks (latest scoped tag, workflow run status, and GitHub Release presence).
- Migrated loopflow's Rust workspace versioning from phantom `0.0.0` to real inline version values and removed CI `sed` mutation steps from `.github/workflows/release.yml`.

## Follow-ups

- Replace workflow trigger string matching with structured workflow parsing to reduce false positives/negatives in bootstrap detection.
- Tighten manifest auto-detection heuristics so mixed-language repos only bump the minimal intended manifest set by default.
- Add end-to-end monorepo coverage for scoped target releases (`--target`, scoped tags, scoped notes) to prevent regressions in the new path.
- Keep watch on `gh pr list --json files` behavior; maintain/fallback logic may need updates if GitHub CLI response contracts change.
- Decide whether to quarantine or fix the intermittent `ConcertoUITests-Runner ... hung before establishing connection` failure observed during local gate runs.
