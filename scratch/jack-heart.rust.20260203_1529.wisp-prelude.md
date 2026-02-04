# lf ops: Rust-Native Workflow Engine

## Problem

Rust `lf ops` commands exist but lack the polish that makes Python's workflow feel seamless. The gaps aren't features—they're the connective tissue: auto-staging before commit, agent-generated messages, conflict resolution via handoff to `lf rebase`, lint checks that invoke fixers on failure.

Python `lf ops` orchestrates git, GitHub CLI, and agents into a single coherent workflow. Rust `lf ops` runs individual git commands. Users feel the difference.

**Who benefits:** Anyone using `lf ops` for daily git workflow. The gap forces users back to Python or manual workarounds.

**Why now:** lfd is the primary execution path. `loopflow-engine` is feature-complete. The CLI is the last piece blocking binary distribution and the Python→Rust migration.

## Approach

Build a **workflow engine** inside `loopflow-engine`, not scattered command-line logic. Each operation (`commit`, `pr`, `land`, `next`, `abandon`) composes from primitives:

```
┌─────────────────────────────────────────────────────────┐
│  loopflow-engine::workflow                              │
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │ Primitives                                       │   │
│  │  stage_all()  commit()  push()  rebase()        │   │
│  │  pr_create()  pr_ready()  pr_merge_auto()       │   │
│  │  clear_scratch()  run_lint()  run_agent()       │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │ Workflows (compose primitives)                  │   │
│  │  add_commit_push()                               │   │
│  │  rebase_with_conflict_recovery()                │   │
│  │  land_with_auto_merge()                         │   │
│  │  iterate_to_next_branch()                       │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │ Agent Integration                                │   │
│  │  generate_commit_message()                       │   │
│  │  generate_pr_description()                       │   │
│  │  resolve_rebase_conflicts()                      │   │
│  │  fix_lint_issues()                               │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

The CLI becomes thin: parse args, call workflow, print output.

### Key Design Decisions

**1. Workflows are library code, not CLI code.**

Today's Rust CLI has `rebase_current()`, `push_current()`, `land_current()` as standalone functions. This scatters logic and prevents composition.

Move workflows into `loopflow-engine::workflow`. The CLI calls `workflow::land(repo, config)`. This enables:
- lfd to use the same workflows
- Testing without spawning processes
- Consistent behavior across entry points

**2. Agent integration via step execution.**

Python calls `generate_commit_message()` which internally runs an agent with a step. Rust should do the same.

```rust
// loopflow-engine/src/workflow/agent.rs

pub fn run_step_for_message(repo: &Path, step: &str) -> Result<String> {
    let step = load_step(repo, step)?;
    let config = load_config_or_default(Some(repo));
    let launch = LaunchConfig {
        auto: true,
        stream: false,  // capture output
        skip_permissions: config.yolo,
        ..Default::default()
    };
    let result = launch_agent_sync(&config.agent_model, &step.prompt, &launch)?;
    Ok(result.output)
}
```

**3. Conflict recovery via agent handoff.**

When `rebase` hits conflicts, Python aborts the rebase and launches `lf rebase` step. The agent resolves conflicts, completes the rebase, and control returns.

```rust
pub fn rebase_with_recovery(repo: &Path, onto: &str) -> Result<RebaseResult> {
    let result = git::rebase(repo, onto, None)?;
    if result.success {
        return Ok(result);
    }

    // Abort and hand off to agent
    git::rebase_abort(repo)?;

    let step = load_step(repo, "rebase")?;
    let prompt = format_rebase_prompt(&step, &result.conflicts);

    launch_agent_interactive(repo, &prompt)?;

    // Verify rebase completed
    if !git::rebase_in_progress(repo)? {
        Ok(RebaseResult { success: true, conflicts: None, new_head: Some(git::head(repo)?) })
    } else {
        Err(anyhow!("rebase still in progress after agent"))
    }
}
```

**4. Lint runs check first, agent on failure.**

```rust
pub fn ensure_lint_passes(repo: &Path) -> Result<bool> {
    let config = load_config_or_default(Some(repo));

    // Try configured command
    if let Some(cmd) = &config.lint_check {
        if run_command(repo, cmd)?.success() {
            return Ok(true);
        }
    } else {
        // Auto-detect ruff
        if which("ruff").is_some() {
            if run_ruff_check(repo)? {
                return Ok(true);
            }
        }
    }

    // Lint failed - run fixer agent
    launch_agent_sync(repo, "lint")?;

    // Verify fixed
    if let Some(cmd) = &config.lint_check {
        Ok(run_command(repo, cmd)?.success())
    } else {
        run_ruff_check(repo)
    }
}
```

**5. Progress messages via callback.**

```rust
pub trait Progress {
    fn status(&self, msg: &str);
    fn error(&self, msg: &str);
    fn confirm(&self, msg: &str) -> bool;
}

