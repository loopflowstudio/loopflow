# Git Sync Hardening

## Problem

Wave execution breaks when concurrent runs push to the same remote branch. Phase 03.5 shipped the happy path — fetch+rebase before steps, commit+push after — but three failure modes remain unrecovered:

1. **Pre-step only rebases onto the wave branch.** Upstream changes on `main` aren't picked up until the worktree is recreated, so wave branches drift from main across long runs.
2. **Rebase conflicts abort the step silently.** The executor logs a warning and continues on stale state, meaning the agent works on code that won't merge.
3. **Push failures hard-fail after one retry.** A transient race between concurrent runs kills the entire run instead of recovering.

These matter now because listen stimulus (chords) enables fan-out: one source wave completing can trigger N downstream waves pushing to the same branch simultaneously.

**Advancing chords goal:** "Listen stimulus fires reliably when source wave completes" — reliability requires that the git layer doesn't hard-fail under concurrent writes that listen fan-out produces.

**Chords risk:** "Listen fan-out — many waves listening to one source triggers N runs simultaneously. No concurrency limiting today." This design doesn't add concurrency limiting but makes the git layer tolerate the concurrency that already exists.

## Approach

Five changes. Four in `helpers.rs` (executor sync functions), plus one new module: `ops/agent.rs` with a shared `run_builtin_agent` helper that both rebase recovery and push escalation use.

### 1. Dual rebase in `pre_step_sync`

Unify `pre_step_sync` with `sync_existing_worktree`'s behavior. After rebasing onto `origin/{branch}`, also rebase onto `origin/{default_branch}`.

`sync_existing_worktree` already does this for worktree creation/reuse. The gap is that `pre_step_sync` (called at every step boundary) skips the upstream rebase. Fix: extract a shared `dual_rebase` helper and call it from both sites.

`pre_step_sync` needs the main repo path to call `get_default_branch`. Currently it only receives `worktree`. Add `main_repo: &Path` parameter to `pre_step_sync` and thread it through from the executor, which already has `wave.repo`.

`sync_existing_worktree` becomes a thin wrapper around `dual_rebase`.

### 2. Rebase conflict recovery via `rebase_with_recovery`

Replace bare `rebase()` calls in sync functions with `rebase_with_recovery()` from `ops/rebase.rs`. This routes conflicts through the rebase agent — an LLM session that resolves conflicts, continues the rebase, and pushes.

The rebase agent path already works for `lf ops rebase`. The change is calling it from the executor instead of aborting the step.

Three call sites use bare `rebase()`:

1. `pre_step_sync` — single rebase onto wave branch
2. `sync_existing_worktree` — dual rebase (wave branch + default branch)
3. `post_step_sync` retry path — rebase after failed push

All three should use `rebase_with_recovery`.

**`Progress` trait adapter.** `rebase_with_recovery` takes `&impl Progress` (3 methods: `status`, `error`, `confirm`). `NullProgress` exists but drops all messages silently. For executor sync where visibility matters, a `TracingProgress` that maps to tracing macros is better:

```rust
struct TracingProgress;

impl Progress for TracingProgress {
    fn status(&self, msg: &str) { info!("{}", msg); }
    fn error(&self, msg: &str) { error!("{}", msg); }
    fn confirm(&self, _msg: &str) -> bool { true }  // auto-confirm in headless executor
}
```

**`dual_rebase` helper:**

```rust
fn dual_rebase(repo: &Path, worktree: &Path, branch: &str) -> Result<()> {
    let progress = TracingProgress;

    // 1. Rebase onto origin/{branch}
    let opts = RebaseOptions { onto: format!("origin/{branch}"), push: false };
    rebase_with_recovery(worktree, &opts, &progress)?;

    // 2. Rebase onto origin/{default_branch}
    let default_branch = get_default_branch(repo)?;
    let opts = RebaseOptions { onto: format!("origin/{default_branch}"), push: false };
    rebase_with_recovery(worktree, &opts, &progress)?;

    Ok(())
}
```

`rebase_with_recovery` handles fetch internally when the target starts with `origin/`, so `dual_rebase` doesn't need explicit fetch calls.

**Error type conversion.** `rebase_with_recovery` returns `OpsResult<RebaseResult>`. Sync functions return `anyhow::Result<()>`. `OpsError` derives `thiserror::Error`, so the `?` operator converts automatically. The `RebaseResult.success` field is always `true` when `Ok` (agent path assumes success or returns `Err`), so there's no need to check it — just propagate the `Result`.

### 3. Shared `run_builtin_agent` helper (`ops/agent.rs`)

Extract the "load builtin step → gather context → format prompt → launch headlessly" pattern from `run_rebase_agent` into a public helper. Both rebase recovery and push escalation use it.

