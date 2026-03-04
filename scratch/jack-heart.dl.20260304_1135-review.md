# Review: `lf ops release run` and CI improvements

## What was implemented

Added `lf ops release run` — a deterministic, end-to-end release command that owns the full lifecycle: check merged PRs, create a release worktree, bump manifests, generate notes, commit/PR/land, wait for merge, tag the merged commit, and wait for the release workflow. Three supporting changes ship alongside:

1. **Auto-tag version verification** — the auto-tag workflow now checks both Cargo.toml and pyproject.toml match RELEASE_NOTES.md before creating a tag
2. **SPM resource bundle copy** — concerto-dev.py copies `.bundle` directories from the build output into the `.app` root so Bundle.module resolves at runtime
3. **Release step simplification** — the builtin release step is now a thin wrapper that calls `lf ops release run <version>`

## Key choices

**Deterministic orchestration over agent orchestration.** The release flow was previously agent-driven (agent calls `lf ops` subcommands). Now `release_run()` is a single Rust function that calls subcommands internally. The agent is reduced to extracting the version and invoking one command. This matches the design doc's principle: ops orchestrate, steps provide judgment.

**Worktree-based release prep.** Release changes (version bumps, notes) happen in a temporary worktree branched from main. The worktree is cleaned up regardless of success/failure, keeping the user's working tree untouched.

**Idempotent tagging.** `tag_and_push_ref` checks both local and remote tag state. If the tag already exists at the target commit, it's a no-op. If it exists at a different commit, it errors. Concurrent tag pushes (race with auto-tag workflow) are tolerated.

**Polling with backoff.** PR merge and release workflow completion use 10-second polling with 1-hour timeouts. Progress messages emit every 60 seconds (every 6th attempt).

## How it fits together

```
lf release <version>        (step — extracts version, calls ops)
  └─ lf ops release run     (Rust — deterministic orchestrator)
       ├─ release_check     (merged PRs since last tag)
       ├─ create_with_schema (release worktree)
       ├─ bump_manifest_versions + generate_release_with_target
       ├─ commit_workflow + land (PR creation + auto-merge)
       ├─ wait_for_pr_merge (polling)
       ├─ tag_and_push_ref  (idempotent)
       └─ wait_for_release_publication (polling)
```

The decomposed subcommands (`check`, `notes`, `bump`, `tag`, `status`) remain individually callable for debugging and manual recovery.

## Risks and bottlenecks

- **1-hour timeout on merge queue wait.** If merge queue is backed up or stuck, the release will time out. Recovery: re-run `lf ops release run` — idempotent tagging and `skip-existing` on publish handle this.
- **Worktree cleanup happens before PR merge wait.** The worktree is removed immediately after `land()` enqueues auto-merge, so if land fails mid-way, the worktree may already be partially cleaned. The `prepared` result is captured before cleanup, and cleanup errors are warnings, not failures.
- **Auto-tag workflow and `release_run` can race on tagging.** Both try to create and push the same tag. The concurrent-push tolerance in `tag_and_push_ref` handles this — if push fails with "already exists" but the remote tag matches, it continues.

## What's not included

- **Narrative release notes** — still mechanical PR lists. The design doc calls for a `release-notes` step that writes narrative notes; this PR ships the orchestration layer that will invoke it.
- **`OpsItem` in flows** — the design doc describes `ops: land` and `ops: release run` as flow items. Not built in this PR; flows still use step wrappers.
- **Staleness check for PR copy** — `land` still requires explicit title/body. The caching/staleness mechanism from the design doc is future work.
