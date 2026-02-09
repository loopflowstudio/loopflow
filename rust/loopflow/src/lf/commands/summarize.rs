use crate::engine::config::SummaryConfig;
use crate::engine::prompt::{
    compute_source_hash, count_area_tokens, count_tokens, is_summary_fresh, walk_area_files,
    write_summary,
};
use crate::engine::{
    check_cli_available, launch_agent, load_config_or_default, parse_model, LaunchConfig,
    StreamFormat,
};
use crate::lf::commands::util::find_repo_root;
use anyhow::{anyhow, Result};
use std::fs;
use tracing::debug;

/// Preload threshold: areas with fewer tokens get all content inlined.
const PRELOAD_THRESHOLD: usize = 50_000;

pub fn run(path: Option<&str>, force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let config = load_config_or_default(Some(&repo_root));

    if config.summaries.is_empty() && path.is_none() {
        println!("No summaries configured. Add to .lf/config.yaml:");
        println!("  summaries:");
        println!("    - path: src/engine/");
        println!("      tokens: 5000");
        return Ok(());
    }

    // Ensure .lf/summaries/ is gitignored
    crate::engine::prompt::ensure_gitignore_entry(&repo_root, ".lf/summaries/")?;

    let areas: Vec<SummaryConfig> = if let Some(p) = path {
        let matching = config
            .summaries
            .iter()
            .find(|s| s.path.trim_end_matches('/') == p.trim_end_matches('/'));
        match matching {
            Some(s) => vec![s.clone()],
            None => vec![SummaryConfig {
                path: p.to_string(),
                tokens: Some(config.summary_tokens),
                model: "gemini".to_string(),
            }],
        }
    } else {
        config.summaries.clone()
    };

    for area in &areas {
        let token_budget = area.tokens.unwrap_or(config.summary_tokens);

        if !force {
            match is_summary_fresh(&repo_root, &area.path) {
                Ok(true) => {
                    println!("{} — up to date", area.path);
                    continue;
                }
                Ok(false) => {}
                Err(e) => {
                    debug!(?e, path = area.path, "freshness check failed, regenerating");
                }
            }
        }

        println!("{} — generating...", area.path);

        let source_hash = compute_source_hash(&repo_root, &area.path)?;
        if source_hash.is_empty() {
            println!("{} — path not found, skipping", area.path);
            continue;
        }

        let area_tokens = count_area_tokens(&repo_root, &area.path)?;
        let preload = area_tokens < PRELOAD_THRESHOLD;

        let prompt = build_summarize_prompt(&repo_root, &area.path, token_budget, preload)?;

        let (backend, _) = parse_model(&area.model);
        if !check_cli_available(&backend) {
            return Err(anyhow!(
                "'{}' CLI not found (needed for summarize)",
                backend
            ));
        }

        let launch_config = LaunchConfig {
            auto: true,
            stream: false,
            skip_permissions: true,
            cwd: Some(repo_root.clone()),
            stream_format: StreamFormat::Raw,
            ..Default::default()
        };

        let result = launch_agent(&area.model, &prompt, &launch_config)?;
        if result.exit_code != 0 {
            eprintln!(
                "{} — agent failed (exit {}): {}",
                area.path, result.exit_code, result.stderr
            );
            continue;
        }

        let output = result.stdout;
        let output_tokens = count_tokens(&output);
        write_summary(
            &repo_root,
            &area.path,
            &source_hash,
            output_tokens,
            &area.model,
            &output,
        )?;
        println!("{} — done ({} tokens)", area.path, output_tokens);
    }

    Ok(())
}

fn build_summarize_prompt(
    repo_root: &std::path::Path,
    area_path: &str,
    token_budget: usize,
    preload: bool,
) -> Result<String> {
    let step_template = include_str!("../../engine/builtins/ops/summarize.md");
    let files = walk_area_files(repo_root, area_path);

    if preload {
        let mut content = String::new();
        for file in &files {
            let rel = file
                .strip_prefix(repo_root)
                .unwrap_or(file)
                .to_string_lossy();
            if let Ok(text) = fs::read_to_string(file) {
                content.push_str(&format!("=== {} ===\n{}\n\n", rel, text));
            }
        }
        Ok(step_template
            .replace("{token_budget}", &token_budget.to_string())
            .replace("{content}", &content))
    } else {
        let file_list: Vec<String> = files
            .iter()
            .map(|f| {
                f.strip_prefix(repo_root)
                    .unwrap_or(f)
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        Ok(step_template
            .replace("{token_budget}", &token_budget.to_string())
            .replace(
                "<source>\n{content}\n</source>",
                &format!(
                    "Read the files in these paths and summarize:\n{}",
                    file_list.join("\n")
                ),
            ))
    }
}
