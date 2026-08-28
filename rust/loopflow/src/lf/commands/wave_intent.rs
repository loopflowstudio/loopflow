//! Pause or resume new turn starts without changing listener residency.

use std::path::Path;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// The authored turn intent after one idempotent pause or resume mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveIntentReceipt {
    pub wave: String,
    pub paused: bool,
}

pub fn run(name: &str, paused: bool, json: bool, repo: &Path) -> Result<()> {
    let receipt = set_wave_paused(repo, name, paused)?;
    if json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if paused {
        println!("paused wave {}", receipt.wave);
    } else {
        println!("resumed wave {}", receipt.wave);
    }
    Ok(())
}

pub fn set_wave_paused(repo: &Path, name: &str, paused: bool) -> Result<WaveIntentReceipt> {
    let name = crate::ops::util::normalize_wave_name(name)
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;
    let repo =
        crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());

    let config = crate::work::wave::config::try_read_wave_config(&repo, &name)?;
    if config.is_none() {
        return Err(anyhow!(
            "Wave '{name}' has no authored goal at {}",
            repo.join("wave").join(&name).join("GOAL.md").display()
        ));
    }

    crate::work::wave::config::update_wave_paused(&repo, &name, paused)
        .map_err(anyhow::Error::msg)?;
    Ok(WaveIntentReceipt { wave: name, paused })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn pause_and_resume_are_idempotent_and_preserve_authored_goal() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("product");
        fs::create_dir_all(&dir).expect("create goal dir");
        let goal = dir.join("GOAL.md");
        let body = "\n## Objective\n\nBuild the control room.\n";
        fs::write(&goal, format!("---\nowner: jack\n---\n{body}")).expect("write goal");

        let paused = set_wave_paused(temp.path(), "product", true).expect("pause");
        assert_eq!(
            paused,
            WaveIntentReceipt {
                wave: "product".to_string(),
                paused: true,
            }
        );
        set_wave_paused(temp.path(), "product", true).expect("pause again");
        let content = fs::read_to_string(&goal).expect("read paused goal");
        assert!(content.contains("owner: jack"));
        assert!(content.contains("paused: true"));
        assert!(content.ends_with(body));

        let resumed = set_wave_paused(temp.path(), "product", false).expect("resume");
        assert!(!resumed.paused);
        set_wave_paused(temp.path(), "product", false).expect("resume again");
        let content = fs::read_to_string(&goal).expect("read resumed goal");
        assert!(!content.contains("paused:"));
        assert!(content.ends_with(body));
    }

    #[test]
    fn missing_or_malformed_goal_fails_without_mutation() {
        let temp = tempdir().expect("temp dir");
        let missing = set_wave_paused(temp.path(), "missing", true).unwrap_err();
        assert!(missing.to_string().contains("has no authored goal"));
        assert!(!temp.path().join("wave/missing/GOAL.md").exists());

        let dir = temp.path().join("wave").join("broken");
        fs::create_dir_all(&dir).expect("create goal dir");
        let goal = dir.join("GOAL.md");
        let original = "---\nowner: [\n---\nDo not rewrite me.\n";
        fs::write(&goal, original).expect("write malformed goal");

        let malformed = set_wave_paused(temp.path(), "broken", true).unwrap_err();
        assert!(malformed
            .to_string()
            .contains("invalid wave goal frontmatter"));
        assert_eq!(fs::read_to_string(goal).expect("read goal"), original);
    }
}
