use crate::commands::util::find_repo_root;
use crate::{OpsCommand, ShellCommand, WtCommand};
use anyhow::{anyhow, Result};
use loopflow_engine::git::{
    commit, current_branch, delete_local_branch, get_default_branch, land, pr_create_draft, push,
    push_with_upstream, rebase, sync_main, LandStrategy,
};
use loopflow_engine::worktrees::{
    create_with_schema, list_worktrees, main_repo_root, preserve_worktree, worktree_path,
};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run(op: &OpsCommand) -> Result<()> {
    match op {
        OpsCommand::Rebase { onto } => rebase_current(onto.as_deref()),
        OpsCommand::Push { force } => push_current(*force),
        OpsCommand::Land { strategy } => land_current(strategy.as_deref()),
        OpsCommand::Pr { title, draft } => open_pr(title.as_deref(), *draft),
        OpsCommand::Sync => sync_current(),
        OpsCommand::Next => next_branch(),
        OpsCommand::Commit { message } => commit_current(message.as_deref()),
        OpsCommand::Abandon { force } => abandon_current(*force),
        OpsCommand::Wt { cmd } => run_worktree(cmd),
        OpsCommand::Shell { cmd } => run_shell(cmd),
    }
}

fn rebase_current(onto: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let base = get_default_branch(&repo_root)?;
    let onto_ref = onto
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("origin/{base}"));
    let result = rebase(&repo_root, &onto_ref, None)?;
    if !result.success {
        return Err(anyhow!("rebase failed"));
    }
    Ok(())
}

fn push_current(force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    push(&repo_root, force).map_err(Into::into)
}

fn land_current(strategy: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let land_strategy = match strategy {
        Some("local") | Some("merge") => LandStrategy::LocalMerge,
        Some("squash") | Some("squash_merge") => LandStrategy::SquashMerge,
        _ => LandStrategy::SquashMerge,
    };
    let _ = land(&repo_root, land_strategy, &main_branch)?;
    Ok(())
}

fn open_pr(title: Option<&str>, draft: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    if draft {
        let url = pr_create_draft(&repo_root)?;
        println!("{}", url);
        return Ok(());
    }

    let mut cmd = Command::new("gh");
    cmd.arg("pr").arg("create").arg("--fill");
    if let Some(title) = title {
        cmd.arg("--title").arg(title);
    }

    let status = cmd.current_dir(&repo_root).status()?;
    if !status.success() {
        return Err(anyhow!("gh pr create failed"));
    }
    Ok(())
}

fn sync_current() -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let ok = sync_main(&repo_root, &main_branch)?;
    if !ok {
        return Err(anyhow!("working tree dirty; sync aborted"));
    }
    Ok(())
}

fn next_branch() -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let main_branch = get_default_branch(&main_repo)?;

    let current = current_branch(&repo_root)?.ok_or_else(|| anyhow!("not on a branch"))?;
    if current == main_branch {
        return Err(anyhow!("cannot run next from {}", main_branch));
    }

    let preserved = preserve_worktree(&main_repo, &repo_root)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let next_name = format!("next-{}", timestamp);

    let status = Command::new("git")
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(&next_name)
        .arg(&repo_root)
        .arg(&main_branch)
        .current_dir(&main_repo)
        .status()?;
    if !status.success() {
        return Err(anyhow!("failed to create worktree {}", repo_root.display()));
    }

    let _ = push_with_upstream(&repo_root, "origin", &next_name);
    let _ = write_shell_directive(&format!("cd {}", repo_root.display()));
    println!("Preserved worktree: {}", preserved.display());
    Ok(())
}

fn commit_current(message: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let message = message.ok_or_else(|| anyhow!("commit message required"))?;
    commit(&repo_root, message).map_err(Into::into)
}

fn abandon_current(force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let branch = current_branch(&repo_root)?;

    if branch.as_deref() == Some(&main_branch) {
        println!("Already on {}", main_branch);
        return Ok(());
    }

    let status = Command::new("git")
        .arg("checkout")
        .arg(&main_branch)
        .current_dir(&repo_root)
        .status()?;
    if !status.success() {
        return Err(anyhow!("failed to checkout {}", main_branch));
    }

    if let Some(branch) = branch {
        if force {
            delete_local_branch(&repo_root, &branch)?;
        } else {
            let status = Command::new("git")
                .arg("branch")
                .arg("-d")
                .arg(&branch)
                .current_dir(&repo_root)
                .status()?;
            if !status.success() {
                return Err(anyhow!("failed to delete branch {}", branch));
            }
        }
    }

    Ok(())
}

