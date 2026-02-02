# 01: Rust lf CLI

Replace the Python `lf` CLI with a Rust binary that directly uses loopflow-engine.

## Problem

The Python `lf` CLI is 8,600 lines of code duplicating logic that already exists in the Rust `loopflow-engine`. This creates:

- **Two implementations to maintain.** Context gathering, prompt formatting, agent invocation—all implemented twice.
- **Python runtime dependency.** Users must have Python + uv for what should be a statically-linked binary.
- **Startup latency.** Python interpreter overhead on every `lf debug -c`.
- **Distribution complexity.** PyPI wheels, platform-specific binaries, venv activation.

The Rust daemon `lfd` exists but isn't the solution—users need a stateless CLI for interactive work. The daemon is for waves and automation; the CLI is for humans at a terminal.

## Approach

Build `rust/lf/` as a thin CLI layer over loopflow-engine. The CLI handles argument parsing and output formatting; all execution logic lives in the engine.

```
┌────────────────────────────────────────────────────────────┐
│  lf (Rust CLI)                                             │
│                                                            │
│  main.rs → clap routing                                    │
│     ↓                                                      │
│  commands/                                                 │
│     step.rs    → loopflow_engine::{gather_context, ...}   │
│     flow.rs    → loopflow_engine::{load_flow, tick_flow}  │
│     context.rs → loopflow_engine::{analyze_tokens, ...}   │
│     ops.rs     → loopflow_engine::git::*                  │
│                                                            │
└────────────────────────────────────────────────────────────┘
         │
         │ spawns via loopflow_engine::agent::launch_agent()
         ▼
   claude / codex / gemini CLI
```

No daemon. No gRPC. No network. Direct execution.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep Python, call Rust via PyO3 | Maintains familiar CLI; uses Rust engine | Still requires Python runtime; doesn't simplify distribution |
| Make lfd the only interface | Single binary for everything | Daemon overhead for interactive work; stateless CLI is the right default for humans |
| FFI from Python to Rust library | Gradual migration, low risk | Complex build system; FFI boundary maintenance; doesn't solve startup latency |
| Port Python incrementally | Lower risk, test as you go | Two CLIs during transition; "v2" maintenance burden; clean break is simpler |
| Generate CLI from shared spec | Guarantees parity | Abstraction overhead; spec drift risk; direct implementation is clearer |

## Key decisions

**Stateless design.** The Python CLI works without lfd. The Rust CLI does too. `lf debug` spawns claude directly—no daemon, no socket, no state. Following wave principle: *"local is simple."*

**Exact command parity.** Every Python flag works identically in Rust. Users shouldn't notice the switch except for speed. `-d`, `-a`, `-c`, `--model`, `--yolo`, `-i`, `-b`, `--web`, `--chrome`, `--wave`—all preserved.

**External subcommand pattern.** `lf debug` isn't a subcommand—it's a step name. clap's `external_subcommand` feature handles this: known commands (`flow`, `ops`, `context`) route to modules; unknown names route to step execution. Matches Python's detection logic.

**execvp for interactive steps.** When running a single interactive step, replace the process entirely. No subprocess management, no signal forwarding. The agent owns the terminal.

**Subprocess for flows.** When chaining steps, use subprocess so the flow can continue after each step completes. Capture exit codes, handle failures.

**No new features.** This is a port, not a redesign. Feature parity first. Following wave principle: *"UX invariants: prompts, flows, directions, and artifact paths must not change."*

## Imagining success

Six months after shipping:

- `lf debug -c` feels instant—users stop noticing startup time entirely.
- `brew install loopflow` just works. No Python, no uv, no venv.
- The Python CLI still exists but nobody uses it. We eventually deprecate it.
- Contributors work entirely in Rust. One language, one test suite, one CI pipeline.
- loopflow-engine becomes the definitive implementation. Python bindings use it; CLI uses it; lfd uses it.

What made it great: **simplicity**. The CLI is under 1,000 lines because the engine does the work. New contributors read `commands/step.rs` and understand everything in five minutes.

## Imagining failure

Six months after shipping:

- Users report subtle behavior differences. `lf implement -d designer` works differently in Rust. Trust erodes.
- Edge cases in the Python CLI weren't documented. Each one becomes a bug report.
- The Rust CLI grew features the Python CLI doesn't have. Now we maintain both.
- loopflow-engine's API evolved for the CLI but broke lfd. Coordination overhead.

What went wrong: **rushing parity**. We declared "done" before testing against real workflows. The Python CLI had eight months of edge case fixes that got lost.

## Scope

