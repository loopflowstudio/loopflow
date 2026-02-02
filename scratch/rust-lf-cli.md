# 01: Rust lf CLI

Replace Python `lf` with a Rust CLI that directly uses loopflow-engine.

## Problem

The Python `lf` CLI works but introduces friction:
- Startup latency from Python interpreter
- Dependency management complexity (uv, venv activation)
- Two-language boundary between CLI (Python) and engine (Rust)
- Distribution complexity (PyPI package with bundled Rust binaries)

Users want `lf debug -c` to feel instant. Rust delivers that.

## Approach

Build `lf` as a pure Rust CLI in `rust/lf/` that:
1. Links directly to `loopflow-engine` (no IPC, no JSON)
2. Replicates Python CLI's command structure and flags exactly
3. Spawns agent CLIs the same way Python does
4. Shares zero code with `lfd`—stateless, no gRPC, no daemon

The engine already has everything: `gather_context()`, `format_prompt()`, `launch_agent()`, git operations. The CLI is a thin command parser over these.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep Python, bundle Rust binary | Two codebases, startup still slow | Doesn't solve the core problem |
| Port Python to Rust incrementally | Hybrid complexity, two CLIs during transition | Clean break is simpler |
| Use lfd as execution backend | Adds daemon dependency, complicates local dev | Stateless CLI is the right default |
| Generate CLI from shared spec | Abstraction overhead, drift risk | Direct implementation is clearer |

## Key decisions

**Stateless design.** The Python CLI works without lfd running. The Rust CLI does too. `lf debug` spawns claude directly—no daemon, no socket, no state. This follows the wave principle: "local is simple."

**Exact command parity.** Every Python flag works identically in Rust. Users shouldn't notice the switch except for speed. `-d`, `-a`, `-c`, `--model`, `--yolo`, `-i`—all preserved.

**execvp for interactive steps.** When running a single interactive step, replace the process entirely. No subprocess management, no signal forwarding complexity. The agent owns the terminal.

**Subprocess for flows.** When chaining steps, use subprocess so the flow can continue after each step completes. Capture exit codes, handle failures.

**No new features.** This is a port, not a redesign. Feature parity first, enhancements later.

## Scope

In scope:
- `lf <step>` with all flags (`-d`, `-a`, `-c`, `--model`, `--yolo`, `-i`, `-b`)
- `lf flow <name>` executing all flow item types
- `lf context`, `lf config`, `lf flows`, `lf steps`, `lf directions`
- `lf ops` subcommands (rebase, push, land, pr, sync, next, commit, abandon)
- Config loading and merging (global + repo)
- Step/flow/direction discovery (repo, global, builtin)
- Token counting and context trimming

Out of scope:
- `lf inline` (rarely used, add later if needed)
- Wave metadata integration (Phase 2 work)
- Skill sources and external skills (future enhancement)
- Python bindings for the CLI (engine bindings are separate)

## Architecture

```
rust/lf/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point, clap setup
    ├── commands/
    │   ├── mod.rs
    │   ├── step.rs          # lf <step>
    │   ├── flow.rs          # lf flow <name>
    │   ├── context.rs       # lf context
    │   ├── config.rs        # lf config
    │   ├── list.rs          # lf flows/steps/directions
    │   └── ops/
    │       ├── mod.rs
    │       ├── rebase.rs
    │       ├── push.rs
    │       ├── land.rs
    │       ├── pr.rs
    │       ├── next.rs
    │       ├── commit.rs
    │       └── abandon.rs
    └── discovery.rs         # Step/flow/direction lookup
```

### Command dispatch

```rust
#[derive(Parser)]
#[command(name = "lf")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Step name (default command when no subcommand)
    step: Option<String>,

    // Flags that apply to step execution
    #[arg(short, long, value_delimiter = ',')]
    directions: Vec<String>,

    #[arg(short, long)]
    area: Vec<PathBuf>,

    #[arg(short, long)]
    clipboard: bool,

    #[arg(long)]
    model: Option<String>,

    #[arg(long)]
    yolo: bool,

    #[arg(short, long)]
    interactive: bool,

    #[arg(short, long)]
    batch: bool,

    #[arg(short, long)]
    list: bool,
}

#[derive(Subcommand)]
enum Commands {
    Run { step: String },
    Flow { name: String },
    Context { #[arg(long)] tokens: bool, #[arg(long)] trim: Option<usize> },
    Config { #[arg(long)] global: bool, #[arg(long)] repo: bool },
    Flows,
    Steps,
    Directions,
    #[command(subcommand)]
    Ops(OpsCommand),
}
```

### Step execution

```rust
fn run_step(step_name: &str, opts: &StepOptions) -> Result<()> {
    let repo = find_repo_root()?;
    let config = loopflow_engine::load_config(&repo)?;
    let step = discover_step(&repo, step_name)?;

    // Merge config: CLI > frontmatter > global
    let directions = merge_directions(&opts.directions, &step, &config);
    let areas = merge_areas(&opts.area, &step, &config);

    // Gather and trim context
    let context = loopflow_engine::gather_context(&GatherContextOpts {
        repo: &repo,
        areas: &areas,
        directions: &directions,
        diff: true,
        clipboard: opts.clipboard,
    })?;
    let context = loopflow_engine::trim_context(context, config.token_budget);

    // Format prompt
    let prompt = loopflow_engine::format_prompt(&context, &step)?;

    // Build agent config
    let model = opts.model.as_ref()
        .map(|m| parse_model(m))
        .unwrap_or(config.agent_model);

    let auto_mode = opts.batch || opts.yolo || !step.interactive;

    // Launch
    let launch_config = LaunchConfig {
        backend: model.backend,
        model: model.name,
        prompt,
        working_dir: repo,
        auto_mode,
        chrome: config.chrome,
    };

    if !auto_mode && !opts.batch {
        // Interactive: replace process
        loopflow_engine::launch_agent_exec(launch_config)?;
        unreachable!()
    } else {
        // Batch: subprocess, wait for completion
        let result = loopflow_engine::launch_agent(launch_config)?;
        std::process::exit(result.exit_code);
    }
}
```

