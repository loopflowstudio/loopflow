use crate::commands::util::find_repo_root;
use anyhow::{anyhow, Result};
use loopflow_engine::config::load_config;
use std::fs;

pub fn show(global: bool, repo: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home directory"))?;

    let global_path = home.join(".lf/config.yaml");
    let repo_path = repo_root.join(".lf/config.yaml");

    if global || repo {
        if global {
            if global_path.exists() {
                println!("# Global config: {}", global_path.display());
                println!("{}", fs::read_to_string(&global_path)?);
            } else {
                println!("No global config found at {}", global_path.display());
            }
        }
        if repo {
            if repo_path.exists() {
                println!("# Repo config: {}", repo_path.display());
                println!("{}", fs::read_to_string(&repo_path)?);
            } else {
                println!("No repo config found at {}", repo_path.display());
            }
        }
        return Ok(());
    }

    match load_config(Some(&repo_root))? {
        Some(config) => {
            println!("{:#?}", config);
            Ok(())
        }
        None => {
            println!("No config found.");
            Ok(())
        }
    }
}
