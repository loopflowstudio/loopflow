use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use time::OffsetDateTime;
use tracing::{debug, info, warn};

use crate::engine::agent::{build_agent_command, AgentCapabilities, LaunchConfig, ProcessConfig};
use crate::engine::builtins::get_builtin_ops_prompt;
use crate::engine::config::load_config_or_default;
use crate::engine::flow::{ConcreteStep, Step};
use crate::engine::git::hash_areas;
use crate::lfd::id::LfdId;
use crate::lfd::types::{Summary, Wave, WaveRun};

use super::launch::AgentLaunchRequest;
use super::WaveExecutor;

impl WaveExecutor {
    /// Check if the wave's area summary is fresh; regenerate if stale or missing.
    pub(crate) async fn ensure_summary_fresh(&self, wave: &Wave, run: &WaveRun) -> Result<()> {
        if wave.area().is_empty() {
            return Ok(());
        }

        let worktree_path = Path::new(&run.worktree);
        let current_hash = match hash_areas(worktree_path, wave.area()) {
            Ok(h) => h,
            Err(err) => {
                warn!(wave = %wave.name(), error = %err, "failed to hash areas, skipping summary");
                return Ok(());
            }
        };

        if let Ok(Some(existing)) = self.store.get_summary(wave.id()).await {
            if existing.source_hash == current_hash {
                debug!(wave = %wave.name(), "summary is fresh");
                return Ok(());
            }
            info!(wave = %wave.name(), "summary is stale, regenerating");
        } else {
            info!(wave = %wave.name(), "no summary found, generating");
        }

        self.run_internal_summarize(wave, run, &current_hash).await
    }

    /// Run the builtin summarize step as an internal agent and store the result.
    async fn run_internal_summarize(
        &self,
        wave: &Wave,
        run: &WaveRun,
        source_hash: &str,
    ) -> Result<()> {
        let template = get_builtin_ops_prompt("summarize")
            .ok_or_else(|| anyhow!("builtin summarize prompt not found"))?;

        let config = load_config_or_default(Some(Path::new(&run.worktree)));
        let token_budget = config.summary_tokens;

        let area_list = wave.area().join(", ");
        let prompt = template
            .replace("{token_budget}", &token_budget.to_string())
            .replace(
                "{content}",
                &format!("Read and summarize these paths: {area_list}"),
            );

        let model = config.agent_model.clone();
        let launch = LaunchConfig {
            task_prompt: prompt,
            model: Some(model.clone()),
            cwd: Some(PathBuf::from(&run.worktree)),
            skip_permissions: config.yolo,
            ..Default::default()
        };
        let process = ProcessConfig {
            auto: true,
            stream: true,
            ..Default::default()
        };
        let capabilities = AgentCapabilities {
            chrome: config.chrome,
        };

        let cmd = build_agent_command(&launch, &process, &capabilities);
        info!(wave = %wave.name(), model = %model, "running internal summarize step");

        let step = ConcreteStep {
            step: Step {
                name: "_summarize".to_string(),
                model: Some(model.clone()),
                directions: Vec::new(),
                interactive: Some(false),
                content: None,
            },
            flow_parents: Vec::new(),
        };

        let outcome = self
            .launch_agent(AgentLaunchRequest {
                wave_id: wave.id().clone(),
                wave_run_id: run.id.clone(),
                branch: Some(run.branch.clone()),
                repo: run.snapshot.repo.clone(),
                worktree: run.worktree.clone(),
                step,
                model: model.clone(),
                cmd,
                output_prefix: None,
            })
            .await?;

        if outcome.exit_code != 0 {
            warn!(wave = %wave.name(), exit_code = outcome.exit_code, "summarize step failed, continuing without summary");
            return Ok(());
        }

        let summary_path = Path::new(&run.worktree).join(".lf/summary.md");
        match std::fs::read_to_string(&summary_path) {
            Ok(content) if !content.trim().is_empty() => {
                let summary = Summary {
                    id: LfdId::new(),
                    wave_id: wave.id().clone(),
                    content,
                    source_hash: source_hash.to_string(),
                    token_budget: token_budget as u32,
                    model: config.agent_model,
                    created_at: Some(OffsetDateTime::now_utc()),
                };
                self.store.upsert_summary(&summary).await?;
                info!(wave = %wave.name(), "summary stored");
            }
            Ok(_) => {
                warn!(wave = %wave.name(), "summarize step produced empty output");
            }
            Err(err) => {
                warn!(wave = %wave.name(), error = %err, "failed to read summary file");
            }
        }

        Ok(())
    }
}
