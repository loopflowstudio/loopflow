use crate::commands::util::find_repo_root;
use crate::output::Colors;
use anyhow::{anyhow, Result};
use loopflow_engine::config::{load_config, load_config_or_default};
use std::fs;

pub fn show(global: bool, repo: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home directory"))?;
    let colors = Colors::default();

    let global_path = home.join(".lf/config.yaml");
    let repo_path = repo_root.join(".lf/config.yaml");

    // If --global or --repo specified, show raw file contents
    if global || repo {
        if global {
            println!(
                "{dim}# Global config: {}{reset}",
                global_path.display(),
                dim = colors.dim,
                reset = colors.reset
            );
            if global_path.exists() {
                print!("{}", fs::read_to_string(&global_path)?);
            } else {
                println!(
                    "{dim}(not found){reset}",
                    dim = colors.dim,
                    reset = colors.reset
                );
            }
            println!();
        }
        if repo {
            println!(
                "{dim}# Repo config: {}{reset}",
                repo_path.display(),
                dim = colors.dim,
                reset = colors.reset
            );
            if repo_path.exists() {
                print!("{}", fs::read_to_string(&repo_path)?);
            } else {
                println!(
                    "{dim}(not found){reset}",
                    dim = colors.dim,
                    reset = colors.reset
                );
            }
        }
        return Ok(());
    }

    // Otherwise, show merged config as YAML
    let config = match load_config(Some(&repo_root))? {
        Some(c) => c,
        None => {
            // No config files exist, show defaults
            println!(
                "{dim}# No config files found. Using defaults:{reset}",
                dim = colors.dim,
                reset = colors.reset
            );
            load_config_or_default(Some(&repo_root))
        }
    };

    // Show sources
    let has_global = global_path.exists();
    let has_repo = repo_path.exists();
    if has_global || has_repo {
        print!("{dim}# Merged from:", dim = colors.dim);
        if has_global {
            print!(" ~/.lf/config.yaml");
        }
        if has_repo {
            print!(" .lf/config.yaml");
        }
        println!("{reset}", reset = colors.reset);
    }

    // Serialize to YAML
    let yaml = serde_yaml::to_string(&config)?;
    print!("{}", yaml);

    Ok(())
}
