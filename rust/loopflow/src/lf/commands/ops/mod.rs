use crate::engine::git::{current_branch, delete_local_branch, get_default_branch, is_clean};
use crate::engine::worktrees::{
    create_with_schema, list_worktrees, main_repo_root, worktree_path, worktree_short_name,
};
use crate::lf::commands::util::find_repo_root;
use crate::lf::output::Colors;
use crate::lf::{OpsCommand, ShellCommand, WtCommand};
use crate::ops::{
    abandon_branch, commit_workflow, create_or_update_pr, land, next_branch, rebase_with_recovery,
    release, AbandonOptions, CommitOptions, LandOptions, NextOptions, PrOptions, Progress,
    RebaseOptions,
};
use anyhow::{anyhow, Result};
use std::io::{self, Write};
use std::process::Command;

pub fn run(op: &OpsCommand) -> Result<()> {
    let progress = CliProgress;
    match op {
        OpsCommand::Cp {
            paths,
            exclude,
            lfdocs,
            no_lfdocs,
        } => copy_context(paths, exclude, *lfdocs, *no_lfdocs),
        OpsCommand::Doctor => doctor(),
        OpsCommand::Rebase { onto } => rebase_current(onto.as_deref(), &progress),
        OpsCommand::Push { force } => push_current(*force),
        OpsCommand::Land {
            strict,
            local,
            create_pr,
            worktree,
            no_lint,
        } => land_current(
            *strict,
            *local,
            *create_pr,
            worktree.as_deref(),
            !no_lint,
            &progress,
        ),
        OpsCommand::Pr { refresh, no_lint } => open_pr(*refresh, !no_lint, &progress),
        OpsCommand::Sync => sync_current(),
        OpsCommand::Next {
            create_pr,
            no_rebase,
        } => next_branch_cmd(*create_pr, !*no_rebase, &progress),
        OpsCommand::Commit {
            message,
            push,
            no_add,
            no_lint,
        } => commit_current(message.as_deref(), *push, !no_add, !no_lint, &progress),
        OpsCommand::Abandon { force, branch } => {
            abandon_current(branch.as_deref(), *force, &progress)
        }
        OpsCommand::Wt { cmd } => run_worktree(cmd),
        OpsCommand::Shell { cmd } => run_shell(cmd),
        OpsCommand::Lint => run_check("lint"),
        OpsCommand::Test => run_check("test"),
        OpsCommand::Release { version } => run_release(version, &progress),
    }
}

fn run_check(kind: &str) -> Result<()> {
    let repo_root = find_repo_root()?;
    let config = crate::engine::config::load_config_or_default(Some(&repo_root));
    let cmd = match kind {
        "lint" => config.lint,
        "test" => config.test,
        _ => None,
    };
    let Some(cmd) = cmd else {
        return Err(anyhow!(
            "no `{}` command configured in .lf/config.yaml",
            kind
        ));
    };
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(&repo_root)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{} failed", kind))
    }
}

struct CliProgress;

impl Progress for CliProgress {
    fn status(&self, msg: &str) {
        println!("{}", msg);
    }

    fn error(&self, msg: &str) {
        eprintln!("{}", msg);
    }

    fn confirm(&self, msg: &str) -> bool {
        print!("{} [y/N]: ", msg);
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return false;
        }
        matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
    }
}

fn rebase_current(onto: Option<&str>, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let base = get_default_branch(&repo_root)?;
    let onto_ref = onto
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("origin/{base}"));
    let _ = rebase_with_recovery(
        &repo_root,
        &RebaseOptions {
            onto: onto_ref,
            push: true,
        },
        progress,
    )?;
    Ok(())
}

fn push_current(force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    crate::engine::git::push(&repo_root, force).map_err(Into::into)
}

fn land_current(
    strict: bool,
    local: bool,
    create_pr: bool,
    worktree: Option<&str>,
    lint: bool,
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let _ = land(
        &repo_root,
        &LandOptions {
            strict,
            local,
            create_pr,
            worktree: worktree.map(str::to_string),
            lint,
        },
        progress,
    )?;
    Ok(())
}

fn open_pr(refresh: bool, lint: bool, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let result = create_or_update_pr(&repo_root, &PrOptions { refresh, lint }, progress)?;
    println!("{}", result.url);
    Ok(())
}

fn sync_current() -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let ok = crate::engine::git::sync_main(&repo_root, &main_branch)?;
    if !ok {
        return Err(anyhow!("working tree dirty; sync aborted"));
    }
    Ok(())
}

