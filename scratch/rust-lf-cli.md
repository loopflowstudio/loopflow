# 01: Rust lf CLI

Replace Python `lf` with a Rust CLI that directly uses loopflow-engine.

## Context

Today the Python `lf` CLI:
- Assembles prompts from steps, directions, context
- Spawns claude/codex/gemini CLI directly
- Manages worktrees and git operations via `lf-engine`
- Stateless - no daemon communication

The Rust `loopflow-engine` already has:
- Flow parsing
- Context gathering and prompt assembly
- Agent invocation
- Git operations
- PyO3 bindings

## Goal

A Rust `lf` binary that:
1. Directly uses loopflow-engine for all operations
2. Maintains stateless design (no daemon dependency)
3. Full command parity with Python `lf`
4. Preserves PyO3 bindings so Python code can use loopflow-engine

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  lf (Rust CLI)                                              │
│                                                             │
│  ┌─────────────┐  ┌──────────────────────────────────────┐ │
│  │ Commands    │  │ loopflow-engine                      │ │
│  │             │──│                                      │ │
│  │ step, flow  │  │ gather_context() → format_prompt()   │ │
│  │ context     │  │ launch_agent()                       │ │
│  │ config      │  │ git operations                       │ │
│  └─────────────┘  └──────────────────────────────────────┘ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
         │
         │ spawns
         ▼
  ┌─────────────┐
  │ claude CLI  │
  │ codex CLI   │
  │ gemini CLI  │
  └─────────────┘
```

No daemon. No gRPC. Direct execution.

## Commands

### Core Commands

```bash
# Run a step
lf debug                   # Run debug step
lf debug -c                # With clipboard content
lf debug -d designer       # With direction
lf implement               # Run implement step

# Run a flow
lf flow ship               # Run ship flow
lf flow grind              # Run grind flow

# Step with full options
lf debug \
  -d designer,product-engineer \  # Multiple directions
  -a src/ \                       # Area
  -c \                            # Clipboard
  --model claude:sonnet \         # Model override
  --yolo                          # Auto-approve
```

### Utility Commands

```bash
lf context                 # Show assembled context
lf context --tokens        # With token counts
lf context --trim 100000   # Trimmed to budget

lf config                  # Show merged config
lf config --global         # Global config only
lf config --repo           # Repo config only

lf flows                   # List available flows
lf steps                   # List available steps
lf directions              # List available directions
```

### Git Operations (via lf-engine)

```bash
lf ops rebase              # Rebase onto main
lf ops push                # Push current branch
lf ops land                # Land PR (squash merge)
lf ops pr                  # Create draft PR
lf ops sync                # Sync main branch
lf ops next                # Start next iteration
```

## Implementation

### Crate Structure

```
rust/lf/
├── Cargo.toml
└── src/
    ├── main.rs           # Entry point, clap setup
    ├── commands/
    │   ├── mod.rs
    │   ├── step.rs       # lf <step>
    │   ├── flow.rs       # lf flow <name>
    │   ├── context.rs    # lf context
    │   ├── config.rs     # lf config
    │   └── ops.rs        # lf ops <subcommand>
    └── output.rs         # Terminal output formatting
```

### Main Entry Point

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lf")]
#[command(about = "Run steps and flows with coding agents")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Step name (when no subcommand)
    #[arg(value_name = "STEP")]
    step: Option<String>,

    /// Directions to apply
    #[arg(short, long, value_delimiter = ',')]
    directions: Vec<String>,

    /// Area paths
    #[arg(short, long)]
    area: Vec<PathBuf>,

    /// Include clipboard content
    #[arg(short, long)]
    clipboard: bool,

    /// Model override (e.g., claude:opus, codex, gemini:2.5-pro)
    #[arg(long)]
    model: Option<String>,

    /// Auto-approve all actions
    #[arg(long)]
    yolo: bool,

    /// Interactive mode (default for interactive steps)
    #[arg(short, long)]
    interactive: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a flow
    Flow { name: String },
    /// Show assembled context
    Context {
        #[arg(long)]
        tokens: bool,
        #[arg(long)]
        trim: Option<usize>,
    },
    /// Show configuration
    Config {
        #[arg(long)]
        global: bool,
        #[arg(long)]
        repo: bool,
    },
    /// Git operations
    Ops {
        #[command(subcommand)]
        op: OpsCommand,
    },
    /// List available flows
    Flows,
    /// List available steps
    Steps,
    /// List available directions
    Directions,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Flow { name }) => run_flow(&name, &cli),
        Some(Commands::Context { tokens, trim }) => show_context(tokens, trim),
        Some(Commands::Config { global, repo }) => show_config(global, repo),
        Some(Commands::Ops { op }) => run_ops(op),
        Some(Commands::Flows) => list_flows(),
        Some(Commands::Steps) => list_steps(),
        Some(Commands::Directions) => list_directions(),
        None => {
            // Default: run step
            let step = cli.step.ok_or_else(|| anyhow!("no step specified"))?;
            run_step(&step, &cli)
        }
    }
}
```

