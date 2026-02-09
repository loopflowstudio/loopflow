use std::path::Path;

use crate::engine::builtins::get_builtin_step;
use crate::engine::git::{fetch, rebase};
use crate::engine::prompt::{format_prompt, gather_context, GatherContextOpts};
use crate::engine::{launch_agent, load_config_or_default, LaunchConfig};

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;

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

    // Try auto-rebase first. rebase() aborts on conflict and returns the conflict list.
    progress.status(&format!("Rebasing onto {}...", options.onto));
    let result = rebase(repo, &options.onto, None)?;
    if result.success {
        if options.push {
            crate::ops::commit::push_with_upstream_or_error(repo)?;
        }
        return Ok(RebaseResult { success: true });
    }

    // Auto-rebase failed (and was aborted). Launch the rebase agent to handle
    // the full workflow: re-attempt the rebase, resolve conflicts, continue.
    progress.status("Auto-rebase failed, launching rebase assistant...");
    run_rebase_agent(repo, &options.onto, progress)?;

    if options.push {
        crate::ops::commit::push_with_upstream_or_error(repo)?;
    }
    Ok(RebaseResult { success: true })
}

fn run_rebase_agent(repo: &Path, onto: &str, progress: &impl Progress) -> OpsResult<()> {
    let config = load_config_or_default(Some(repo));

    let step_content = get_builtin_step("rebase")
        .expect("built-in rebase step must exist");

    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: None,
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
    let prompt = format!(
        "{}\n\n<lf:step>\n{}\n</lf:step>\n\nRebase onto: {}\n",
        base_prompt, step_content, onto
    );

    let launch_config = LaunchConfig {
        auto: true,
        stream: false,
        skip_permissions: true,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(repo.to_path_buf()),
        context_file: None,
        ..Default::default()
    };

    progress.status("Launching rebase assistant...");
    let result = launch_agent(&config.agent_model, &prompt, &launch_config)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }

    Ok(())
}