fn next_branch_cmd(create_pr: bool, rebase: bool, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let result = next_branch(
        &repo_root,
        &NextOptions {
            create_pr,
            rebase,
            wave_name: None,
        },
        progress,
    )?;
    println!("{}", result.new_branch);
    Ok(())
}

fn commit_current(
    message: Option<&str>,
    push: bool,
    add: bool,
    lint: bool,
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let _ = commit_workflow(
        &repo_root,
        &CommitOptions {
            add,
            lint,
            push,
            create_draft_pr: true,
            task: "commit".to_string(),
            flow_parents: Vec::new(),
            message: message.map(str::to_string),
        },
        progress,
    )?;
    Ok(())
}

fn abandon_current(branch: Option<&str>, force: bool, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    abandon_branch(
        &repo_root,
        &AbandonOptions {
            branch: branch.map(str::to_string),
            force,
        },
        progress,
    )?;
    Ok(())
}

fn run_release(version: &str, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let url = release(&repo_root, version, progress)?;
    if !url.is_empty() {
        println!("{}", url);
    }
    Ok(())
}

fn run_worktree(cmd: &WtCommand) -> Result<()> {
    match cmd {
        WtCommand::Create { name, base, stack } => wt_create(name, base.as_deref(), *stack),
        WtCommand::Switch { name } => wt_switch(name),
        WtCommand::List { format, .. } => wt_list(format.as_deref()),
        WtCommand::Remove { name, force } => wt_remove(name, *force),
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

    let config = crate::engine::config::load_config(Some(&main_repo))
        .ok()
        .flatten();
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

    let c = Colors::new();
    let default_branch = get_default_branch(&main_repo)?;

    // Collect display info for all worktrees
    struct Row {
        name: String,
        is_current: bool,
        is_main: bool,
        merged: bool,
        dirty: bool,
        diff_stat: String,
    }

    let rows: Vec<Row> = worktrees
        .iter()
        .map(|wt| {
            let is_main = wt.branch.as_deref() == Some(&default_branch);
            let name = if is_main {
                default_branch.clone()
            } else {
                worktree_short_name(&wt.path).unwrap_or_else(|| {
                    wt.path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "?".to_string())
                })
            };
            let is_current = wt.path == repo_root;
            let dirty = !is_clean(&wt.path).unwrap_or(true);
            let diff_stat = if is_main {
                String::new()
            } else {
                wt_diff_stat(&main_repo, wt.branch.as_deref(), &default_branch)
            };
            Row {
                name,
                is_current,
                is_main,
                merged: wt.merged,
                dirty,
                diff_stat,
            }
        })
        .collect();

    let max_name = rows.iter().map(|r| r.name.len()).max().unwrap_or(0);

    for row in &rows {
        let marker = if row.is_current { "*" } else { " " };

        let fresh = !row.is_main && !row.merged && row.diff_stat.is_empty() && !row.dirty;
        let name_color = if row.is_main || row.merged || fresh {
            c.dim
        } else {
            c.bold
        };

        let status = if row.merged {
            format!("{}merged{}", c.green, c.reset)
        } else if fresh {
            format!("{}fresh{}", c.dim, c.reset)
        } else {
            format!("{}active{}", c.cyan, c.reset)
        };

        let dirty_flag = if row.dirty {
            format!(" {}dirty{}", c.yellow, c.reset)
        } else {
            String::new()
        };

        let diff = if row.diff_stat.is_empty() {
            String::new()
        } else {
            format!("  {}{}{}", c.dim, row.diff_stat, c.reset)
        };

        println!(
            "{marker} {name_color}{:<width$}{reset}  {status}{dirty_flag}{diff}",
            row.name,
            width = max_name,
            marker = marker,
            name_color = name_color,
            reset = c.reset,
            status = status,
            dirty_flag = dirty_flag,
            diff = diff,
        );
    }
    Ok(())
}

fn wt_remove(name: &str, force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;

    // Find the worktree by short name or directory name
    let worktrees = list_worktrees(&main_repo)?;
    let target = worktrees.iter().find(|wt| {
        worktree_short_name(&wt.path).as_deref() == Some(name)
            || wt
                .path
                .file_name()
                .map(|n| n.to_string_lossy() == name)
                .unwrap_or(false)
    });

    let wt = match target {
        Some(wt) => wt,
        None => return Err(anyhow!("no worktree found for '{}'", name)),
    };

    if wt.path == repo_root {
        return Err(anyhow!("cannot remove the current worktree"));
    }

    let default_branch = get_default_branch(&main_repo)?;
    if wt.branch.as_deref() == Some(&default_branch) {
        return Err(anyhow!("cannot remove the main worktree"));
    }

    if !force && !is_clean(&wt.path).unwrap_or(false) {
        return Err(anyhow!(
            "worktree has uncommitted changes (use --force to override)"
        ));
    }

    let branch = wt.branch.clone();
    crate::engine::git::worktree_remove(&main_repo, &wt.path)?;
    if let Some(branch) = branch {
        let _ = delete_local_branch(&main_repo, &branch);
    }
    println!("Removed {}", name);
    Ok(())
}

