//! `lfq` — the exec-door client. Thin verb-mirror of an lfd: `lfq exec <lf
//! argv…>` forwards the argv to a wave's (or the machine's) `/v0/exec` and
//! propagates the `lf` exit code. See [`loopflow::lfq`].

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args
        .get(1)
        .is_some_and(|arg| arg == "--version" || arg == "-V")
    {
        println!("lfq {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    match args.get(1).map(String::as_str) {
        Some("exec") => match loopflow::lfq::run(args[2..].to_vec()).await {
            // Propagate the remote `lf` exit code (clamped to a u8 process code).
            Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
            Err(err) => {
                eprintln!("lfq: {err}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: lfq exec <lf argv…>");
            ExitCode::FAILURE
        }
    }
}
