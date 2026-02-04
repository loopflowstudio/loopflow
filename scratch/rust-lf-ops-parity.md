# lf ops: Rust-First Git Workflow

## Problem

The Python `lf ops` CLI has rich automation: auto-commit, auto-rebase, agent-assisted conflict resolution, LLM-generated messages, lint integration. The Rust implementation exists but lacks this automation. Users reaching for `lf ops commit`, `lf ops pr`, or `lf ops land` get a fraction of the capability.

**Who benefits:** Anyone using `lf` for git workflows. The gap blocks the Phase 1 goal of making `lf` (Rust) the primary CLI.

**Why now:** Phase 2 requires `lfd` to call git operations. Having one authoritative implementation (Rust) prevents drift between daemon and CLI.

## Approach

Replace Python `lf ops` with Rust entirely. Delete `lf-engine` binary. Expose `loopflow-engine` to Python via PyO3 for any remaining Python callers.

Three priorities:
1. **Agent integration** - Rust can launch agents for commit messages, PR descriptions, conflict resolution
2. **Auto-commit workflow** - Stage, generate message, commit, push, ensure draft PR
3. **Lint integration** - Check before commit/PR/land, launch fixer agent on failure

The workflow commands (`commit`, `pr`, `land`, `next`) are interconnected. Fix all four together rather than incrementally.

## Alternatives Considered

| Approach | Tradeoff | Why Not |
|----------|----------|---------|
| Keep Python ops, call from Rust | Two implementations, maintenance burden | Goal is single source of truth |
| Port incrementally, maintain parity | Longer timeline, feature drift | Users get inconsistent behavior |
| Just expose Python to Rust via subprocess | Slow, awkward, circular deps | Defeats purpose of Rust-first |

## Key Decisions

**1. Agent invocation from Rust**

The roadmap says: "protocol first... lf uses loopflow-engine directly."

Rust `lf ops commit` without `-m` will:
1. Build prompt via `loopflow_engine::prompt::gather_context()`
2. Launch agent via `loopflow_engine::agent::launch_agent()`
3. Parse agent output for commit message
4. Commit with that message

This is the same pattern Python uses, but without subprocess boundary.

**2. Auto-commit is opt-out, not opt-in**

The roadmap says: "start with minimal data structures and APIs."

Current Rust: commit requires `-m`. Python: auto-generates message.

Decision: Rust matches Python. `-m` is optional. If omitted, agent generates message. Add `--no-add` and `--no-lint` as opt-outs.

**3. Conflict resolution launches rebase agent**

When `lf ops rebase` or auto-rebase hits conflicts:
1. Abort rebase (leave files in conflict state)
2. Launch `lf rebase` step with conflict context
3. Agent resolves, retries rebase
4. Verify branch is ahead of base

This matches Python's pattern and leverages existing `rebase` step infrastructure.

**4. Delete lf-engine binary**

The gap analysis notes: "Python calls `lf-engine` binary for git operations."

Instead: Python imports `loopflow_engine` via PyO3. The `python` feature is already defined in Cargo.toml. No intermediate binary.

## Scope

### In scope

**Core workflow commands:**
- `commit` - auto-stage, lint, agent message, push, draft PR
- `pr` - auto-commit, rebase, LLM title/body, draft→ready, browser open
- `land` - auto-commit, rebase, lint, clear scratch/, auto-merge, worktree cleanup
- `rebase` - conflict detection, agent handoff
- `next` - auto-commit, auto-merge current PR, create stacked branch
- `abandon` - close PR, delete remote, remove worktree

**Supporting infrastructure:**
- Agent launch from Rust (already exists: `loopflow_engine::agent`)
- Lint check and agent fixer
- Auto-commit workflow (stage, message, commit, push, draft PR)
- Progress messages ("Fetching...", "Rebasing...", etc.)

**Architecture cleanup:**
- Delete `lf-engine` binary
- Python uses PyO3 bindings exclusively

### Out of scope

- Wave integration (deferred to Phase 2 - depends on lfd primary)
- Fish shell support (deferred - limited user base)
- `lf ops doctor` command (useful but not blocking)
- Container/K8s executors (Phase 2)

## Implementation

### Phase 1: Agent + Auto-commit (1 week)

Add to `loopflow_engine`:

