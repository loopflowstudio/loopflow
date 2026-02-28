# `lf land` + worktree rotation

## Problem

Landing a PR and advancing to the next wave item is a multi-step manual process: enable auto-merge, rename the worktree, check for remaining wave items, create a new worktree. This should be one command — `lf land` — that runs at shell speed on the happy path and falls back to an agent only when something goes wrong.

This is the first consumer of `fast-path`, a step runner feature that lets any step declare a shell command to try before spinning up an agent. Exit 0 = done, no LLM. Non-zero = agent starts with failure output as context.

## Approach

Three deliverables, in dependency order:

### 1. `fast-path` step runner feature

Add `fast_path: Option<String>` to step frontmatter. Both runners (CLI `lf` and daemon `lfd`) check this field before launching an agent.

**Step struct** (`engine/flow.rs`):

```rust
pub struct Step {
    // existing fields...
    pub fast_path: Option<String>,
}
```

**StepFrontmatter** (`engine/flow.rs`):

```rust
struct StepFrontmatter {
    // existing fields...
    fast_path: Option<String>,
}
```

Parse `fast-path` (kebab-case in YAML, mapped to `fast_path` in Rust) in `parse_frontmatter_value()`.

**CLI runner** (`lf/commands/run.rs`):

In `run()`, after `build_prompt()` resolves the step, check `step.fast_path`. If present:

```rust
fn try_fast_path(cmd: &str, repo: &Path) -> Result<Option<FastPathResult>> {
    let output = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(repo)
        .env("LOOPFLOW_DIRECTIVE_FILE", /* propagate from parent */)
        .output()?;
    if output.status.success() {
        Ok(Some(FastPathResult::Success))
    } else {
        Ok(Some(FastPathResult::Failed {
            exit_code: output.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        }))
    }
}
```

If success → return early, skip agent. If failed → inject output into agent prompt as context, then launch agent normally.

**lfd runner** (`lfd/executor/wave/mod.rs`):

Handle fast-path in `execute()`, not inside `run_step()`, so it can skip `post_step_sync()`:

```rust
FlowAction::RunStep { step } => {
    pre_step_sync(...)?;

    if let Some(ref cmd) = step.fast_path {
        let result = try_fast_path(cmd, Path::new(&run.worktree))?;
        match result {
            FastPathResult::Success => {
                // Skip agent AND post_step_sync — the command handled everything.
                // post_step_sync would fail here anyway (branch merged, worktree renamed).
                self.advance_run_step(&mut run, &plan, wave.id()).await?;
                continue;
            }
            FastPathResult::Failed { stdout, stderr, .. } => {
                // Inject failure output into step content, then fall through to agent
            }
        }
    }

    let exit_code = self.run_step(&wave, &mut run, &step).await?;
    // ... existing post_step_sync + advance logic ...
}
```

This matters because `post_step_sync()` stages, commits, and pushes — which would fail after `lf ops land` (branch merged, worktree renamed).

### 2. Worktree rotation in `lf ops land`

After landing succeeds, add rotation. New function `rotate_worktree()` called at the end of `land()`:

```rust
pub fn land(repo: &Path, options: &LandOptions, progress: &impl Progress) -> OpsResult<LandResult> {
    let (repo_root, main_repo) = resolve_repos(repo, options.worktree.as_deref())?;
    // ... existing land logic ...

    let rotation = rotate_worktree(&repo_root, &main_repo, progress)?;
    Ok(LandResult { merged: true, rotation })
}
```

**Rotation logic:**

```rust
fn rotate_worktree(
    repo_root: &Path,
    main_repo: &Path,
    progress: &impl Progress,
) -> OpsResult<Option<RotationResult>> {
    let wave_name = match wave_name_from_worktree_and_main(repo_root, main_repo) {
        Some(name) => name,
        None => return Ok(None), // not in a worktree, or already full-path
    };

    // Only rotate shortname worktrees (no timestamp suffix)
    if wave_name.contains('.') {
        return Ok(None); // already a full-path/preserved worktree
    }

    // 1. Preserve: repo.mobile → repo.mobile.1740506522
    let preserved = preserve_worktree(main_repo, repo_root)?;
    progress.status(&format!("Preserved {} → {}", repo_root.display(), preserved.display()));

    // 2. Check wave/<shortname>/ for remaining items
    let wave_dir = main_repo.join("wave").join(&wave_name);
    let has_items = wave_dir.exists() && has_wave_items(&wave_dir)?;

    if has_items {
        // 3. Create new shortname worktree on fresh branch
        let new_path = worktree_path(main_repo, &wave_name);
        let branch = format_branch_name(main_repo, &wave_name)?;
        create_with_schema(main_repo, &wave_name, &branch)?;
        progress.status(&format!("Created new worktree at {}", new_path.display()));
        Ok(Some(RotationResult::Advanced { preserved, new_path }))
    } else {
        progress.status("Wave complete — no more items");
        Ok(Some(RotationResult::Complete { preserved }))
    }
}
```

**Shell directive** (CLI only, in `lf/commands/ops/mod.rs` land handler):