pub fn land(repo: &Path, config: &LandConfig, progress: &impl Progress) -> Result<LandResult> {
    progress.status("Checking for pending changes...");
    if !config.strict && add_commit_push(repo, progress)? {
        progress.status("Committed pending changes");
    }

    progress.status("Rebasing onto main...");
    rebase_with_recovery(repo, &format!("origin/{}", config.main_branch))?;

    progress.status("Enabling auto-merge...");
    gh::pr_merge_auto(repo)?;

    Ok(LandResult { merged: true })
}
```

## Alternatives Considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep workflows in CLI, share via crate | Simpler structure | Forces lfd to duplicate logic or shell out |
| Shell out to Python for workflows | Zero rewrite | Defeats purpose of Rust migration |
| Minimal Rust CLI, Python remains primary | Less work | Blocks binary distribution goal |
| Async workflows with tokio | Modern Rust | Overkill for sequential git operations |

## Key Decisions

**Stateless workflows.** Each workflow function takes a repo path and config, does work, returns result. No workflow state stored in engine. The CLI/lfd owns state.

> From wave principles: "loopflow-engine is a stateless library of pure functions"

**Sync agent execution for workflows.** Commit message generation and conflict resolution are synchronous operations in user flow. Use `launch_agent_sync()` that blocks until agent completes. Reserve async for lfd's wave execution.

**Config drives behavior.** `load_config_or_default()` returns config with sensible defaults. No hardcoded paths or tool assumptions. If user sets `lint_check`, use it. Otherwise auto-detect.

**Composable primitives.** `add_commit_push()` is used by `pr`, `land`, and `next`. `rebase_with_recovery()` is used by `rebase`, `land`, and `next`. One implementation, tested once.

## Scope

### In scope

- Workflow engine in `loopflow-engine::workflow`
- Agent integration for messages, conflicts, lint
- All `lf ops` commands reaching Python parity
- Progress callback for UX
- Confirmation prompts for destructive operations

### Out of scope

- Fish shell support (deferred)
- Wave metadata updates (deferred until wave module ported)
- `lf ops doctor` (low priority)
- Windows support (separate work item)

## Done When

```bash
# Core workflow commands work like Python
lf ops commit                    # auto-stage, lint, agent message
lf ops commit -p                 # + push + draft PR
lf ops pr                        # auto-commit, rebase if behind, agent description
lf ops land                      # auto-commit, rebase, clear scratch, auto-merge
lf ops next                      # preserve worktree, auto-merge current, create new
lf ops abandon my-branch         # close PR, delete remote, remove worktree

# Conflict recovery works
lf ops rebase                    # on conflict: abort, launch agent, complete

# Progress messages visible
lf ops land
# Output:
# Checking for pending changes...
# Committed pending changes
# Rebasing onto origin/main...
# Clearing scratch/...
# Enabling auto-merge...

# Tests pass
cargo test -p loopflow-engine workflow
```

Observable: A user can complete a full feature cycle (create branch → work → commit → pr → land) using only Rust `lf ops`, with the same experience as Python.
