use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::engine::agent::LaunchConfig;
use crate::engine::config::load_config_or_default;
use crate::engine::flow::load_step;
use crate::engine::fork::merge_directions;
use crate::engine::prompt::{
    default_gather_sources, drop_native_instruction_docs, format_context_prompt, format_prompt,
    format_task_prompt, gather_context, trim_context_with_breakdown, write_prompt_log, Document,
    DocumentSource, GatherContextOpts, PromptFormatMode, DEFAULT_CONTEXT_BUDGET,
};
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
    let loaded_step = load_step(config.step, repo_root)?;
    let file_config = load_config_or_default(Some(repo_root));
    let directions = merge_directions(config.directions, &loaded_step.directions);

    let opts = GatherContextOpts {
        repo_root: repo_root.to_path_buf(),
        step: Some(config.step.to_string()),
        message: config.message,
        run_mode: Some(config.run_mode.to_string()),
        directions,
        files: Vec::new(),
        sources: default_gather_sources(
            file_config.lfdocs,
            file_config.diff_files || file_config.diff,
            file_config.paste,
        ),
        area: config.area.or_else(|| file_config.area.clone()),
        wave: config.wave,
    };

    let mut components = gather_context(&opts)?;
    drop_native_instruction_docs(&mut components, repo_root);

    if let Some((store, wave_id)) = config.summary_source {
        if let Ok(Some(summary)) = store.get_summary(wave_id).await {
            components.summaries.push(Document {
                path: "wave-summary".to_string(),
                content: summary.content,
                source: DocumentSource::Summary,
            });
        }
    }

    let (components, _breakdown) = trim_context_with_breakdown(components, DEFAULT_CONTEXT_BUDGET);
    let _ = write_prompt_log(
        repo_root,
        &format_prompt(PromptFormatMode::Full, &components),
        config.step,
        None,
    );

    let task_prompt = format_task_prompt(&components);
    let system_prompt = format_context_prompt(&components);

    let model = config
        .model
        .or(loaded_step.model)
        .or(Some(file_config.agent_model.clone()));
    let cwd = config.cwd.unwrap_or_else(|| repo_root.to_path_buf());

    Ok(LaunchConfig {
        system_prompt,
        task_prompt,
        model,
        cwd: Some(cwd),
        max_turns: config.max_turns,
        skip_permissions: config.yolo_mode,
    })
}