/// Get a compact diff stat for a branch vs default branch.
fn wt_diff_stat(repo: &std::path::Path, branch: Option<&str>, default_branch: &str) -> String {
    let branch = match branch {
        Some(b) => b,
        None => return String::new(),
    };
    let target = format!("origin/{default_branch}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["diff", "--shortstat", &format!("{target}...{branch}")])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            // "3 files changed, 10 insertions(+), 5 deletions(-)" → "+10 -5 (3 files)"
            parse_shortstat(&raw)
        }
        _ => String::new(),
    }
}

fn parse_shortstat(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut files = "";
    let mut insertions = "";
    let mut deletions = "";
    for part in raw.split(", ") {
        let part = part.trim();
        if part.contains("file") {
            files = part.split_whitespace().next().unwrap_or("0");
        } else if part.contains("insertion") {
            insertions = part.split_whitespace().next().unwrap_or("0");
        } else if part.contains("deletion") {
            deletions = part.split_whitespace().next().unwrap_or("0");
        }
    }
    let ins = if insertions.is_empty() {
        "0"
    } else {
        insertions
    };
    let del = if deletions.is_empty() { "0" } else { deletions };
    format!("+{ins} -{del} ({files} files)")
}

fn wt_prune(dry_run: bool, force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let current_path = repo_root;

    let default_branch = get_default_branch(&main_repo)?;
    let _ = crate::engine::git::fetch(&main_repo, "origin", &default_branch);

    let worktrees = list_worktrees(&main_repo)?;
    let mut prunable = worktrees
        .into_iter()
        .filter(|wt| wt.prunable)
        .filter(|wt| wt.path != current_path)
        .filter(|wt| is_clean(&wt.path).unwrap_or(false))
        .collect::<Vec<_>>();

    if prunable.is_empty() {
        println!("No prunable worktrees.");
        return Ok(());
    }

    if force {
        let default_branch = get_default_branch(&main_repo)?;
        for wt in prunable.drain(..) {
            crate::engine::git::worktree_remove(&main_repo, &wt.path)?;
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
#
# Enables directory switching after `lf ops wt create`.

if command -v lf >/dev/null 2>&1; then
    lf() {
        local directive_file exit_code=0
        directive_file="$(mktemp)"

        LOOPFLOW_DIRECTIVE_FILE="$directive_file" command lf "$@" || exit_code=$?

        if [[ -s "$directive_file" ]]; then
            source "$directive_file"
            if [[ $exit_code -eq 0 ]]; then
                exit_code=$?
            fi
        fi

        rm -f "$directive_file"
        return "$exit_code"
    }
fi
"#;

const SHELL_INIT_BASH: &str = r#"# loopflow shell integration for bash
#
# Enables directory switching after `lf ops wt create`.

if command -v lf >/dev/null 2>&1; then
    lf() {
        local directive_file exit_code=0
        directive_file="$(mktemp)"

        LOOPFLOW_DIRECTIVE_FILE="$directive_file" command lf "$@" || exit_code=$?

        if [[ -s "$directive_file" ]]; then
            source "$directive_file"
            if [[ $exit_code -eq 0 ]]; then
                exit_code=$?
            fi
        fi

        rm -f "$directive_file"
        return "$exit_code"
    }
fi
"#;

const SHELL_INSTALL_LINE_ZSH: &str =
    "if command -v lf >/dev/null 2>&1; then eval \"$(command lf ops shell init zsh)\"; fi";
const SHELL_INSTALL_LINE_BASH: &str =
    "if command -v lf >/dev/null 2>&1; then eval \"$(command lf ops shell init bash)\"; fi";

// ==========================================================================
// lf ops cp
// ==========================================================================

fn copy_context(paths: &[String], exclude: &[String], lfdocs: bool, no_lfdocs: bool) -> Result<()> {
    use crate::engine::prompt::{
        count_tokens, default_gather_sources, gather_context, Document, GatherContextOpts,
    };
    use std::collections::HashSet;

    let repo_root = find_repo_root()?;

    // When paths are given, skip lfdocs unless --lfdocs is explicit.
    // When no paths, include lfdocs by default unless --no-lfdocs.
    let has_paths = !paths.is_empty();
    let include_lfdocs = if has_paths { lfdocs } else { !no_lfdocs };

    // Gather context
    let opts = GatherContextOpts {
        repo_root: repo_root.clone(),
        files: paths.to_vec(),
        sources: default_gather_sources(
            include_lfdocs,
            !has_paths, // Use diff files if no paths specified
            false,
        ),
        ..Default::default()
    };

    let components = gather_context(&opts)?;

    // Collect all documents to format
    let mut all_docs: Vec<Document> = Vec::new();
    all_docs.extend(components.diff_files);
    all_docs.extend(components.docs);

    // Apply exclusion patterns
    if !exclude.is_empty() {
        let exclude_set: HashSet<&str> = exclude.iter().map(|s| s.as_str()).collect();
        all_docs.retain(|doc| !exclude_set.iter().any(|pattern| doc.path.contains(pattern)));
    }

    if all_docs.is_empty() {
        println!("No files to copy.");
        return Ok(());
    }

    // Format files as raw content (similar to Python's format_files_raw)
    let mut output = String::new();
    for doc in &all_docs {
        output.push_str(&format!("=== {} ===\n", doc.path));
        output.push_str(&doc.content);
        if !doc.content.ends_with('\n') {
            output.push('\n');
        }
        output.push('\n');
    }

    // Copy to clipboard
    copy_to_clipboard(&output)?;

    // Display token tree
    let mut total_tokens = 0;
    for doc in &all_docs {
        let tokens = count_tokens(&doc.content);
        total_tokens += tokens;
        println!("{:>6} tokens  {}", tokens, doc.path);
    }
    println!("─────────────");
    println!("{:>6} tokens  total", total_tokens);
    println!("\nCopied to clipboard.");

    Ok(())
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    crate::engine::clipboard::write(text)?;
    Ok(())
}

// ==========================================================================
// lf ops doctor
// ==========================================================================

fn doctor() -> Result<()> {
    use std::path::Path;

    let repo_root = find_repo_root().ok();

    // Repo status
    if let Some(ref root) = repo_root {
        let lf_dir = root.join(".lf");
        if lf_dir.join("steps").is_dir() || lf_dir.join("flows").is_dir() {
            println!("✓ task files found");
        } else {
            println!("- no task files (run: lf init)");
        }
    } else {
        println!("- not in a git repo");
    }

    let is_macos = cfg!(target_os = "macos");

    // Optional: npm
    if which("npm") {
        println!("✓ npm");
    } else if is_macos {
        println!("- npm: brew install node");
    } else {
        println!("- npm: https://nodejs.org/");
    }

    // Optional: coding agents
    if check_claude_available() {
        println!("✓ claude");
    } else {
        println!("- claude: lf init");
    }

    if check_codex_available() {
        println!("✓ codex");
    } else {
        println!("- codex: npm install -g @openai/codex");
    }

    if check_gemini_available() {
        println!("✓ gemini");
    } else {
        println!("- gemini: npm install -g @google/gemini-cli");
    }

    // Optional: IDE/terminals (macOS-only apps)
    if is_macos {
        if which("warp") {
            println!("✓ warp");
        } else {
            println!("- warp: brew install --cask warp");
        }

        if which("cursor") {
            println!("✓ cursor");
        } else {
            println!("- cursor: brew install --cask cursor");
        }
    }

    // Optional: superpowers
    let superpowers_path = dirs::home_dir()
        .map(|h| h.join(".superpowers"))
        .unwrap_or_else(|| Path::new("~/.superpowers").to_path_buf());
    if superpowers_path.exists() {
        println!("✓ superpowers");
    } else {
        println!("- superpowers: git clone https://github.com/obra/superpowers ~/.superpowers");
    }

    // Optional: gh for PR creation
    if which("gh") {
        println!("✓ gh");
    } else if is_macos {
        println!("- gh: brew install gh");
    } else {
        println!("- gh: https://cli.github.com/");
    }

    Ok(())
}

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn check_claude_available() -> bool {
    // Check for claude CLI
    which("claude")
}

fn check_codex_available() -> bool {
    // Check for codex CLI
    which("codex")
}

fn check_gemini_available() -> bool {
    // Check for gemini CLI
    which("gemini")
}
