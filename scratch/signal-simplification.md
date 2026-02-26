# Signal Simplification

Unify CI fix and listen under a single stimulus activation model. Drop `WaveRunKind`, `CiFixKind`, and the dedicated ci_fix executor. Rename `StimulusKind` to `Signal`.

## Problem

CI fix runs bypass the stimulus model entirely. They have their own Rust type system (`WaveRunKind::CiFix`, `CiFixKind`), their own executor (`ci_fix.rs`), and their own triggering path (webhook → `spawn_ci_fix_agent` directly). Meanwhile, listen stimuli go through the proper activation pipeline.

This means two parallel systems for "something happened, run a flow in response." Every new reaction type (lint-fix, rebase, deploy) would need its own Rust enum variant, executor module, and special-case wiring.

## Design

### Rename: `StimulusKind` → `Signal`

`kind` is non-descriptive. A stimulus responds to a signal. The signal is the event type.

```rust
pub enum Signal {
    Once = 1,
    Loop = 2,
    Watch = 3,
    Cron = 4,
    Listen = 5,
    CiFailure = 6,
}
```

DB column: `stimulus.kind` → `stimulus.signal` (migration).

### Add: flow override on `Stimulus`

A stimulus can override the wave's default flow when it fires.

```rust
pub struct Stimulus {
    pub signal: Signal,           // was `kind: StimulusKind`
    pub flow: Option<String>,     // NEW — override wave.flow for this activation
    pub source_wave_id: Option<LfdId>,
    // ...existing fields
}
```

- `signal: Listen, flow: None` → fires on source completion, runs wave's default flow
- `signal: CiFailure, flow: Some("ci-fix")` → fires on CI failure, runs ci-fix flow
- `signal: Watch, flow: Some("lint")` → fires on file change, runs lint flow instead of default

### Drop: `WaveRunKind` and `CiFixKind`

These become unnecessary. A wave run is a wave run. How it was triggered is captured by `activation_log_id` → activation → stimulus → signal. The run itself doesn't need to know.

- Remove `WaveRunKind` enum
- Remove `CiFixKind` enum
- Remove `run_kind` and `ci_fix_kind` columns from `wave_runs` (migration)
- Remove `WaveRun.is_main()` — callers that need this check the activation source instead

### Fold: `ci_fix.rs` into normal activation path

`spawn_ci_fix_agent` currently:
1. Creates a worktree off the failing branch
2. Builds a `WaveRun` with `WaveRunKind::CiFix`
3. Runs the `ci-fix` step
4. Commits and pushes back to the PR branch

After this change:
1. CI failure webhook creates a `PendingActivation` for the wave's `CiFailure` stimulus
2. Normal activation drain picks it up
3. Run uses `stimulus.flow` override → resolves to `ci-fix` flow
4. The worktree-off-existing-branch + push-back behavior is part of the `ci-fix` flow/step definition, not the executor

The `ci-fix` step already exists as a regular step. The special worktree/push behavior moves into flow-level configuration or step-level hooks, not Rust type discrimination.

### CI webhook target scoping

Currently: "CI webhook updates apply to main runs only (exclude CI-fix runs)" — uses `run_kind == WaveRunKind::Main`.

After: scope by checking whether the run's activation source is `CiFailure`. Or simpler: scope by checking `activation_log_id` presence and the linked stimulus signal. The exact mechanism depends on how hot this path is.

## Migration

1. Add `signal` column to `stimulus` table, backfill from `kind`, drop `kind`
2. Add `flow` column to `stimulus` table (nullable text)
3. Drop `run_kind` and `ci_fix_kind` columns from `wave_runs`
4. Create `CiFailure` stimulus rows for waves that previously had CI fix enabled

## What changes in the diff

Files affected on the current branch:

