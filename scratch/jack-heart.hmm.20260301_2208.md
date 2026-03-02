# ops = no agents

## Vision

Every `lf ops X` command is purely mechanical — no LLM calls, no agent launches. Exit 0 on success, non-zero on failure. Steps (`lf X`) provide the judgment layer: fast-path the mechanical command, agent only when it fails or when judgment is needed.

This makes the ops layer a reliable API for scripts, daemons, and agents alike. And it makes steps the user-rewritable surface — swap out `lf land` by dropping a new `land.md` in `.lf/steps/`.

---

## PR 1: worktree improvements + rebase/lint extraction

One commit. All changes are independent deletions or small additions — no conflicts between them.

### Worktree improvements (done)

- `preserve_worktree` uses human-readable timestamps (`YYYYMMDD_HHMM`) instead of unix epoch
- `wt_switch` matches by wave name (via `wave_name_from_worktree_and_main`), with dot-delimited prefix fallback for timestamped/preserved worktrees, then raw directory name
- Tests for all of the above

### `git worktree prune` in `wt prune`

Before pruning loopflow worktrees, run `git worktree prune` to clean up stale git worktree bookkeeping (e.g. worktrees whose directories were manually deleted). Without this, `git worktree list` returns stale entries and subsequent `git worktree add` can fail.

### Extract rebase agent from `ops/rebase.rs`

**Today:** `rebase_with_recovery()` tries `git rebase`, on conflict launches the builtin rebase step agent internally.

**After:** `rebase_with_recovery()` tries `git rebase`. Conflict → return error. The `lf rebase` step already has `fast-path: lf ops rebase` — when the fast-path fails, the step agent handles conflict resolution.

`rebase_with_recovery()` is also called from `lf ops land`, `lf ops pr`, and the daemon's `post_step_sync`. After this change, those commands fail on conflict instead of silently resolving it. That's correct — the step layer handles recovery.

Delete `run_rebase_agent()` and its agent imports.

### Remove lint from ops commands

Linting is its own concern. Ops commands shouldn't lint.

**Today:** `ensure_lint_passes()` runs lint, on failure launches the lint step agent to fix it, re-runs lint. Called from `commit_workflow`, `create_or_update_pr`, and `prepare_land` behind `options.lint` flags.

**After:** Delete `ensure_lint_passes()`, `run_lint_agent()`, and all lint calls from commit/pr/land. The `lf lint` step exists for when you want to lint. Flows that need lint-before-commit sequence it as a separate step.

Remove the `lint` field from `CommitOptions`, `PrOptions`, and `LandOptions`. Remove `OpsError::LintFailed` if it becomes unused.

### Done when (PR 1)

- `run_rebase_agent` deleted from `ops/rebase.rs`
- `ensure_lint_passes` and `run_lint_agent` deleted from `ops/lint.rs`
- No lint calls in `ops/commit.rs`, `ops/pr.rs`, or `ops/land.rs`
- `lf ops wt prune --force` runs `git worktree prune`
- Tests pass

---

## Future PRs

### PR 2: commit message + PR message extraction

**Commit:** `lf ops commit` without `-m` fails with "message required." The `lf commit` step generates a message and calls `lf ops commit -m "..."`.

**PR:** `lf ops pr` requires `--title` and `--body` flags (or `--draft --fill` for draft PRs from commit messages). New `lf pr` step reads the diff, writes title/body, calls `lf ops pr --title "..." --body "..."`.

Daemon callers (`auto_create_pr`, `post_step_sync`) route through steps instead of calling ops directly when they need LLM-generated messages.

### PR 3: release cleanup

Remove `publish_release` (the ops-layer orchestrator) from `ops/release.rs`. `lf release` (the step) becomes the single orchestrator — it calls the decomposed ops primitives (`release-check`, `release-bump`, `release-tag`, `release-status`) and adds judgment for notes and diagnosis.

Keep all decomposed primitives. Drop or make mechanical: `release-notes` (dump raw changelog, no narrative). Remove `diagnose_release_failure` and `bootstrap_release` from ops — those belong in steps or `lf init`.

### Done when (all PRs)

- `grep -r "launch_agent\|run_builtin_agent\|launch_ops_agent" rust/loopflow/src/ops/` returns nothing
- Every `lf ops X` command works without network access to any LLM provider
- `lf pr` exists as a step
- `lf ops release` orchestrator is gone (sub-commands remain)
- `lf release` (step) orchestrates the full release workflow
- Cron release waves work through `lf release` step
