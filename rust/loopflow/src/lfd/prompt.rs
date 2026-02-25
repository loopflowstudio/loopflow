use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::engine::agent::LaunchConfig;
use crate::engine::config::load_config_or_default;
use crate::engine::launch::{prepare_launch_prompt, LaunchPromptInput};
use crate::engine::prompt::write_prompt_log;
use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;

pub struct PrepareStepPromptConfig<'a> {
    pub repo_root: &'a Path,
    pub step: &'a str,
    pub run_mode: &'a str,
    pub directions: &'a [String],
    pub area: Option<String>,
    pub wave: Option<String>,
    pub message: Option<String>,
    pub model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub max_turns: Option<u32>,
    pub yolo_mode: bool,
    pub summary_source: Option<(&'a SharedStore, &'a LfdId)>,
}

pub async fn prepare_step_prompt(config: PrepareStepPromptConfig<'_>) -> Result<LaunchConfig> {
    let repo_root = config.repo_root;
    let file_config = load_config_or_default(Some(repo_root));
    let summary = if let Some((store, wave_id)) = config.summary_source {
        store
            .get_summary(wave_id)
            .await
            .ok()
            .flatten()
            .map(|record| record.content)
    } else {
        None
    };

    let prepared = prepare_launch_prompt(
        &file_config,
        LaunchPromptInput {
            repo_root: repo_root.to_path_buf(),
            step: Some(config.step.to_string()),
            run_mode: Some(config.run_mode.to_string()),
            directions: config.directions.to_vec(),
            area: config.area,
            wave: config.wave,
            message: config.message,
            model: config.model,
            cwd: config.cwd,
            max_turns: config.max_turns,
            yolo_mode: config.yolo_mode,
            include_config_directions: false,
            include_config_area: true,
            source_overrides: Default::default(),
            summary,
        },
    )?;

    let _ = write_prompt_log(repo_root, &prepared.prompt, config.step, None);
    Ok(prepared.launch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn prepare_step_prompt_matches_engine_prepare_launch_prompt() {
        let tmp = tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join(".lf/steps")).expect("steps dir");
        fs::write(
            tmp.path().join(".lf/steps/test.md"),
            r#"---
model: codex:o3
---
Test step body.
"#,
        )
        .expect("write step");
        fs::write(
            tmp.path().join(".lf/config.yaml"),
            "lfdocs: false\ndiff_files: false\ndiff: false\npaste: false\n",
        )
        .expect("write config");

        let launch = prepare_step_prompt(PrepareStepPromptConfig {
            repo_root: tmp.path(),
            step: "test",
            run_mode: "auto",
            directions: &[],
            area: None,
            wave: None,
            message: Some("hello".to_string()),
            model: None,
            cwd: None,
            max_turns: Some(4),
            yolo_mode: true,
            summary_source: None,
        })
        .await
        .expect("prepare step prompt");

        let file_config = load_config_or_default(Some(tmp.path()));
        let prepared = prepare_launch_prompt(
            &file_config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                step: Some("test".to_string()),
                run_mode: Some("auto".to_string()),
                message: Some("hello".to_string()),
                max_turns: Some(4),
                yolo_mode: true,
                include_config_directions: false,
                include_config_area: true,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(launch.system_prompt, prepared.launch.system_prompt);
        assert_eq!(launch.task_prompt, prepared.launch.task_prompt);
        assert_eq!(launch.model, prepared.launch.model);
        assert_eq!(launch.max_turns, prepared.launch.max_turns);
        assert_eq!(launch.cwd, prepared.launch.cwd);
        assert_eq!(launch.skip_permissions, prepared.launch.skip_permissions);
    }
}