| File | Change |
|------|--------|
| `types/wave.rs` | Delete `WaveRunKind`, `CiFixKind` enums |
| `types/stimulus.rs` | Rename `StimulusKind` → `Signal`, add `CiFailure` variant, add `flow: Option<String>` |
| `executor/wave/ci_fix.rs` | Delete — fold into normal activation path |
| `executor/wave/mod.rs` | Remove `WaveRunKind` branching |
| `store/catalog.rs` | Drop `run_kind`/`ci_fix_kind` from queries |
| `store/rows.rs` | Update row mapping |
| `store/sqlite.rs` / `postgres.rs` | Drop columns, rename `kind` → `signal` |
| `store/migrations/` | New migration for schema changes |
| `http/routes/waves.rs` | Remove `run_kind`/`ci_fix_kind` from DTOs |
| `http/routes/hooks.rs` | CI failure → create activation instead of direct spawn |
| `triggers/pending_activations.rs` | Handle `CiFailure` activations |
| `queue.rs` | Remove `ci_fix_kind` field |

## Git Sync Hardening

The signal simplification enables auxiliary runs (ci-fix, lint-fix, etc.) pushing to `origin/{wave-branch}`. This breaks the current assumption that each wave branch has one writer. Origin becomes the coordination point, not any local worktree.

### Current state (audit)

| Operation | File | Fetches first? | Problem |
|-----------|------|---------------|---------|
| CI fix push `HEAD:{branch}` | helpers.rs:201 | No | Fails if main run pushed since fix started |
| Advance branch | helpers.rs:454 | No | New branch based on stale local state |
| Auto-create PR | helpers.rs:386 | No | commit_workflow doesn't pull first |
| Ensure wave worktree (reuse) | helpers.rs:103 | No | Existing worktree could be behind origin |
| Background upstream sync | worktrees.rs:410 | No | Best-effort, silent failure |

No force pushes (good). No fetches before most pushes (bad for multi-writer).

### New model: push always, rebase aggressively

Origin is the source of truth. Every wave worktree is a mirror that flushes eagerly and pulls frequently.

**Pre-step** (before each step in a flow):
```
git fetch origin/{wave-branch}
git rebase origin/{wave-branch}
```
Incorporate anything pushed by auxiliary runs or previous iterations. If an auxiliary run fixed CI, the main wave picks it up before its next step.

**Post-step** (after each step completes):
```
git add -A && git commit
git push origin
```
Flush local work to origin immediately. Don't accumulate unpushed commits across steps.

### One conflict resolution strategy

Two failure modes, two appropriate responses:

- **Rebase conflict** → dedicated rebase agent (`ops/rebase.rs`, already exists). Conflicts need the rebase step's specific instructions about resolving markers, continuing the rebase, etc.
- **Failed push** (non-fast-forward) → `lf debug` with the push error output. It's just "this command failed, fix it." Debug handles that without a custom prompt.

`push_with_recovery`:

```
push
  → success? done
  → non-fast-forward? fetch + rebase_with_recovery + push retry
  → still fails? lf debug with the error output
```

**Pre-step** (before each step in a flow):

```rust
fn pre_step_sync(worktree, wave_branch, progress) -> OpsResult<()> {
    // 1. Incorporate auxiliary work (ci-fix, lint-fix, etc.)
    rebase_with_recovery(worktree, &RebaseOptions {
        onto: format!("origin/{wave_branch}"),
        push: false,
    }, progress)?;

    // 2. Incorporate upstream
    rebase_with_recovery(worktree, &RebaseOptions {
        onto: format!("origin/{}", get_default_branch(worktree)?),
        push: false,
    }, progress)?;

    Ok(())
}
```

Two sequential rebases. Step 1 is almost always clean (auxiliary runs work on the same feature). Step 2 is where conflicts are more likely (upstream main diverged). Rebase conflicts get the rebase agent.

**Post-step** (after each step completes):

```rust
fn post_step_sync(worktree, progress) -> OpsResult<()> {
    auto_commit_if_dirty(worktree)?;
    push_with_recovery(worktree, progress)?;  // try push, fetch+rebase+retry, debug on failure
    Ok(())
}
```

Flush local work to origin immediately. Don't accumulate unpushed commits across steps.

### What changes

The core change is in the step execution loop (`executor/wave/mod.rs`). Currently the loop is:

