use anyhow::Result;
use clap::Parser;
use loopflow::agent::turn::{
    self, TurnConfig, TurnError, DEFAULT_MAX_ITERATIONS, DEFAULT_TIMEOUT_SECS,
};

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config = TurnConfig {
        max_iterations: args.max_iterations,
        timeout: std::time::Duration::from_secs(args.timeout),
        system: args.system,
    };

    match turn::run(&args.prompt, &config).await {
        Ok(result) => {
            println!("{}", result.response);
            eprintln!(
                "[lf-agent] done: {} iterations, {} input tokens, {} output tokens",
                result.iterations, result.input_tokens, result.output_tokens
            );
        }
        Err(TurnError::MaxIterations(n)) => {
            eprintln!("[lf-agent] error: max iterations ({n}) exceeded");
            std::process::exit(1);
        }
        Err(TurnError::Timeout(d)) => {
            eprintln!("[lf-agent] error: timeout ({d:?}) exceeded");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("[lf-agent] error: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
