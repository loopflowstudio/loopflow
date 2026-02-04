# lf ops: Rust-Native Workflow Engine

## Problem

Rust `lf ops` commands exist but lack the polish that makes Python's workflow feel seamless. The gaps aren't features—they're the connective tissue: auto-staging before commit, agent-generated messages, conflict resolution via handoff to `lf rebase`, lint checks that invoke fixers on failure.

Python `lf ops` orchestrates git, GitHub CLI, and agents into a single coherent workflow. Rust `lf ops` runs individual git commands. Users feel the difference.

**Who benefits:** Anyone using `lf ops` for daily git workflow. The gap forces users back to Python or manual workarounds.

**Why now:** lfd is the primary execution path. `loopflow-engine` is feature-complete. The CLI is the last piece blocking binary distribution and the Python→Rust migration.

## Approach

Build a **separate `loopflow-ops` crate** for workflow orchestration. This crate depends on `loopflow-engine` for primitives (git, agent, config) and composes them into complete workflows.

```
rust/
├── loopflow-engine/     # Primitives (git, agent, flow, prompt)
├── loopflow-ops/        # Workflow orchestration (NEW)
│   └── src/
│       ├── lib.rs
│       ├── messages.rs  # generate_commit_message(), generate_pr_message()
│       ├── commit.rs    # commit()
│       ├── pr.rs        # create_or_update_pr()
│       ├── land.rs      # land()
│       ├── rebase.rs    # rebase_with_recovery()
│       ├── next.rs      # next_branch()
│       ├── abandon.rs   # abandon_branch()
│       ├── lint.rs      # ensure_lint_passes()
│       └── progress.rs  # Progress trait
├── lf/                  # CLI (thin wrapper, calls loopflow-ops)
└── lfd/                 # Daemon (can also use loopflow-ops)
```

The CLI becomes thin: parse args, call `loopflow_ops::land()`, print output.

### Key Design Decisions

**1. Separate crate, not module.**

`loopflow-ops` is a standalone crate that depends on `loopflow-engine`. This provides:
- Clear dependency direction (ops → engine, not engine → ops)
- Cleaner separation of concerns (engine = primitives, ops = orchestration)
- lfd can use loopflow-ops without circular dependencies
- Testing without spawning processes

**2. lf-driven, not agent-driven.**

Agent generates messages; lf does git operations. This gives lf full control over orchestration.

```rust
// loopflow-ops/src/messages.rs

pub fn generate_commit_message(repo: &Path) -> Result<CommitMessage> {
    let diff = get_staged_diff(repo)?;
    let prompt = build_message_prompt(&diff, COMMIT_MESSAGE_TEMPLATE);

    // Run agent in batch mode, capture JSON output
    let output = launch_agent_batch(repo, &prompt)?;

    // Parse {title, body} from output
    parse_commit_message(&output)
}
```

The agent returns structured data (JSON `{title, body}`), lf runs `git commit`.

**3. Batched agent calls.**

Pre-check what needs to be done, then make 0 or 1 agent call per operation:

```rust
pub fn land(repo: &Path, config: &LandConfig, progress: &impl Progress) -> Result<LandResult> {
    // Pre-check phase (no agent)
    let needs_commit = !is_clean(repo)?;
    let rebase_result = try_rebase(repo, &config.main_branch)?;
    let needs_conflict_resolution = !rebase_result.success;

    // Agent phase (0 or 1 call)
    if needs_commit || needs_conflict_resolution {
        let mut tasks = vec![];
        if needs_commit { tasks.push("commit staged changes"); }
        if needs_conflict_resolution { tasks.push("resolve rebase conflicts"); }
        launch_agent_for_tasks(repo, &tasks)?;
    }

    // Mechanical phase (no agent)
    clear_scratch(repo)?;  // Simple commit: "lf land: clear scratch/"
    push(repo)?;
    pr_ready(repo)?;
    pr_merge_auto(repo)?;

    Ok(LandResult { merged: true })
}
```

**4. Commit message format with flow lineage.**

All commits include the flow hierarchy for traceability:

```
lf {flow_parents} {task}: {generated_title}

{generated_body}
```

Examples:
- `lf commit: add dark mode toggle` (direct ops command)
- `lf my-wave ship implement: refactor auth module` (step in flow in wave)

```rust
pub fn commit(
    repo: &Path,
    task: &str,
    flow_parents: &[String],
    progress: &impl Progress,
) -> Result<bool> {
    let generated = generate_commit_message(repo)?;

    let prefix = if flow_parents.is_empty() {
        format!("lf {task}")
    } else {
        format!("lf {} {task}", flow_parents.join(" "))
    };

    let message = format!("{prefix}: {}", generated.title);
    git::commit(repo, &message)?;
    Ok(true)
}
```

**5. Mechanical ops don't need agent.**

These operations are fully programmatic—no agent involvement:
- `clear_scratch()` → delete files + commit "lf land: clear scratch/"
- `push()` → `git push`
- `pr_ready()` → `gh pr ready`
- `pr_merge_auto()` → `gh pr merge --auto --squash`

Agent is only consulted for:
- Commit message generation
- PR description generation
- Conflict resolution
- Lint fixing (if lint fails)

**6. Progress messages via callback.**

```rust
pub trait Progress {
    fn status(&self, msg: &str);
    fn error(&self, msg: &str);
    fn confirm(&self, msg: &str) -> bool;
}
```

CLI implementation prompts stdin. lfd implementation uses config flags or fails.

## Alternatives Considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep workflows in CLI, share via crate | Simpler structure | Forces lfd to duplicate logic or shell out |
| Shell out to Python for workflows | Zero rewrite | Defeats purpose of Rust migration |
| Minimal Rust CLI, Python remains primary | Less work | Blocks binary distribution goal |
| Async workflows with tokio | Modern Rust | Overkill for sequential git operations |

## Key Decisions

**Stateless workflows.** Each workflow function takes a repo path, config, and flow context. Returns result. No workflow state stored in the crate. The CLI/lfd owns execution state.

**Sync agent execution.** Commit message generation and conflict resolution are synchronous operations in user flow. Use batch mode (`--print`) that blocks until agent completes. Reserve async for lfd's wave execution.

**Config drives behavior.** `load_config_or_default()` returns config with sensible defaults. No hardcoded paths or tool assumptions. If user sets `lint_check`, use it. Otherwise auto-detect.

**Composable primitives.** `commit()` is used by `pr`, `land`, and `next`. `rebase_with_recovery()` is used by `rebase`, `land`, and `next`. One implementation, tested once.

**Commit messages from CLAUDE.md.** Commit message style is defined in repo's CLAUDE.md, which is included in agent context. No separate "commit" step needed—agent already knows the conventions.

## Scope

### In scope

- `loopflow-ops` crate with workflow orchestration
- Message generation (commit, PR) via agent batch mode
- All `lf ops` commands reaching Python parity
- Progress callback for UX
- Confirmation prompts for destructive operations
- Flow lineage in commit messages (`lf {flow_parents} {task}: ...`)

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

# Commit messages have correct format
git log --oneline
# lf land: clear scratch/
# lf my-wave ship implement: add auth middleware

# Tests pass
cargo test -p loopflow-ops
```

Observable: A user can complete a full feature cycle (create branch → work → commit → pr → land) using only Rust `lf ops`, with the same experience as Python.