fn run_worktree(cmd: &WtCommand) -> Result<()> {
    match cmd {
        WtCommand::Create { name, base, stack } => wt_create(name, base.as_deref(), *stack),
        WtCommand::Switch { name } => wt_switch(name),
        WtCommand::List { format, .. } => wt_list(format.as_deref()),
        WtCommand::Prune { dry_run, force, .. } => wt_prune(*dry_run, *force),
        WtCommand::Ci { watch, logs } => wt_ci(*watch, *logs),
    }
}

fn wt_create(name: &str, base: Option<&str>, stack: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;

    let mut base_branch = base.map(str::to_string);
    if stack {
        let current = current_branch(&repo_root)?.ok_or_else(|| anyhow!("not on a branch"))?;
        if current == "main" || current == "master" {
            return Err(anyhow!("cannot stack on main/master"));
        }
        base_branch = Some(current);
    }

    let config = loopflow_engine::config::load_config(&main_repo).ok();
    let branch_config = config.as_ref().and_then(|c| c.branch_names.as_ref());
    let result = create_with_schema(&main_repo, name, base_branch.as_deref(), branch_config)?;

    println!("Created worktree: {}", result.path.display());
    if result.branch != name {
        println!("Branch: {}", result.branch);
    }
    if let Some(base_branch) = result.base_branch {
        println!("Base: {}", base_branch);
    }

    if !write_shell_directive(&format!("cd {}", result.path.display()))? {
        println!("cd {}", result.path.display());
        println!("Tip: Run 'lf ops shell install' for auto-cd");
    }

    Ok(())
}

fn wt_switch(name: &str) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;

    let target = worktree_path(&main_repo, name);
    let path = if target.exists() {
        target
    } else {
        let worktrees = list_worktrees(&main_repo)?;
        let mut matches = worktrees
            .into_iter()
            .filter(|wt| {
                wt.path
                    .file_name()
                    .map(|n| n.to_string_lossy() == name)
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            matches.remove(0).path
        } else if matches.is_empty() {
            return Err(anyhow!("no worktree found for '{}'", name));
        } else {
            return Err(anyhow!("multiple worktrees match '{}'", name));
        }
    };

    if !write_shell_directive(&format!("cd {}", path.display()))? {
        println!("cd {}", path.display());
    }
    Ok(())
}

fn wt_list(format: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let worktrees = list_worktrees(&main_repo)?;

    if matches!(format, Some("json")) {
        let json = serde_json::to_string_pretty(&worktrees)?;
        println!("{}", json);
        return Ok(());
    }

    for wt in worktrees {
        let branch = wt.branch.unwrap_or_else(|| "detached".to_string());
        let merged = if wt.merged { "merged" } else { "active" };
        println!("{}  {}  {}", branch, wt.path.display(), merged);
    }
    Ok(())
}

fn wt_prune(dry_run: bool, force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let current_path = repo_root;

    let worktrees = list_worktrees(&main_repo)?;
    let mut prunable = worktrees
        .into_iter()
        .filter(|wt| wt.prunable)
        .filter(|wt| wt.path != current_path)
        .collect::<Vec<_>>();

    if prunable.is_empty() {
        println!("No prunable worktrees.");
        return Ok(());
    }

    if force {
        let default_branch = get_default_branch(&main_repo)?;
        for wt in prunable.drain(..) {
            loopflow_engine::git::worktree_remove(&main_repo, &wt.path)?;
            if let Some(branch) = wt.branch {
                if branch != default_branch {
                    let _ = delete_local_branch(&main_repo, &branch);
                }
            }
            println!("Removed {}", wt.path.display());
        }
        return Ok(());
    }

    if !dry_run {
        println!("Run with --force to remove prunable worktrees.");
    }
    println!("Prunable worktrees:");
    for wt in prunable {
        let branch = wt.branch.unwrap_or_else(|| "detached".to_string());
        println!("{}  {}", branch, wt.path.display());
    }
    Ok(())
}

fn wt_ci(watch: bool, logs: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let branch = current_branch(&repo_root)?.ok_or_else(|| anyhow!("not on a branch"))?;

    let mut args = vec!["pr", "checks", &branch];
    if watch {
        args.push("--watch");
    }

    let status = Command::new("gh")
        .args(&args)
        .current_dir(&repo_root)
        .status()?;

    if !status.success() && logs {
        println!("\n--- Failed check logs ---\n");
        let output = Command::new("gh")
            .args([
                "pr",
                "view",
                &branch,
                "--json",
                "statusCheckRollup",
                "-q",
                ".statusCheckRollup[] | select(.conclusion == \"FAILURE\" or .conclusion == \"failure\") | .detailsUrl",
            ])
            .current_dir(&repo_root)
            .output()?;
        if output.status.success() {
            let urls = String::from_utf8_lossy(&output.stdout);
            for url in urls.lines().filter(|line| !line.trim().is_empty()) {
                let _ = Command::new("gh")
                    .args(["run", "view", url])
                    .current_dir(&repo_root)
                    .status();
            }
        }
    }

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("ci checks failed"))
    }
}