### Step Execution

```rust
fn run_step(step_name: &str, cli: &Cli) -> Result<()> {
    let repo = find_repo_root()?;
    let config = loopflow_engine::config::load_config(&repo)?;

    // Load step definition
    let step = loopflow_engine::flow::load_step(&repo, step_name)?;

    // Gather context
    let context_config = ContextConfig {
        repo: &repo,
        areas: &cli.area,
        directions: &cli.directions,
        diff: true,
        clipboard: cli.clipboard,
        token_budget: config.token_budget,
    };
    let context = loopflow_engine::prompt::gather_context(&context_config)?;

    // Trim if needed
    let context = loopflow_engine::prompt::trim_context(context, config.token_budget);

    // Format prompt
    let prompt = loopflow_engine::prompt::format_prompt(&context, &step)?;

    // Resolve model
    let model = cli.model.as_ref()
        .map(|m| loopflow_engine::config::parse_model(m))
        .unwrap_or_else(|| config.agent_model.clone());

    // Launch agent
    let agent_config = AgentConfig {
        backend: model.backend,
        model: model.name,
        prompt,
        working_dir: repo.clone(),
        auto_mode: cli.yolo || !step.interactive,
        streaming: true,
        chrome: config.chrome,
        skip_permissions: cli.yolo,
    };

    loopflow_engine::agent::launch_agent(agent_config)?;

    Ok(())
}
```

### Flow Execution

```rust
fn run_flow(flow_name: &str, cli: &Cli) -> Result<()> {
    let repo = find_repo_root()?;

    // Load flow
    let flow = loopflow_engine::flow::load_flow(&repo, flow_name)?;

    // Execute each item
    for item in &flow.items {
        match item {
            FlowItem::Step { name, interactive, directions } => {
                // Merge directions
                let mut dirs = cli.directions.clone();
                dirs.extend(directions.clone());

                println!("→ Running step: {}", name);
                run_step_internal(&repo, name, &dirs, cli)?;
            }
            FlowItem::Fork { branches, synthesize } => {
                // Run branches (could parallelize)
                let mut results = Vec::new();
                for branch in branches {
                    println!("→ Fork branch: {}", branch.name);
                    let result = run_step_internal(&repo, &branch.step, &branch.directions, cli)?;
                    results.push(result);
                }

                // Synthesize if specified
                if let Some(synth_step) = synthesize {
                    println!("→ Synthesizing: {}", synth_step);
                    run_step_internal(&repo, synth_step, &[], cli)?;
                }
            }
            FlowItem::Choose { options } => {
                // Evaluate conditions, run matching branch
                for option in options {
                    if evaluate_condition(&repo, &option.condition)? {
                        run_step_internal(&repo, &option.step, &[], cli)?;
                        break;
                    }
                }
            }
            FlowItem::LoopUntilEmpty { source, step } => {
                while !is_source_empty(&repo, source)? {
                    run_flow_item(&repo, step, cli)?;
                }
            }
        }
    }

    Ok(())
}
```

### Context Display