**In scope:**
- `lf <step>` with all flags (-d, -a, -c, -m, --yolo, -i, -b, --web, --chrome, --wave)
- `lf flow <name>` with full flow execution (Step, Fork, Choose, LoopUntilEmpty)
- `lf : "prompt"` inline execution
- `lf ops` subcommands (rebase, push, land, pr, sync, next, commit, abandon)
- `lf context [--tokens] [--trim N]` for debugging
- `lf config [--global] [--repo]` display
- `lf --list` formatted output matching Python exactly
- `lf flows`, `lf steps`, `lf directions` listings
- Step/flow/direction discovery (repo → global → builtin)
- Config loading and merging (global + repo, CLI overrides)
- Token counting and context trimming
- Colored terminal output (respecting NO_COLOR)

**Out of scope:**
- Daemon communication (that's lfd's job)
- Wave management (that's lfd's job)
- New features not in Python CLI
- Skill sources and external skills (future enhancement)
- Windows support (Phase 1 is macOS/Linux)

## Done when

```bash
# All these work identically to Python lf:
lf debug -c                           # step with clipboard
lf implement -d product-engineer      # step with direction
lf flow ship                          # flow execution
lf : "add logging to auth.py"         # inline prompt
lf ops rebase && lf ops push          # git operations
lf --list                             # formatted step/flow list
lf context --tokens                   # token analysis

# And these pass:
cargo test -p lf                      # unit tests
cargo clippy -p lf -- -D warnings     # lint clean
```

Verification: Run Python CLI's test scenarios against Rust binary. Zero regressions.

---

## Architecture

```
rust/lf/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point, clap setup
    ├── commands/
    │   ├── mod.rs
    │   ├── step.rs          # lf <step>
    │   ├── inline.rs        # lf : "prompt"
    │   ├── flow.rs          # lf flow <name>
    │   ├── context.rs       # lf context
    │   ├── config.rs        # lf config
    │   ├── list.rs          # lf flows/steps/directions, lf --list
    │   └── ops/
    │       ├── mod.rs
    │       ├── rebase.rs
    │       ├── push.rs
    │       ├── land.rs
    │       ├── pr.rs
    │       ├── next.rs
    │       ├── commit.rs
    │       └── abandon.rs
    ├── discovery.rs         # Step/flow/direction lookup
    └── output.rs            # Terminal colors, formatting
```

### Cargo.toml

```toml
[package]
name = "lf"
version.workspace = true
edition.workspace = true

[[bin]]
name = "lf"
path = "src/main.rs"

[dependencies]
loopflow-engine = { path = "../loopflow-engine" }
clap = { version = "4", features = ["derive", "env"] }
anyhow = "1"
thiserror = "1"
atty = "0.2"

[dev-dependencies]
tempfile = "3"
```

### Command dispatch (main.rs)

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod discovery;
mod output;

#[derive(Parser)]
#[command(name = "lf")]
#[command(about = "Run steps and flows with coding agents")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// List available steps and flows
    #[arg(short, long)]
    list: bool,

    // Flags that apply to step execution
    #[arg(short, long, value_delimiter = ',')]
    direction: Vec<String>,
    #[arg(short, long)]
    area: Vec<PathBuf>,
    #[arg(short, long)]
    clipboard: bool,
    #[arg(short, long)]
    model: Option<String>,
    #[arg(long)]
    yolo: bool,
    #[arg(short, long)]
    interactive: bool,
    #[arg(short, long)]
    batch: bool,
    #[arg(long)]
    web: bool,
    #[arg(long)]
    chrome: Option<bool>,
    #[arg(short, long)]
    wave: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a step (explicit)
    Run {
        step: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Run a named flow
    Flow {
        name: String,
        #[arg(short, long)]
        area: Vec<PathBuf>,
        #[arg(short, long)]
        model: Option<String>,
        #[arg(long)]
        pr: bool,
    },
    /// Run an inline prompt
    #[command(name = ":")]
    Inline { prompt: String },
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
    /// External: step name (when no subcommand matches)
    #[command(external_subcommand)]
    Step(Vec<String>),
}