fn run_shell(cmd: &ShellCommand) -> Result<()> {
    match cmd {
        ShellCommand::Init { shell } => shell_init(shell.as_deref()),
        ShellCommand::Install { shell } => shell_install(shell.as_deref()),
        ShellCommand::Directive { command } => shell_directive(command),
    }
}

fn shell_init(shell: Option<&str>) -> Result<()> {
    let shell = shell.unwrap_or("zsh");
    let init = match shell {
        "zsh" => SHELL_INIT_ZSH,
        "bash" => SHELL_INIT_BASH,
        _ => return Err(anyhow!("unsupported shell: {}", shell)),
    };
    println!("{}", init);
    Ok(())
}

fn shell_install(shell: Option<&str>) -> Result<()> {
    let shell = shell
        .map(|value| value.to_string())
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "zsh".to_string());
    let shell_name = if shell.contains("bash") {
        "bash"
    } else {
        "zsh"
    };
    let (config_path, install_line) = match shell_name {
        "bash" => (
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".bashrc"),
            SHELL_INSTALL_LINE_BASH,
        ),
        _ => (
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".zshrc"),
            SHELL_INSTALL_LINE_ZSH,
        ),
    };

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if content.contains("lf ops shell init") {
            println!("Already installed in {}", config_path.display());
            return Ok(());
        }
    }

    std::fs::create_dir_all(
        config_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(".")),
    )?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config_path)?;
    use std::io::Write;
    writeln!(file, "\n{}", install_line)?;
    println!("Installed to {}", config_path.display());
    println!(
        "Restart your shell or run: source {}",
        config_path.display()
    );
    Ok(())
}

fn shell_directive(command: &[String]) -> Result<()> {
    if command.is_empty() {
        return Err(anyhow!("command required"));
    }
    let line = command.join(" ");
    if !write_shell_directive(&line)? {
        println!("{}", line);
    }
    Ok(())
}

fn write_shell_directive(command: &str) -> Result<bool> {
    let directive = std::env::var("LOOPFLOW_DIRECTIVE_FILE").ok();
    let Some(path) = directive else {
        return Ok(false);
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    use std::io::Write;
    writeln!(file, "{}", command)?;
    Ok(true)
}

const SHELL_INIT_ZSH: &str = r#"# loopflow shell integration for zsh
+#
+# Enables directory switching after `lf ops wt create`.
+
+if command -v lf >/dev/null 2>&1; then
+    lf() {
+        local directive_file exit_code=0
+        directive_file="$(mktemp)"
+
+        LOOPFLOW_DIRECTIVE_FILE="$directive_file" command lf "$@" || exit_code=$?
+
+        if [[ -s "$directive_file" ]]; then
+            source "$directive_file"
+            if [[ $exit_code -eq 0 ]]; then
+                exit_code=$?
+            fi
+        fi
+
+        rm -f "$directive_file"
+        return "$exit_code"
+    }
+fi
+"#;

const SHELL_INIT_BASH: &str = r#"# loopflow shell integration for bash
+#
+# Enables directory switching after `lf ops wt create`.
+
+if command -v lf >/dev/null 2>&1; then
+    lf() {
+        local directive_file exit_code=0
+        directive_file="$(mktemp)"
+
+        LOOPFLOW_DIRECTIVE_FILE="$directive_file" command lf "$@" || exit_code=$?
+
+        if [[ -s "$directive_file" ]]; then
+            source "$directive_file"
+            if [[ $exit_code -eq 0 ]]; then
+                exit_code=$?
+            fi
+        fi
+
+        rm -f "$directive_file"
+        return "$exit_code"
+    }
+fi
+"#;

const SHELL_INSTALL_LINE_ZSH: &str =
    "if command -v lf >/dev/null 2>&1; then eval \"$(command lf ops shell init zsh)\"; fi";
const SHELL_INSTALL_LINE_BASH: &str =
    "if command -v lf >/dev/null 2>&1; then eval \"$(command lf ops shell init bash)\"; fi";