```
for step in flow:
    run_agent(step)
    maybe_auto_commit()
```

After:

```
for step in flow:
    pre_step_sync()         # rebase onto origin/wave-branch, then origin/main
    run_agent(step)
    post_step_sync()        # commit + push (with recovery on non-ff)
```

Specific files:

| File | Change |
|------|--------|
| `executor/wave/mod.rs` | Add pre/post step sync calls around each step |
| `executor/helpers.rs` | Add `pre_step_sync()`, `post_step_sync()` helpers; remove ci-fix-specific worktree/push code |
| `ops/rebase.rs` | No change needed — `rebase_with_recovery` already handles the pattern |
| `ops/commit.rs` | `push_with_upstream_or_error` → `push_with_recovery` (fetch+rebase+retry on non-ff) |
| `engine/worktrees.rs` | `ensure_wave_worktree` should fetch+rebase on reuse, not just schedule background upstream sync |

### Auxiliary runs

With this model, auxiliary runs (ci-fix, lint-fix, etc.) work identically to main runs:

1. Activation fires → creates a normal `WaveRun`
2. Run gets its own worktree (via `ensure_wave_worktree` or fresh creation)
3. Pre-step: fetch + rebase from `origin/{wave-branch}`
4. Run the flow (e.g., `ci-fix`)
5. Post-step: commit + push to `origin/{wave-branch}`
6. Main wave picks up changes on its next pre-step rebase

No special worktree-off-branch logic. No dedicated push-back code. The same fetch/rebase/push cycle handles everything.

### Worktree strategy for auxiliary runs

Auxiliary runs need their own worktree so they don't interfere with the main run. Two options:

**(a) Separate worktree, same remote branch.** Auxiliary run creates `repo.wave.ci-fix.{hash}/` worktree, checks out `origin/{wave-branch}`, works, pushes back to `origin/{wave-branch}`. Main run's next fetch+rebase picks it up.

**(b) Separate worktree, separate branch, merge to remote.** More isolation but more complexity. Not needed unless concurrent auxiliary runs become common.

Leaning (a). Simple, matches the "origin is truth" model.

## Lint in build

Add lint to the build flow as part of the polishing sequence, between compress and gate.

Current build flow:
```
implement → compress → gate → update-wave
```

After:
```
implement → compress → lint → gate → update-wave
```

Lint is polish, not a gate. If lint fails, it shouldn't block the release flow. The agent fixes what it can and moves on.

## Resolved questions

### `is_main()` callers

Three call sites, none need a run-level type discriminant:

| Caller | Real question | Replacement |
|--------|--------------|-------------|
| `hooks.rs:591` — CI webhook target | "Don't recurse ci-fix on ci-fix" | `snapshot.flow != "ci-fix"` |
| `wave/mod.rs:201` — Worktree janitor | "Is this worktree ephemeral?" | `is_ephemeral_worktree_path()` (already exists) |
| `pending_activations.rs:270` — Test | "Did a run get created?" | Check run exists + activation source is Listen |

### Concurrent auxiliary run push conflicts

Same pattern as everything else: `push_with_recovery`. Try push, if non-fast-forward then fetch + `rebase_with_recovery` + retry. The rebase agent handles conflicts if mechanical rebase fails. No special case for auxiliary vs main runs.

Additionally, the pending activation queue already coalesces — if a second CI failure arrives while the first ci-fix is running, it gets coalesced, not spawned as a duplicate.

### Rebase and push failure handling

Two failure modes, two responses:

- **Rebase conflict**: `rebase_with_recovery` (already exists in `ops/rebase.rs`). Dedicated rebase agent handles conflict markers, continues the rebase. This is the right tool — rebase resolution needs specific instructions.
- **Push failure**: `push_with_recovery`. Try push, if non-fast-forward then fetch + `rebase_with_recovery` + retry. If still fails, `lf debug` with the error output. Debug already knows how to look at an error and fix it — no custom prompt needed.

If both agents fail, the wave fails — but that's a genuine unresolvable conflict, not a system limitation.

## Open questions

None. All resolved above.