### Flow execution

```rust
fn run_flow(flow_name: &str, opts: &StepOptions) -> Result<()> {
    let repo = find_repo_root()?;
    let flow = loopflow_engine::load_flow(&repo, flow_name)?;

    for item in &flow.items {
        match item {
            FlowItem::Step { name, directions, .. } => {
                let mut step_opts = opts.clone();
                step_opts.directions.extend(directions.clone());
                run_step_subprocess(&repo, name, &step_opts)?;
            }
            FlowItem::Fork { branches, synthesize } => {
                // Run branches sequentially (parallelization is lfd's job)
                for branch in branches {
                    run_step_subprocess(&repo, &branch.step, &branch_opts(opts, branch))?;
                }
                if let Some(synth) = synthesize {
                    run_step_subprocess(&repo, synth, opts)?;
                }
            }
            FlowItem::Choose { options } => {
                for opt in options {
                    if evaluate_condition(&repo, &opt.condition)? {
                        run_step_subprocess(&repo, &opt.step, opts)?;
                        break;
                    }
                }
            }
            FlowItem::LoopUntilEmpty { source, step } => {
                while !is_source_empty(&repo, source)? {
                    run_step_subprocess(&repo, &step.name(), opts)?;
                }
            }
        }
    }
    Ok(())
}
```

### Discovery

Step/flow/direction discovery mirrors Python's lookup order:

```rust
fn discover_step(repo: &Path, name: &str) -> Result<Step> {
    // 1. Repo-local: .lf/steps/{name}.md, .claude/commands/{name}.md
    if let Some(step) = try_load_step(repo.join(".lf/steps").join(format!("{name}.md")))? {
        return Ok(step);
    }
    if let Some(step) = try_load_step(repo.join(".claude/commands").join(format!("{name}.md")))? {
        return Ok(step);
    }

    // 2. Global: ~/.lf/steps/{name}.md, ~/.claude/commands/{name}.md
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    if let Some(step) = try_load_step(home.join(".lf/steps").join(format!("{name}.md")))? {
        return Ok(step);
    }
    if let Some(step) = try_load_step(home.join(".claude/commands").join(format!("{name}.md")))? {
        return Ok(step);
    }

    // 3. Built-in
    load_builtin_step(name)
}
```

### Ops commands

Each ops command is thin wrapper around `loopflow_engine::git`:

```rust
// lf ops rebase
fn rebase(onto: Option<String>) -> Result<()> {
    let repo = find_repo_root()?;
    let onto = onto.unwrap_or_else(|| "main".to_string());

    match loopflow_engine::git::rebase(&repo, &onto)? {
        RebaseResult::Success => println!("Rebased onto {onto}"),
        RebaseResult::Conflict => {
            eprintln!("Rebase conflict. Resolve and run: git rebase --continue");
            std::process::exit(1);
        }
    }
    Ok(())
}

// lf ops land
fn land(strategy: Option<LandStrategy>) -> Result<()> {
    let repo = find_repo_root()?;
    let strategy = strategy.unwrap_or(LandStrategy::SquashMerge);

    // Clear scratch/ before landing
    let scratch = repo.join("scratch");
    if scratch.exists() {
        for entry in fs::read_dir(&scratch)? {
            let path = entry?.path();
            if path.is_file() {
                fs::remove_file(&path)?;
            }
        }
    }

    loopflow_engine::git::land(&repo, strategy)?;
    println!("Landed");
    Ok(())
}
```

## Engine additions

The engine needs a few additions for full CLI support:

**launch_agent_exec**: Replace current process (for interactive mode)
```rust
pub fn launch_agent_exec(config: LaunchConfig) -> Result<std::convert::Infallible> {
    let cmd = build_command(&config);
    let err = exec::execvp(&cmd[0], &cmd);
    Err(anyhow!("exec failed: {err}"))
}
```

**Builtin steps**: Embed step markdown files at compile time
```rust
const BUILTIN_STEPS: &[(&str, &str)] = &[
    ("debug", include_str!("builtins/debug.md")),
    ("implement", include_str!("builtins/implement.md")),
    // ...
];
```

**Condition evaluation**: For `Choose` flow items
```rust
pub fn evaluate_condition(repo: &Path, condition: &str) -> Result<bool> {
    match condition {
        "has_diff" => Ok(!git::is_clean(repo)?),
        "has_scratch" => Ok(repo.join("scratch").read_dir()?.next().is_some()),
        _ => Err(anyhow!("unknown condition: {condition}")),
    }
}
```

## Done when

```bash
# All these work identically to Python lf:
lf debug -c
lf implement -d product-engineer -a src/
lf flow ship
lf context --tokens
lf config
lf ops rebase
lf ops land
lf steps

# And pass tests:
cargo test -p lf
```

Verification: Run the Python test suite against the Rust binary. All tests pass.