After `land()` returns, check the rotation result:
- `Advanced { new_path }` → `write_shell_directive(&format!("cd {}", new_path.display()))`
- `Complete { preserved }` → `write_shell_directive(&format!("cd {}", main_repo.display()))`
- `None` → no directive

**lfd handling** (in wave executor):

The daemon doesn't use `write_shell_directive`. After `land()` returns with rotation, the current wave run completes normally — land is the terminal step, the PR is merged, this item is done. The next wave item starts a fresh run, and `ensure_wave_worktree()` finds the newly-created shortname worktree at the same path. No worktree path update needed.

**`cleanup_run_worktree()` safety:** On `FlowAction::Complete`, the executor calls `cleanup_run_worktree()` with the path from `WaveRun.worktree`. After rotation, that path was renamed (e.g., `repo.mobile` → `repo.mobile.1740506522`). The cleanup must handle "path doesn't exist" gracefully — the worktree was intentionally preserved, not abandoned.

### 3. `lf land` step prompt

New builtin step at `rust/loopflow/src/engine/builtins/ops/land.md`:

```yaml
---
fast-path: lf ops land
---
```

The body guides the agent when fast-path fails:

```markdown
Land the current PR: rebase, lint, enable auto-merge, rotate worktree.

## API

lf ops land [--local] [--create-pr] [--no-lint]
lf ops wt move <worktree> <new-path>
lf ops wt create <name> [--base BRANCH]
lf ops wt list [--format json]

## Workflow

1. Read the error output from the failed fast-path attempt.
2. Diagnose the issue — common failures:
   - Merge conflicts: resolve them, then retry `lf ops land`
   - No PR: run `lf ops land --create-pr`
   - Lint failures: fix lint issues, then retry
   - CI failures: investigate and fix
3. After the underlying issue is resolved, run `lf ops land` to complete.
```

### 4. `lf ops wt prune` — shortname protection

In `list_worktrees()` (`engine/worktrees.rs`), add a check: if the wave name is a shortname (no dot/timestamp), force `prunable = false`.

```rust
// After computing prunable:
let is_shortname = wave_name_from_worktree_and_main(&wt_path, main_repo)
    .map(|name| !name.contains('.'))
    .unwrap_or(false);
if is_shortname {
    prunable = false;
}
```

This propagates through `wt_prune()` automatically since prune already filters on `prunable`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Hook system instead of `fast-path` | More general — pre/post hooks per step | Over-engineered. `fast-path` is the specific pattern we need: try command, fall back to agent. Hooks would need event types, ordering, error handling. |
| Rotation as separate `lf ops rotate` command | Explicit, composable | Wrong granularity. Users think "land" not "land then rotate." Rotation is part of landing in a wave context. |
| Rotation waits for PR to actually merge | Correct ordering | PR merge is async (CI must pass first). User shouldn't wait. Rotation happens when *intent to merge* is committed. |
| Shortname detection via wave directory existence | Semantically correct | Too expensive — requires checking filesystem. Dot heuristic (`name.contains('.')`) is fast and reliable since `preserve_worktree` always adds `.{unix_ts}`. |

## Key decisions

**`fast-path` runs the command with `sh -c`, not as a parsed command.** This lets step authors write `lf ops land --no-lint` without us parsing flags. The command inherits the repo as cwd and `LOOPFLOW_DIRECTIVE_FILE` from the parent environment.

**Rotation happens regardless of `--local` vs remote.** Both paths end with "this branch's work is done." The only difference is whether merge happens now (local) or later (auto-merge).

**Shortname detection uses the dot heuristic.** `preserve_worktree()` always produces `{name}.{unix_timestamp}`, so a wave name containing a dot is always a preserved worktree. No filesystem lookups needed.

**Failed fast-path output goes into the agent prompt, not a file.** The agent needs to see what went wrong immediately. Injected as a `<lf:fast-path-failure>` tag at the top of the step body.

**`has_wave_items()` counts markdown files, ignoring README.md and YAML config.** A wave directory with only `README.md` and `wave.yaml` is "empty" — no items to advance to.

## Scope

- In scope: `fast-path` in step frontmatter + both runners, worktree rotation in `lf ops land`, `lf land` step prompt, shortname protection in prune
- Out of scope: `lfd` wave scheduling changes (daemon already handles step advancement), Concerto UI for rotation, `lf rebase` step (sprint 04)

## Done when

```bash
# fast-path works
lf land                    # exits 0 → no agent spun up, PR landed + worktree rotated
lf land                    # exits non-zero → agent starts with error context

# Rotation works
lf ops land                # in repo.mobile → lands PR, renames to repo.mobile.1740506522
                           # if wave/mobile/ has items → creates new repo.mobile on fresh branch
                           # shell cds to new repo.mobile (or main repo if wave complete)

# Prune respects shortnames
lf ops wt prune --dry-run  # repo.mobile never listed as prunable
                           # repo.mobile.1740506522 listed if branch merged

# cargo test --all passes
# cargo clippy -- -D warnings clean
```

Goals advanced from wave README:
- "`lf land` lands the PR, rotates the shortname worktree, advances to next wave item — fast-path, no agent"
- "`fast-path` as a general step feature — any step can declare a fast command that skips the agent on success"
