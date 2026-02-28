# Opsflows Wave Plan — Review

## What was implemented

New wave `opsflows` with 4 sprint items, a README, and wave config. Sprint 01 (land + rotation) has been ingested into `scratch/opsflows-land-rotation.md` with a detailed design doc covering fast-path, worktree rotation, step prompt, and prune protection.

No code changes — this is a wave plan branch.

## Key choices

**Ordering:** Sprints 01 and 04 share the `fast-path` dependency — 01 builds it, 04 consumes it. Sprints 02 and 03 are a separate dependency chain (release notes quality → release cadence). This lets 02 start before 01 ships.

**`fast-path` as general infrastructure:** Rather than hardcoding land/rebase behavior, `fast-path` is a step frontmatter field any step can use. This means future ops-as-steps (deploy, migrate, etc.) get the same pattern for free.

**Dot heuristic for shortname detection:** `preserve_worktree()` always produces `{name}.{unix_ts}`, so `.contains('.')` reliably distinguishes preserved from active worktrees. No filesystem lookups needed.

**Rotation on intent, not merge:** Worktree rotates when `lf land` commits intent to merge (enables auto-merge), not when CI finishes and the PR actually merges. The user shouldn't wait.

## How it fits together

```
wave/opsflows/           → wave plan (this branch)
scratch/opsflows-land-rotation.md  → sprint 01 design doc

Sprint 01: fast-path + land rotation (in progress)
Sprint 02: release notes quality (independent)
Sprint 03: release step + cadence (depends on 02)
Sprint 04: rebase step (depends on 01's fast-path)
```

The opsflows wave uses the `ship-wave` flow (ingest → kickoff → build) with `alive` + `simplicity` directions. Area scopes to ops Rust code, builtin ops prompts, lfd triggers, and Swift.

## Risks and bottlenecks

- **Worktree rename while cwd is inside it.** Design addresses this: CLI uses `write_shell_directive` to cd the user's shell; lfd renames post-run.
- **Release notes quality is subjective.** Sprint 02 is prompt-driven — expect iteration after seeing real output.
- **Ops decomposition scope.** Sprint 03 splits `lf ops release` into sub-commands. Biggest Rust surface area. Existing monolithic command should keep working during transition.

## What's not included

- Concerto UI for release config (sprint 03 defines it but it's a separate implementation concern)
- lfd wave scheduling changes (daemon already handles step advancement)
- Any code implementation — this is purely the wave plan

## Unstaged changes

The working tree has editorial improvements not yet committed:

1. **`01-land-rotation.md` deleted** — ingested to `scratch/`. Standard wave workflow.
2. **`02-release-notes.md` trimmed** — removed sub-agent overflow detail and noisy code comments. Cleaner.
3. **`03-release-cadence.md` consolidated** — merged redundant ops API listing and step frontmatter. More direct.
4. **`README.md` refined** — resolved the worktree risk with concrete mitigation (`write_shell_directive` + post-run rename). Removed cron metric (captured in sprint 03's done-when).

All improvements. Should be staged and committed before merging.
