use std::path::Path;

use anyhow::{anyhow, Result};
use time::OffsetDateTime;
use tracing::warn;

use crate::engine::agent::build_agent_command;
use crate::engine::flow::{ConcreteStep, Step};
use crate::lfd::id::LfdId;
use crate::lfd::types::{SidecarKind, Wave, WaveRun, WaveRunKind, WaveRunStatus};

use super::launch::AgentLaunchRequest;
use super::WaveExecutor;
use crate::lfd::executor::helpers::{
    build_agent_capabilities, build_step_prompt, ci_fix_worktree_path, cleanup_ci_fix_worktree,
    commit_and_push_ci_fix, create_ci_fix_worktree, format_ci_failure_message, short_hash,
};
use crate::lfd::executor::CiFailure;

impl WaveExecutor {
    pub async fn spawn_ci_fix_agent(&self, failure: &CiFailure) -> Result<()> {
        let sidecar_run_id = LfdId::new();
        let _slot_guard = self
            .scheduler
            .acquire_guard(sidecar_run_id.as_str())
            .await
            .map_err(|reason| {
                anyhow!("scheduler at capacity, cannot start CI fix agent: {reason}")
            })?;

        self.run_ci_fix_agent_with_slot(failure, &sidecar_run_id)
            .await
    }

    async fn run_ci_fix_agent_with_slot(
        &self,
        failure: &CiFailure,
        sidecar_run_id: &LfdId,
    ) -> Result<()> {
        let wave = self
            .store
            .get_wave(&failure.wave_id)
            .await?
            .ok_or_else(|| anyhow!("wave not found for CI fix"))?;
        let source_run = self
            .store
            .get_wave_run(&failure.wave_run_id)
            .await?
            .ok_or_else(|| anyhow!("wave run not found for CI fix"))?;

        let worktree_path =
            ci_fix_worktree_path(Path::new(wave.repo()), wave.name(), sidecar_run_id);
        let worktree = worktree_path.to_string_lossy().to_string();
        let temp_branch = format!("ci-fix-{}", short_hash(sidecar_run_id.as_str(), 8));

        create_ci_fix_worktree(
            Path::new(wave.repo()),
            &worktree_path,
            &failure.branch,
            &temp_branch,
        )?;

        let mut snapshot = source_run.snapshot.clone();
        snapshot.flow = "debug".to_string();
        snapshot.pr = None;
        let mut run = WaveRun {
            id: sidecar_run_id.clone(),
            wave_id: wave.id().clone(),
            snapshot,
            iteration: source_run.iteration,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: worktree.clone(),
            branch: temp_branch,
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind: WaveRunKind::Sidecar,
            sidecar_kind: Some(SidecarKind::CiFix),
            parent_run_id: Some(source_run.id.clone()),
            parent_pr_number: source_run.snapshot.pr.as_ref().and_then(|pr| pr.number),
            stack_position: source_run.stack_position,
            stack_group_id: source_run.stack_group_id.clone(),
            stack_status: source_run.stack_status,
            lineage_inferred: false,
        };
        if let Err(err) = self.store.create_wave_run(&run).await {
            if let Err(cleanup_err) = cleanup_ci_fix_worktree(&worktree_path) {
                warn!(
                    worktree = %worktree,
                    error = %cleanup_err,
                    "failed to clean up CI fix worktree after create_wave_run error"
                );
            }
            return Err(err.into());
        }

        let result = self.execute_ci_fix_agent(&wave, &run, failure).await;
        run.ended_at = Some(OffsetDateTime::now_utc());
        if let Err(err) = &result {
            run.status = WaveRunStatus::Failed;
            run.error = Some(err.to_string());
        } else {
            run.status = WaveRunStatus::Completed;
        }

        let update_result = self.store.update_wave_run(&run).await;

        if let Err(err) = cleanup_ci_fix_worktree(&worktree_path) {
            warn!(worktree = %worktree, error = %err, "failed to clean up CI fix worktree");
        }

        update_result?;
        result
    }

    async fn execute_ci_fix_agent(
        &self,
        wave: &Wave,
        run: &WaveRun,
        failure: &CiFailure,
    ) -> Result<()> {
        let step = ConcreteStep {
            step: Step::named("debug"),
            flow_parents: Vec::new(),
        };
        let message = format_ci_failure_message(failure);
        let (launch, process) = build_step_prompt(
            &run.worktree,
            &step,
            &run.snapshot.direction,
            Some(wave.name()),
            Some((&self.store, wave.id())),
            Some(message),
        )
        .await?;
        let capabilities = build_agent_capabilities(&run.worktree);
        let model = launch.model.clone().unwrap_or_else(|| "claude".to_string());

        let outcome = self
            .launch_agent(AgentLaunchRequest {
                wave_id: wave.id().clone(),
                wave_run_id: run.id.clone(),
                repo: run.snapshot.repo.clone(),
                worktree: run.worktree.clone(),
                step,
                model: model.clone(),
                cmd: build_agent_command(&launch, &process, &capabilities),
                output_prefix: None,
            })
            .await?;

        if outcome.exit_code != 0 {
            return Err(anyhow!(
                "CI fix debug step failed with exit code {}",
                outcome.exit_code
            ));
        }

        commit_and_push_ci_fix(
            Path::new(&run.worktree),
            &failure.branch,
            &failure.check_name,
        )?;
        Ok(())
    }
}
