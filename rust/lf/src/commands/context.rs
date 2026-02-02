use crate::commands::util::find_repo_root;
use crate::Cli;
use anyhow::Result;
use loopflow_engine::{
    analyze_tokens, format_prompt, gather_context, load_config_or_default, trim_context,
    GatherContextOpts,
};

pub fn show(tokens: bool, trim: Option<usize>, cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    let config = load_config_or_default(Some(&repo_root));

    let mut directions = config.direction.clone().unwrap_or_default();
    directions.extend(cli.direction.clone());

    let area = if !cli.area.is_empty() {
        cli.area.first().map(|p| p.to_string_lossy().to_string())
    } else {
        config.area.clone()
    };

    let include_clipboard = cli.clipboard || config.paste;

    let components = gather_context(&GatherContextOpts {
        repo_root: repo_root.clone(),
        step: None,
        inline: None,
        step_args: Vec::new(),
        run_mode: Some("auto".to_string()),
        directions,
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: include_clipboard,
        area,
        wave: cli.wave.clone(),
    })?;

    let components = if let Some(max_tokens) = trim {
        trim_context(components, max_tokens)
    } else {
        components
    };

    if tokens {
        let total = analyze_tokens(&components);
        println!("Tokens: {}", total);
        println!();
    }

    let prompt = format_prompt(&components);
    println!("{}", prompt);

    Ok(())
}
