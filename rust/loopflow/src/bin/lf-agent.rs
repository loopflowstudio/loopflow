use clap::Parser;
use loopflow::agent::context::ContextStore;
use loopflow::agent::tools;
use loopflow::agent::turn::{self, TurnConfig, DEFAULT_MAX_ITERATIONS, DEFAULT_TIMEOUT_SECS};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Parser, Debug)]
#[command(
    name = "lf-agent",
    about = "Run a single agent turn with tool dispatch"
)]
struct Args {
    /// The prompt to send to the model
    prompt: String,

    /// Maximum number of tool-call iterations
    #[arg(long, default_value_t = DEFAULT_MAX_ITERATIONS)]
    max_iterations: u32,

    /// Timeout in seconds
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECS)]
    timeout: u64,

    /// System prompt
    #[arg(long)]
    system: Option<String>,

    /// Workspace directory for file tools (defaults to a temporary directory)
    #[arg(long)]
    workspace: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = TurnConfig {
        max_iterations: args.max_iterations,
        timeout: std::time::Duration::from_secs(args.timeout),
        system: args.system,
    };

    let store = Arc::new(Mutex::new(ContextStore::new()));

    // Use provided workspace or create a temporary one
    let _tempdir;
    let workspace = match args.workspace {
        Some(path) => path,
        None => {
            _tempdir = tempfile::tempdir().expect("failed to create temp workspace");
            _tempdir.path().to_path_buf()
        }
    };

    let registry = tools::full_registry(store, workspace);

    match turn::run(&args.prompt, &config, &registry).await {
        Ok(result) => {
            // Emit each event as a JSONL line to stdout
            for event in &result.events {
                if let Ok(json) = serde_json::to_string(event) {
                    println!("{json}");
                }
            }

            eprintln!(
                "[lf-agent] done: {} iterations, {} input tokens, {} output tokens, {} events",
                result.iterations,
                result.input_tokens,
                result.output_tokens,
                result.events.len()
            );
        }
        Err(e) => {
            eprintln!("[lf-agent] error: {e}");
            std::process::exit(1);
        }
    }
}
