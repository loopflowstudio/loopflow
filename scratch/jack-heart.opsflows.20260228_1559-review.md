# Release Cadence: Design Review

## What was implemented

Decomposed `lf ops release` into five independent CLI commands (`release-check`, `release-notes`, `release-bump`, `release-tag`, `release-status`) backed by public functions extracted from `release.rs`. Updated the `release` step prompt to orchestrate these commands in sequence. Added wave configs for daily patch and monthly minor cron releases. Installed `lf` in the agent container.

## Key choices

- **Sibling OpsCommand variants, not nested subcommands.** Avoids clap ambiguity with the positional `version` arg on `Release`. `lf ops release patch` stays unambiguous as the monolith.

- **`resolve_repo_and_target()` helper.** DRY extraction of the three-line preamble shared by all release functions. Also refactored into existing `publish_release` and `release_status` for consistency.

- **`release-check` exits 1 for "nothing merged."** Follows Unix convention (`grep` returns 1 for no matches). The step prompt uses this exit code to skip cleanly.

- **Structured output from `release-check`.** JSON when piped, human-readable table when TTY. Agents get structured data; humans get scannable output.

- **Monolith preserved.** `publish_release()` retains its orchestration logic (resume, bootstrap, worktree). The decomposed functions are called by the step prompt, not by the monolith — avoiding coupling.

## How it fits together

```
Step prompt (lf release)
  ├── lf ops release-check   → release_check()
  ├── lf ops release-bump    → release_bump()  → bump_manifest_versions()
  ├── lf ops release-notes   → release_notes() → merged_prs_since() + generate_release_notes()
  ├── lf ops commit + land
  ├── lf ops release-tag     → release_tag()   → tag_and_push()
  └── lf ops release-status  → release_status()

Wave configs (cron)
  ├── wave/release-patch/ → daily at 2 AM, message: patch
  └── wave/release-minor/ → monthly at 2 AM, message: minor
```

All decomposed functions share `resolve_repo_and_target()` for consistent target/repo resolution. The monolith (`lf ops release patch`) still works independently via its own orchestration path.

## Risks and bottlenecks

- **`release-check` uses `std::process::exit(1)` in the CLI handler.** This bypasses normal error return. Intentional for exit code semantics, but means any future cleanup in the caller won't run. Acceptable since this is a leaf CLI command.

- **No version resolution in decomposed commands.** `release-bump 0.9.6` takes an explicit version, not `patch`/`minor`. Version resolution lives in the step prompt (step 2). If someone calls `release-bump patch` directly, `normalize_version` strips the 'v' prefix but doesn't resolve — it would try to set version to "patch". This is by design (the commands are building blocks, not workflows) but could confuse direct users.

- **`tag_and_push` is not idempotent.** `git tag` fails if the tag exists. The step prompt handles re-entry by checking for existing tags, but `release_tag()` itself will error. Acceptable — the step prompt owns idempotency.

## What's not included

- Concerto UI ("Release Now" button, config editing) — deferred to follow-up wave.
- Refactoring monolith internals to call decomposed functions — the monolith works and has its own orchestration (resume, bootstrap) that doesn't map 1:1 to the decomposed flow.
- `release-notes` test — requires mocking both `gh` and `claude` CLI; existing `generate_release` tests cover the same code path.

## Gate fix applied

- `install-loopflow.sh`: Changed unsupported-arch handling for `lf` from `exit 1` (kills entire script) to warning and skip. Also removed redundant `arch` re-computation (already set earlier in the script). Consistent with the "optional" intent in the design doc.