```rust
// ops/agent.rs

pub struct BuiltinAgentOptions {
    pub step_name: String,       // key for get_builtin_step (e.g. "rebase", "debug")
    pub suffix: String,          // appended after the step content in the prompt
    pub timeout: Option<Duration>,
}

/// Launch a builtin step as a headless agent session.
pub fn run_builtin_agent(
    repo: &Path,
    options: &BuiltinAgentOptions,
    progress: &impl Progress,
) -> OpsResult<()> {
    let config = load_config_or_default(Some(repo));
    let step_content = get_builtin_step(&options.step_name)
        .ok_or_else(|| OpsError::AgentFailed(
            format!("built-in step '{}' not found", options.step_name)
        ))?;

    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: None,
        message: None,
        surface: Surface::Headless,
        directions: config.direction.unwrap_or_default(),
        files: Vec::new(),
        sources: default_gather_sources(config.lfdocs, config.diff_files || config.diff, config.paste),
        area: config.area,
        wave: None,
    };
    let gathered = gather_context(&opts)?;
    let budgeted = trim_context_with_breakdown(gathered, DEFAULT_CONTEXT_BUDGET);
    let base_prompt = format_prompt(PromptFormatMode::Full, &budgeted).into_string();
    let prompt = format!("{}\n\n<lf:step>\n{}\n</lf:step>\n\n{}\n", base_prompt, step_content, options.suffix);

    let launch = AgentConfig {
        task_prompt: prompt,
        agent: config.agent.clone(),
        cwd: Some(repo.to_path_buf()),
        skip_permissions: true,
        ..Default::default()
    };
    let process = ProcessConfig {
        auto: true,
        stream: true,
        timeout: options.timeout,
        ..Default::default()
    };
    let capabilities = AgentCapabilities { chrome: config.chrome };

    progress.status(&format!("Launching {} agent...", options.step_name));
    let result = launch_agent(&launch, &process, &capabilities)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }
    Ok(())
}
```

`run_rebase_agent` in `ops/rebase.rs` becomes a thin wrapper:

```rust
fn run_rebase_agent(repo: &Path, onto: &str, progress: &impl Progress) -> OpsResult<()> {
    let options = BuiltinAgentOptions {
        step_name: "rebase".into(),
        suffix: format!("Rebase onto: {onto}"),
        timeout: Some(Duration::from_secs(30 * 60)),  // 30 minutes
    };
    run_builtin_agent(repo, &options, progress)
}
```

### 4. Push failure escalation via debug agent

After the existing fetch+rebase+retry cycle in `post_step_sync`, escalate to a debug agent session instead of hard-failing.

```rust
// In post_step_sync, after rebase+retry push fails:
Err(push_err) => {
    warn!("push retry exhausted, escalating to debug agent");
    let error_context = format!(
        "git push to origin/{branch} failed after fetch+rebase retry.\n\
         Error: {push_err}\n\
         Working directory: {}\n\
         Branch: {branch}",
        worktree.display()
    );
    let agent_opts = BuiltinAgentOptions {
        step_name: "debug".into(),
        suffix: error_context.clone(),
        timeout: Some(Duration::from_secs(5 * 60)),  // 5 minutes
    };
    match run_builtin_agent(worktree, &agent_opts, &TracingProgress) {
        Ok(()) => {
            push_with_upstream(worktree, "origin", branch)
                .map_err(|err| anyhow!(
                    "push failed after debug agent intervention.\n\
                     Original error: {push_err}\n\
                     Post-agent error: {err}\n\
                     Worktree: {}\n\
                     Branch: {branch}\n\
                     Manual resolution may be needed.",
                    worktree.display()
                ))
        }
        Err(agent_err) => {
            Err(anyhow!(
                "push failed and debug agent could not resolve it.\n\
                 Push error: {push_err}\n\
                 Agent error: {agent_err}\n\
                 Worktree: {}\n\
                 Branch: {branch}\n\
                 Manual resolution needed.",
                worktree.display()
            ))
        }
    }
}
```

No `run_debug_agent` function needed — `run_builtin_agent` with `step_name: "debug"` does the job directly.

The retry path's rebase also upgrades from bare `rebase()` to `rebase_with_recovery()`, so conflicts during the push-retry cycle also route through the agent.

### 4a. Agent session timeouts

`launch_agent` (CLI-side) has no timeout support — it does unbounded `wait()`. The daemon's `LocalProcessExecutor` has timeouts via `tokio::time::timeout`, but sync helpers call `launch_agent` directly, bypassing that.

Add an optional `timeout: Option<Duration>` field to `ProcessConfig`. `launch_agent` enforces it via `Arc<Mutex<Child>>` shared between the wait thread and a monitor thread. The monitor thread calls `child.kill()` through the shared handle, avoiding the PID-reuse race that raw `libc::kill` would have.

```rust
// In ProcessConfig:
pub timeout: Option<Duration>,

// In launch_agent, after spawning child:
if let Some(timeout) = process.timeout {
    let child = Arc::new(Mutex::new(child));
    let child_for_timeout = Arc::clone(&child);
    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        if let Ok(mut c) = child_for_timeout.lock() {
            let _ = c.kill();
        }
    });
    let status = child.lock().expect("lock poisoned").wait()?;
    // ...
}
```

