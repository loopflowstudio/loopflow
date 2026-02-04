use std::path::Path;
use std::process::Command;

use loopflow_engine::git::{fetch, rebase};
use loopflow_engine::prompt::{format_prompt, gather_context, GatherContextOpts};
use loopflow_engine::{launch_agent, load_config_or_default, LaunchConfig};

use crate::error::{OpsError, OpsResult};
use crate::progress::Progress;

#[derive(Debug, Clone)]
pub struct RebaseOptions {
    pub onto: String,
    pub push: bool,
}

#[derive(Debug, Clone)]
pub struct RebaseResult {
    pub success: bool,
}

pub fn rebase_with_recovery(
    repo: &Path,
    options: &RebaseOptions,
    progress: &impl Progress,
) -> OpsResult<RebaseResult> {
    if options.onto.starts_with("origin/") {
        if let Some(branch) = options.onto.strip_prefix("origin/") {
            let _ = fetch(repo, "origin", branch);
        }
    }

    progress.status(&format!("Rebasing onto {}...", options.onto));
    let result = rebase(repo, &options.onto, None)?;
    if result.success {
        if options.push {
            crate::commit::push_with_upstream_or_error(repo)?;
        }
        return Ok(RebaseResult { success: true });
    }

    if let Some(conflicts) = result.conflicts {
        let conflict_list = conflicts
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        progress.error("Rebase conflicts detected");
        run_rebase_agent(repo, &conflict_list, progress)?;

        progress.status(&format!("Retrying rebase onto {}...", options.onto));
        let retry = rebase(repo, &options.onto, None)?;
        if retry.success {
            if options.push {
                crate::commit::push_with_upstream_or_error(repo)?;
            }
            return Ok(RebaseResult { success: true });
        }
    }

    Err(OpsError::Message("rebase failed".to_string()))
}

fn run_rebase_agent(repo: &Path, conflicts: &[String], progress: &impl Progress) -> OpsResult<()> {
    let config = load_config_or_default(Some(repo));
    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: None,
        inline: None,
        step_args: Vec::new(),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: config.direction.unwrap_or_default(),
        files: Vec::new(),
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: config.paste,
        area: config.area,
        wave: None,
    };
    let components = gather_context(&opts)?;
    let base_prompt = format_prompt(&components);
    let task = format!(
        "Resolve git rebase conflicts.\n\nConflicts:\n{}\n\nFix conflicts and ensure the rebase can be retried.",
        conflicts.join("\n")
    );
    let prompt = format!("{}\n\n<lf:task>\n{}\n</lf:task>", base_prompt, task);

    let launch_config = LaunchConfig {
        auto: true,
        stream: false,
        skip_permissions: true,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(repo.to_path_buf()),
        context_file: None,
    };

    progress.status("Launching rebase assistant...");
    let result = launch_agent(&config.agent_model, &prompt, &launch_config)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }

    let _ = Command::new("git").arg("status").current_dir(repo).status();

    Ok(())
}