#[derive(Subcommand)]
enum OpsCommand {
    Rebase { onto: Option<String> },
    Push { #[arg(long)] force: bool },
    Land { #[arg(long)] strategy: Option<String> },
    Pr { title: Option<String>, #[arg(long)] draft: bool },
    Sync,
    Next,
    Commit { #[arg(short)] message: Option<String> },
    Abandon { #[arg(long)] force: bool },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.list {
        return commands::list::show_all();
    }

    match cli.command {
        Some(Commands::Run { step, args }) => {
            commands::step::run(&step, &args, &cli)
        }
        Some(Commands::Flow { name, area, model, pr }) => {
            commands::flow::run(&name, &area, model.as_deref(), pr, &cli)
        }
        Some(Commands::Inline { prompt }) => {
            commands::inline::run(&prompt, &cli)
        }
        Some(Commands::Context { tokens, trim }) => {
            commands::context::show(tokens, trim)
        }
        Some(Commands::Config { global, repo }) => {
            commands::config::show(global, repo)
        }
        Some(Commands::Ops { op }) => {
            commands::ops::run(op)
        }
        Some(Commands::Flows) => commands::list::flows(),
        Some(Commands::Steps) => commands::list::steps(),
        Some(Commands::Directions) => commands::list::directions(),
        Some(Commands::Step(args)) => {
            // External subcommand: first arg is step name
            let step = args.first()
                .ok_or_else(|| anyhow::anyhow!("no step specified"))?;
            let step_args = args.iter().skip(1).cloned().collect::<Vec<_>>();
            commands::step::run(step, &step_args, &cli)
        }
        None => {
            // No args: launch interactive claude with docs context
            commands::step::run_interactive(&cli)
        }
    }
}
```

### Step execution (commands/step.rs)

```rust
use anyhow::{anyhow, Result};
use loopflow_engine::{
    gather_context, format_prompt, load_step, launch_agent,
    GatherContextOpts, LaunchConfig, load_config_or_default, parse_model,
};
use std::path::PathBuf;

pub fn run(step_name: &str, step_args: &[String], cli: &super::Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    let config = load_config_or_default(&repo_root);

    // Load step to check if interactive
    let step = load_step(step_name, &repo_root)?;
    let is_interactive = cli.interactive
        || (!cli.batch && step.interactive.unwrap_or(false));

    // Gather context
    let components = gather_context(&GatherContextOpts {
        repo_root: repo_root.clone(),
        step: Some(step_name.to_string()),
        step_args: step_args.to_vec(),
        run_mode: Some(if is_interactive { "interactive" } else { "auto" }.to_string()),
        directions: cli.direction.clone(),
        lfdocs: true,
        diff_files: true,
        diff: false,
        clipboard: cli.clipboard,
        area: cli.area.first().map(|p| p.to_string_lossy().to_string()),
        wave: cli.wave.clone(),
    })?;

    // Format prompt
    let prompt = format_prompt(&components);

    // Handle --web: copy to clipboard and open browser
    if cli.web {
        copy_to_clipboard(&prompt)?;
        let (backend, _) = parse_model(cli.model.as_deref().unwrap_or(&config.agent_model));
        open_web_client(&backend)?;
        println!("Copied to clipboard.");
        return Ok(());
    }

    // Launch agent
    let model = cli.model.as_deref().unwrap_or(&config.agent_model);
    let (backend, variant) = parse_model(model);

    let launch_config = LaunchConfig {
        auto: !is_interactive,
        stream: !is_interactive,
        skip_permissions: cli.yolo || config.yolo,
        model_variant: variant,
        chrome: cli.chrome.unwrap_or(config.chrome),
        cwd: Some(repo_root),
    };

    let result = launch_agent(&backend, &prompt, &launch_config)?;
    std::process::exit(result.exit_code);
}

pub fn run_interactive(cli: &super::Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    let config = load_config_or_default(&repo_root);

    // Interactive mode with docs context, no step
    let components = gather_context(&GatherContextOpts {
        repo_root: repo_root.clone(),
        step: None,
        run_mode: Some("interactive".to_string()),
        directions: cli.direction.clone(),
        lfdocs: true,
        diff_files: true,
        clipboard: cli.clipboard,
        ..Default::default()
    })?;

    let prompt = format_prompt(&components);
    let model = cli.model.as_deref().unwrap_or(&config.agent_model);
    let (backend, variant) = parse_model(model);

    let launch_config = LaunchConfig {
        auto: false,
        stream: false,
        skip_permissions: cli.yolo || config.yolo,
        model_variant: variant,
        chrome: cli.chrome.unwrap_or(config.chrome),
        cwd: Some(repo_root),
    };

    let result = launch_agent(&backend, &prompt, &launch_config)?;
    std::process::exit(result.exit_code);
}

fn find_repo_root() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;

    if !output.status.success() {
        return Ok(std::env::current_dir()?);
    }

    Ok(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::io::Write;
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    child.stdin.take().unwrap().write_all(text.as_bytes())?;
    child.wait()?;
    Ok(())
}

fn open_web_client(backend: &str) -> Result<()> {
    let url = match backend {
        "claude" => "https://claude.ai/new",
        "codex" => "https://chatgpt.com",
        "gemini" => "https://aistudio.google.com/prompts/new_chat",
        _ => "https://claude.ai/new",
    };
    std::process::Command::new("open").arg(url).spawn()?;
    Ok(())
}
```

### Discovery (discovery.rs)

```rust
use anyhow::{anyhow, Result};
use loopflow_engine::Step;
use std::path::{Path, PathBuf};

/// Discover a step by name, checking repo → global → builtin
pub fn discover_step(repo: &Path, name: &str) -> Result<Step> {
    // 1. Repo-local
    let repo_paths = [
        repo.join(".lf/steps").join(format!("{name}.md")),
        repo.join(".claude/commands").join(format!("{name}.md")),
    ];
    for path in repo_paths {
        if path.exists() {
            return loopflow_engine::load_step(name, repo);
        }
    }

    // 2. Global
    if let Some(home) = dirs::home_dir() {
        let global_paths = [
            home.join(".lf/steps").join(format!("{name}.md")),
            home.join(".claude/commands").join(format!("{name}.md")),
        ];
        for path in global_paths {
            if path.exists() {
                return loopflow_engine::load_step(name, &home);
            }
        }
    }

    // 3. Builtin (if engine supports it)
    loopflow_engine::load_step(name, repo)
}

/// List all available steps
pub fn list_steps(repo: &Path) -> Vec<String> {
    let mut steps = Vec::new();

    // Repo-local
    for dir in [repo.join(".lf/steps"), repo.join(".claude/commands")] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.path().file_stem() {
                    if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
                        steps.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    // Global
    if let Some(home) = dirs::home_dir() {
        for dir in [home.join(".lf/steps"), home.join(".claude/commands")] {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.path().file_stem() {
                        if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
                            let name = name.to_string_lossy().to_string();
                            if !steps.contains(&name) {
                                steps.push(name);
                            }
                        }
                    }
                }
            }
        }
    }

    steps.sort();
    steps.dedup();
    steps
}

/// List all available flows
pub fn list_flows(repo: &Path) -> Vec<String> {
    let mut flows = Vec::new();
    let flows_dir = repo.join(".lf/flows");

    if let Ok(entries) = std::fs::read_dir(flows_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.path().file_stem() {
                let ext = entry.path().extension().map(|e| e.to_string_lossy().to_string());
                if matches!(ext.as_deref(), Some("yaml") | Some("yml") | Some("json")) {
                    flows.push(name.to_string_lossy().to_string());
                }
            }
        }
    }

    flows.sort();
    flows
}

/// List all available directions
pub fn list_directions(repo: &Path) -> Vec<String> {
    let mut directions = Vec::new();
    let dir = repo.join(".lf/directions");

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.path().file_stem() {
                if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
                    directions.push(name.to_string_lossy().to_string());
                }
            }
        }
    }

    directions.sort();
    directions
}
```

### Output formatting (output.rs)

```rust
pub fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && atty::is(atty::Stream::Stdout)
}