Timeouts are set per call site:
- Rebase agent: 30 minutes (conflict resolution can involve many files)
- Debug agent: 5 minutes (diagnostic, not heavy editing)

Both are configured via `BuiltinAgentOptions.timeout`, which `run_builtin_agent` passes through to `ProcessConfig`.

### 5. Worktree reuse eager sync (already done)

`sync_existing_worktree` already does dual rebase on worktree reuse. The remaining gap: it uses bare `rebase()`, not `rebase_with_recovery`. Switching it to the shared `dual_rebase` (which uses `rebase_with_recovery`) closes this gap for free.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Merge instead of rebase | Simpler conflict model, no history rewriting | Creates merge commits that clutter the wave branch history. PRs become hard to review. |
| Lock-based push serialization | Prevents conflicts entirely | Adds distributed lock infrastructure (redis/file). Overkill — rebasing handles races fine. |
| Retry loop with backoff for push | More retries before escalation | Delays feedback. If rebase+retry fails once, the problem is structural, not transient. Agent escalation is faster to resolution. |
| Skip upstream rebase, only rebase on wave branch | Less rebase surface area | Wave branches drift from main. Merge conflicts accumulate and become harder to resolve later. |
| Use `NullProgress` instead of `TracingProgress` | No new code | Drops all messages silently. Executor sync is the one place where visibility into rebase/agent activity matters most. |

## Key decisions

**`rebase_with_recovery` over bare `rebase` everywhere.** Every rebase in the executor should route conflicts through the agent. Silent failures were the worst part of the 03.5 implementation — a step running on stale code wastes compute and produces commits that won't merge. Agent recovery is worth the latency.

**Debug agent for push failures, not more retries.** A push that fails after fetch+rebase indicates something the automation doesn't understand. An agent can read the error output and act on it. This is the same pattern as rebase recovery — escalate to an agent when git operations fail.

**Shared `run_builtin_agent`, not duplicated agent launch code.** `run_rebase_agent` and push escalation both do the same thing: load a builtin step, gather context, launch headlessly. One public helper in `ops/agent.rs` eliminates the duplication. `run_rebase_agent` becomes a thin wrapper. Push escalation calls the helper directly with `step_name: "debug"`.

**Shared `dual_rebase` helper, not two call sites.** `pre_step_sync` and `sync_existing_worktree` currently implement the same logic differently. One function, called from both, eliminates the drift.

**`main_repo` parameter threading.** `pre_step_sync` needs the main repo path for `get_default_branch`. Threading it from the executor is a mechanical change — the executor already knows the repo path from `wave.repo`. The alternative (discovering it from the worktree path) is fragile.

**Agent escalation is best-effort, not blocking.** If `rebase_with_recovery` or `run_builtin_agent` fails (API down, rate limit, agent error), the `OpsError` propagates and the run fails — same as today's behavior. The design doesn't add retry loops around agent sessions. This means an API outage still kills runs, but it doesn't make things worse than the current hard-fail behavior.

## Scope

- In scope: `pre_step_sync`, `post_step_sync`, `sync_existing_worktree`, and a `TracingProgress` adapter
- In scope: `run_builtin_agent` public helper in `ops/agent.rs` (replaces `run_rebase_agent` pattern)
- In scope: `run_rebase_agent` becomes thin wrapper around `run_builtin_agent`
- In scope: Push escalation calls `run_builtin_agent` with `"debug"` step directly (no `run_debug_agent`)
- In scope: Upgrade `post_step_sync` retry rebase from bare to `rebase_with_recovery`
- In scope: `timeout` field on `ProcessConfig`, enforced in `launch_agent` via `Arc<Mutex<Child>>`
- In scope: 30-min timeout for rebase agent, 5-min timeout for debug agent
- In scope: Rich error messages on agent timeout/failure (original error + agent error + worktree path)
- Out of scope: Concurrency limiting for listen fan-out (separate concern)
- Out of scope: Push retry backoff (agent escalation replaces this)
- Out of scope: Changes to `rebase_with_recovery` itself (it works as-is)
- Out of scope: Agent rate limiting (separate concern)

## Done when

```bash
# Dual rebase: pre_step_sync rebases onto both wave branch and default branch
cargo test -p loopflow pre_step_sync  # unit test covers both rebases

# Rebase recovery: conflicts route through agent
cargo test -p loopflow rebase_recovery  # test that rebase failure triggers agent

# Push escalation: debug agent called after retry exhaustion
cargo test -p loopflow push_escalation  # test that push failure escalates

# Worktree sync: uses shared dual_rebase with recovery
cargo test -p loopflow sync_worktree  # existing behavior preserved

# Integration: full cycle with concurrent pushes
cargo test -p loopflow git_sync  # all sync tests pass
```

Observable outcome: a CI-fix run pushing to a wave's PR branch while the main run is mid-flow no longer hard-fails the main run. The main run picks up the CI-fix commits at the next step boundary via dual rebase, and any conflicts get resolved by the rebase agent.