```rust
fn show_context(tokens: bool, trim: Option<usize>) -> Result<()> {
    let repo = find_repo_root()?;
    let config = loopflow_engine::config::load_config(&repo)?;

    let context = loopflow_engine::prompt::gather_context(&ContextConfig {
        repo: &repo,
        areas: &[],
        directions: &[],
        diff: true,
        clipboard: false,
        token_budget: config.token_budget,
    })?;

    if tokens {
        let analysis = loopflow_engine::prompt::analyze_tokens(&context);
        println!("Token counts:");
        println!("  docs:       {:>8}", analysis.docs);
        println!("  diff:       {:>8}", analysis.diff);
        println!("  directions: {:>8}", analysis.directions);
        println!("  summaries:  {:>8}", analysis.summaries);
        println!("  ─────────────────");
        println!("  total:      {:>8}", analysis.total);
        println!("  budget:     {:>8}", config.token_budget);
        return Ok(());
    }

    let context = if let Some(budget) = trim {
        loopflow_engine::prompt::trim_context(context, budget)
    } else {
        context
    };

    // Print formatted context
    for doc in &context.docs {
        println!("── {} ──", doc.path.display());
        println!("{}", doc.content);
    }

    if let Some(diff) = &context.diff {
        println!("── diff ──");
        println!("{}", diff);
    }

    Ok(())
}
```

## PyO3 Bindings

loopflow-engine already has PyO3 bindings. Ensure they cover:

```rust
// rust/loopflow-engine/src/python.rs

#[pymodule]
fn loopflow_engine(py: Python, m: &PyModule) -> PyResult<()> {
    // Config
    m.add_function(wrap_pyfunction!(load_config, m)?)?;

    // Context
    m.add_function(wrap_pyfunction!(gather_context, m)?)?;
    m.add_function(wrap_pyfunction!(count_tokens, m)?)?;
    m.add_function(wrap_pyfunction!(trim_context, m)?)?;
    m.add_function(wrap_pyfunction!(format_prompt, m)?)?;

    // Flow
    m.add_function(wrap_pyfunction!(load_flow, m)?)?;
    m.add_function(wrap_pyfunction!(load_step, m)?)?;
    m.add_function(wrap_pyfunction!(list_flows, m)?)?;
    m.add_function(wrap_pyfunction!(list_steps, m)?)?;

    // Agent
    m.add_function(wrap_pyfunction!(launch_agent, m)?)?;
    m.add_function(wrap_pyfunction!(check_cli_available, m)?)?;

    // Git (via lf-engine or direct)
    m.add_function(wrap_pyfunction!(rebase, m)?)?;
    m.add_function(wrap_pyfunction!(push, m)?)?;
    m.add_function(wrap_pyfunction!(land, m)?)?;
    m.add_function(wrap_pyfunction!(pr_create_draft, m)?)?;
    m.add_function(wrap_pyfunction!(sync_main, m)?)?;

    Ok(())
}
```

Python usage:

```python
import loopflow_engine

# Load config
config = loopflow_engine.load_config("/path/to/repo")

# Gather context
context = loopflow_engine.gather_context(
    repo="/path/to/repo",
    areas=["src/"],
    directions=["designer"],
    diff=True,
    clipboard=False,
)

# Count tokens
tokens = loopflow_engine.count_tokens(context.formatted)

# Launch agent
loopflow_engine.launch_agent(
    backend="claude",
    model="sonnet",
    prompt=context.formatted,
    working_dir="/path/to/repo",
    auto_mode=True,
)
```

## Done When

- [ ] `lf <step>` runs steps with full flag parity
- [ ] `lf flow <name>` runs flows
- [ ] `lf context` shows assembled context
- [ ] `lf context --tokens` shows token analysis
- [ ] `lf config` shows merged config
- [ ] `lf flows`, `lf steps`, `lf directions` list available items
- [ ] `lf ops` subcommands work (rebase, push, land, pr, sync, next)
- [ ] All flags work: `-d`, `-a`, `-c`, `--model`, `--yolo`, `-i`
- [ ] Fork steps execute branches
- [ ] Choose steps evaluate conditions
- [ ] LoopUntilEmpty iterates correctly
- [ ] PyO3 bindings expose all engine functions
- [ ] Python can `import loopflow_engine` and use it
- [ ] `cargo install lf` works
- [ ] Full parity with Python `lf` commands

## Dependencies

- `clap` for CLI parsing
- `loopflow-engine` for all execution
- No daemon, no gRPC, no network