pub struct Colors {
    pub cyan: &'static str,
    pub bold: &'static str,
    pub dim: &'static str,
    pub yellow: &'static str,
    pub green: &'static str,
    pub reset: &'static str,
}

impl Colors {
    pub fn new() -> Self {
        if use_color() {
            Colors {
                cyan: "\x1b[36m",
                bold: "\x1b[1m",
                dim: "\x1b[90m",
                yellow: "\x1b[33m",
                green: "\x1b[32m",
                reset: "\x1b[0m",
            }
        } else {
            Colors {
                cyan: "",
                bold: "",
                dim: "",
                yellow: "",
                green: "",
                reset: "",
            }
        }
    }
}

impl Default for Colors {
    fn default() -> Self {
        Self::new()
    }
}
```

## Workspace integration

Update `Cargo.toml` in repo root:

```toml
[workspace]
resolver = "2"
members = [
  "rust/loopflow-engine",
  "rust/lfd",
  "rust/lf",
]

[workspace.package]
version = "0.7.1"
edition = "2021"
license = "MIT"
```

## Dependencies on loopflow-engine

The CLI needs these exports (all already exist):

| Function | Module | Purpose |
|----------|--------|---------|
| `load_config_or_default` | config | Load merged config |
| `parse_model` | config | Parse "claude:opus" → (backend, variant) |
| `gather_context` | prompt | Assemble context components |
| `format_prompt` | prompt | Format components into prompt string |
| `analyze_tokens` | prompt | Token count breakdown |
| `trim_context` | prompt | Trim to budget |
| `load_step` | flow | Load step definition |
| `load_flow` | flow | Load flow definition |
| `load_direction` | flow | Load direction |
| `launch_agent` | agent | Spawn agent process |
| `check_cli_available` | agent | Check if CLI exists |
| `LaunchConfig` | agent | Agent spawn configuration |

## Migration path

1. Add `rust/lf/` to workspace
2. Implement commands one by one, testing against Python behavior
3. Add integration tests that compare Python and Rust output
4. Once parity achieved, update `pyproject.toml` to bundle Rust `lf` binary
5. Users get Rust binary via `uv tool install loopflow`
6. Python `lf` remains as fallback (`python -m loopflow.lf`)