```rust
// loopflow_engine/src/ops.rs (new module)

/// Auto-commit workflow: stage, generate message, commit, push, draft PR.
pub fn add_commit_push(repo: &Path, push: bool) -> Result<bool> {
    if is_clean(repo)? {
        if push {
            push_branch(repo)?;
            ensure_draft_pr(repo)?;
        }
        return Ok(false);
    }

    stage_all(repo)?;
    let message = generate_commit_message(repo)?; // agent invocation
    commit(repo, &message)?;

    if push {
        push_branch(repo)?;
        ensure_draft_pr(repo)?;
    }
    Ok(true)
}

/// Lint check with agent fixer fallback.
pub fn run_lint(repo: &Path) -> Result<bool> {
    match check_lint(repo)? {
        LintResult::Pass => Ok(true),
        LintResult::Fail | LintResult::Unknown => {
            launch_lint_fixer_agent(repo)?;
            Ok(check_lint(repo)? == LintResult::Pass)
        }
    }
}

/// Generate commit message via agent.
fn generate_commit_message(repo: &Path) -> Result<String> {
    let components = gather_context(&GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("commit".to_string()),
        ..Default::default()
    })?;
    let prompt = format_prompt(&components);
    let result = launch_agent("claude:opus", &prompt, &LaunchConfig::auto())?;
    parse_commit_message(&result.stdout)
}
```

Update `lf ops commit`:

```rust
fn commit_current(message: Option<&str>, add: bool, lint: bool, push: bool) -> Result<()> {
    let repo = find_repo_root()?;

    if add {
        stage_all(&repo)?;
    }

    if lint && !run_lint(&repo)? {
        return Err(anyhow!("lint failed"));
    }

    let msg = match message {
        Some(m) => m.to_string(),
        None => generate_commit_message(&repo)?,
    };

    commit(&repo, &msg)?;

    if push {
        push_branch(&repo)?;
        ensure_draft_pr(&repo)?;
    }
    Ok(())
}
```

### Phase 2: PR and Land (1 week)

Update `lf ops pr`:

```rust
fn create_or_update_pr(refresh: bool, lint: bool) -> Result<()> {
    let repo = find_repo_root()?;

    if lint && !run_lint(&repo)? {
        return Err(anyhow!("lint failed"));
    }

    sync_main(&repo, &get_default_branch(&repo)?)?;
    add_commit_push(&repo, true)?;

    if is_behind_main(&repo)? {
        auto_rebase(&repo)?;
    }

    let existing = get_pr_url(&repo)?;
    if let Some(url) = existing {
        if !refresh && !has_unpushed_commits(&repo)? && !is_draft_pr(&repo)? {
            open_browser(&url)?;
            return Ok(());
        }
        let message = generate_pr_message(&repo)?;
        update_pr(&repo, &message)?;
        if is_draft_pr(&repo)? {
            mark_pr_ready(&repo)?;
        }
    } else {
        let message = generate_pr_message(&repo)?;
        let base = detect_stacked_base(&repo)?;
        create_pr(&repo, &message, &base)?;
    }

    open_browser(&get_pr_url(&repo)?.unwrap())?;
    Ok(())
}
```

Update `lf ops land`:

```rust
fn land_current(strict: bool, worktree: Option<&str>, create_pr: bool, lint: bool) -> Result<()> {
    let (repo, main_repo) = resolve_repos(worktree, strict)?;

    if lint && !run_lint(&repo)? {
        return Err(anyhow!("lint failed"));
    }

    if !strict {
        add_commit_push(&repo, true)?;
    }

    if !rebase_onto_main(&repo)? {  // launches agent on conflict
        return Err(anyhow!("rebase failed"));
    }

    clear_scratch(&repo)?;  // delete scratch/*, commit, push

    if is_draft_pr(&repo)? {
        mark_pr_ready(&repo)?;
    }

    let message = generate_pr_message(&repo)?;
    update_pr(&repo, &message)?;
    enable_auto_merge(&repo, &message)?;

    open_browser(&get_pr_url(&repo)?)?;
    Ok(())
}
```

### Phase 3: Rebase + Next + Abandon (3 days)

Update `lf ops rebase`:

