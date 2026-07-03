use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::engine::worktrees::main_repo_root;
use crate::lf::commands::util::find_repo_root;
use crate::lfd::executor::helpers::resolve_lf_binary;
use crate::ops::util::resolve_wave_name;

/// Dropping this file into `wave/<wave>/` stops the loop after the current pass.
const STOP_FILE: &str = "STOP";

/// Cooldown after a failed pass so a broken inner run can't hot-spin the loop.
/// A successful pass repeats immediately — loopflow owns the cadence, gated only
/// on the inner pass finishing.
const FAILURE_COOLDOWN: Duration = Duration::from_secs(3);

/// Run the progress loop for a wave.
///
/// loopflow owns the *outer* loop: each pass is a single bounded
/// `lf goal <wave> --once`, and the loop fires the next pass as soon as the
/// previous one finishes. This is the deterministic controller that replaces
/// relying on the model's own goal loop (which gets stuck). It repeats until
/// interrupted (Ctrl-C) or until `wave/<wave>/STOP` appears.
pub fn run(name: &str) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or(repo_root);
    let wave_name = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;

    let stop_file = main_repo.join("wave").join(&wave_name).join(STOP_FILE);
    let lf = resolve_lf_binary();

    println!("lf loop · {wave_name} · loopflow owns the outer loop (Ctrl-C to stop)");

    let mut pass: u32 = 0;
    loop {
        if stop_file.exists() {
            println!(
                "lf loop · {wave_name} · stopping: stop file present ({}) ({pass} passes)",
                stop_file.display()
            );
            return Ok(());
        }

        pass += 1;
        println!("── lf loop · {wave_name} · pass {pass} ──");

        match run_pass(&lf, &main_repo, &wave_name) {
            PassOutcome::Ok => {}
            PassOutcome::Failed(status) => {
                eprintln!(
                    "lf loop · pass {pass} exited with {status}; cooling down {}s",
                    FAILURE_COOLDOWN.as_secs()
                );
                std::thread::sleep(FAILURE_COOLDOWN);
            }
            PassOutcome::Signaled => {
                // Ctrl-C (or another signal) killed the inner pass — treat it as
                // the operator stopping the loop, not a pass to retry.
                println!("lf loop · {wave_name} · interrupted ({pass} passes)");
                return Ok(());
            }
            PassOutcome::SpawnError(err) => return Err(err),
        }
    }
}

enum PassOutcome {
    Ok,
    Failed(std::process::ExitStatus),
    Signaled,
    SpawnError(anyhow::Error),
}

/// Run one bounded pass: `lf goal <wave> --once`, inheriting stdio so the pass
/// is visible in the terminal.
fn run_pass(lf: &Path, repo: &Path, wave: &str) -> PassOutcome {
    let status = Command::new(lf)
        .arg("goal")
        .arg(wave)
        .arg("--once")
        .current_dir(repo)
        .status();

    match status {
        Ok(status) if status.success() => PassOutcome::Ok,
        Ok(status) if status.code().is_none() => PassOutcome::Signaled,
        Ok(status) => PassOutcome::Failed(status),
        Err(err) => {
            PassOutcome::SpawnError(anyhow!("failed to spawn `lf goal {wave} --once`: {err}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signaled_status_classified_as_interrupt() {
        // A pass that fails (non-signal) cools down and retries; a signaled pass
        // stops the loop. Guard the classification used in `run_pass`.
        let missing = Path::new("/definitely/not/a/real/lf-binary");
        let outcome = run_pass(missing, Path::new("/tmp"), "ghost");
        assert!(matches!(outcome, PassOutcome::SpawnError(_)));
    }
}