```rust
fn rebase_current(onto: Option<&str>) -> Result<()> {
    let repo = find_repo_root()?;
    let onto_ref = onto.unwrap_or(&format!("origin/{}", get_default_branch(&repo)?));

    let result = rebase(&repo, onto_ref, None)?;
    if !result.success {
        // Rebase aborted, conflicts in working tree
        println!("Conflicts detected, launching rebase assistant...");
        launch_rebase_agent(&repo, &result.conflicts)?;

        // Verify rebase completed
        if !is_ahead_of(&repo, onto_ref)? {
            return Err(anyhow!("rebase did not complete"));
        }
    }

    // Force-push after rebase
    if has_upstream(&repo)? {
        push(&repo, true)?;
    }
    Ok(())
}
```

Update `lf ops next`:

```rust
fn next_branch(block: bool, create_pr: bool, rebase: bool) -> Result<()> {
    let repo = find_repo_root()?;

    add_commit_push(&repo, true)?;

    if rebase && !rebase_onto_main(&repo)? {
        return Err(anyhow!("rebase failed"));
    }

    if let Some(pr_number) = get_pr_number(&repo)? {
        enable_auto_merge(&repo)?;
        if block {
            wait_for_merge(&repo, pr_number)?;
        }
    } else if create_pr {
        create_or_update_pr(false, false)?;
        enable_auto_merge(&repo)?;
    }

    let new_branch = generate_branch_name(&repo)?;
    create_branch(&repo, &new_branch)?;
    push_with_upstream(&repo, &new_branch)?;

    // Update wave metadata when wave module exists
    // update_wave_branch(&repo, &new_branch)?;

    Ok(())
}
```

Update `lf ops abandon`:

```rust
fn abandon_current(force: bool, branch: Option<&str>) -> Result<()> {
    let repo = find_repo_root()?;
    let main_repo = main_repo_root(&repo)?;

    let target_branch = branch.or_else(|| current_branch(&repo).ok().flatten())
        .ok_or_else(|| anyhow!("no branch specified"))?;

    let worktree = find_worktree_by_branch(&main_repo, &target_branch)?;

    if !force && !is_clean(&worktree)? {
        return Err(anyhow!("uncommitted changes; use --force"));
    }

    // Close PR if exists
    if let Ok(Some(_)) = get_pr_number(&worktree) {
        close_pr(&worktree)?;
    }

    // Delete remote branch
    let _ = delete_remote_branch(&main_repo, &target_branch);

    // Remove worktree
    worktree_remove(&main_repo, &worktree)?;

    // Delete local branch
    delete_local_branch(&main_repo, &target_branch)?;

    Ok(())
}
```

### Phase 4: Delete lf-engine (2 days)

1. Verify PyO3 bindings in `loopflow_engine/src/python.rs` expose all needed functions
2. Update Python `loopflow.lf.ops.git` to use PyO3 imports exclusively
3. Delete `rust/loopflow-engine/src/bin/lf-engine.rs`
4. Update `Cargo.toml` to remove the binary target

## Done When

```bash
# These commands work identically to Python:
lf ops commit                    # auto-stage, agent message, commit
lf ops commit -p                 # + push + draft PR
lf ops commit -m "msg"           # explicit message
lf ops commit --no-lint          # skip lint check

lf ops pr                        # auto-commit, rebase, LLM message, open browser
lf ops pr --refresh              # regenerate title/body
lf ops pr --no-lint              # skip lint

lf ops land                      # auto-commit, rebase, clear scratch/, auto-merge
lf ops land --strict             # fail on uncommitted changes
lf ops land --local              # local merge (no PR)
lf ops land -c                   # create PR and land
lf ops land -w feature           # target specific worktree

lf ops rebase                    # rebase, agent on conflict
lf ops next                      # auto-commit, auto-merge, new branch
lf ops next --block              # wait for merge
lf ops next --create-pr          # create PR first

lf ops abandon                   # close PR, delete remote, remove worktree
lf ops abandon feature           # target by branch name
lf ops abandon --force           # skip dirty check

# Python callers use PyO3:
from loopflow_engine import rebase, commit, push
```

Verification:
- `cargo test` passes for all new ops functions
- Golden flow set runs with Rust `lf`
- `lf-engine` binary deleted from repo
- Python tests pass with PyO3 backend
