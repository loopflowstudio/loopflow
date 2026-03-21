use std::collections::HashSet;
use std::path::Path;

use anyhow::{anyhow, Result};
use time::OffsetDateTime;
use tracing::{info, warn};

use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::types::{AgentRun, AgentStatus, WaveRunStatus, WaveStatus};

use super::{
    DockerExecutor, DockerRecoveryBackend, OutputContext, ReattachTarget, RehydrationPlan,
    StartupRecovery,
};

impl DockerExecutor {
    pub(super) async fn active_container_ids(&self) -> HashSet<String> {
        self.active
            .lock()
            .await
            .values()
            .cloned()
            .collect::<HashSet<_>>()
    }

    pub(super) async fn find_running_container(
        &self,
        backend: &dyn DockerRecoveryBackend,
        agent_run: &AgentRun,
    ) -> Result<Option<String>> {
        let container_name = Self::build_container_name(agent_run.id.as_str());
        let persisted_ref = agent_run
            .container_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());
        for container_ref in persisted_ref
            .into_iter()
            .chain(std::iter::once(container_name.as_str()))
        {
            match backend.inspect_container(container_ref).await {
                Ok(Some(container)) if container.running => return Ok(Some(container.id)),
                Ok(Some(_)) => {}
                Ok(None) => {}
                Err(err) => {
                    warn!(
                        agent_id = %agent_run.id,
                        container_ref,
                        error = %err,
                        "failed inspecting container during startup recovery"
                    );
                }
            }
        }

        Ok(None)
    }

    pub(super) async fn plan_rehydration(
        &self,
        backend: &dyn DockerRecoveryBackend,
    ) -> Result<RehydrationPlan> {
        let agents = self.store.list_agents().await?;
        let mut plan = RehydrationPlan {
            reattach: Vec::new(),
            lost: Vec::new(),
        };

        for agent_run in agents.into_iter().filter(|agent_run| {
            agent_run.status == AgentStatus::Running && agent_run.ended_at.is_none()
        }) {
            let Some(wave_run_id) = agent_run.wave_run_id.clone() else {
                plan.lost.push(agent_run);
                continue;
            };

            let Some(run) = self.store.get_wave_run(&wave_run_id).await? else {
                plan.lost.push(agent_run);
                continue;
            };

            match self.find_running_container(backend, &agent_run).await? {
                Some(container_id) => plan.reattach.push(ReattachTarget {
                    agent_run,
                    wave_id: run.wave_id.clone(),
                    wave_run_id,
                    container_id,
                }),
                None => plan.lost.push(agent_run),
            }
        }

        Ok(plan)
    }

    pub(super) fn spawn_reattach_task(
        &self,
        output: OutputHub,
        target: ReattachTarget,
    ) -> tokio::task::JoinHandle<()> {
        let executor = self.clone();
        tokio::spawn(async move {
            let result = executor
                .reattach_agent(
                    &output,
                    &target.agent_run,
                    &target.wave_id,
                    &target.wave_run_id,
                )
                .await;
            if let Err(err) = executor
                .finalize_reattached_agent(
                    &target.agent_run,
                    &target.wave_id,
                    &target.wave_run_id,
                    result,
                )
                .await
            {
                warn!(
                    agent_id = %target.agent_run.id,
                    wave_run_id = %target.wave_run_id,
                    error = %err,
                    "failed finalizing reattached container"
                );
            }
        })
    }

    pub(super) async fn reattach_agent(
        &self,
        output: &OutputHub,
        agent_run: &AgentRun,
        wave_id: &LfdId,
        wave_run_id: &LfdId,
    ) -> Result<i32> {
        let container_id = self
            .active
            .lock()
            .await
            .get(agent_run.id.as_str())
            .cloned()
            .ok_or_else(|| anyhow!("active container missing for reattach"))?;

        let workspace = self
            .resolve_workspace_for_recovery(
                wave_id.as_str(),
                wave_run_id.as_str(),
                Path::new(&agent_run.worktree),
            )
            .await?;
        let exit_code = self
            .wait_for_container_with_logs(
                &container_id,
                OutputContext {
                    wave_id: wave_id.to_string(),
                    wave_run_id: wave_run_id.to_string(),
                    agent_id: agent_run.id.to_string(),
                    output: output.clone(),
                    output_prefix: None,
                },
            )
            .await;

        self.active.lock().await.remove(agent_run.id.as_str());

        let sync_result = self
            .sync_to_host_worktree(&workspace, Path::new(&agent_run.worktree))
            .await;
        self.remove_container(&container_id).await;
        sync_result?;

        exit_code
    }

    pub(super) async fn finalize_reattached_agent(
        &self,
        agent_run: &AgentRun,
        wave_id: &LfdId,
        wave_run_id: &LfdId,
        result: Result<i32>,
    ) -> Result<()> {
        let ended_at = OffsetDateTime::now_utc().unix_timestamp();
        let (agent_status, run_status, run_error) = match result {
            Ok(0) => (AgentStatus::Completed, WaveRunStatus::Completed, None),
            Ok(code) => (
                AgentStatus::Failed,
                WaveRunStatus::Failed,
                Some(format!("reattached agent exited with code {code}")),
            ),
            Err(err) => (
                AgentStatus::Failed,
                WaveRunStatus::Failed,
                Some(format!("reattached agent failed: {err}")),
            ),
        };

        let _ = self
            .store
            .end_agent(&agent_run.id, agent_status.as_i32(), ended_at)
            .await;

        let mut next_wave_status = None;
        if let Some(mut run) = self.store.get_wave_run(wave_run_id).await? {
            if !matches!(run.status, WaveRunStatus::Completed | WaveRunStatus::Failed) {
                run.status = run_status;
                run.ended_at = Some(OffsetDateTime::now_utc());
                run.error = run_error;
                self.store.update_wave_run(&run).await?;
                next_wave_status = Some(if run_status == WaveRunStatus::Completed {
                    WaveStatus::Idle
                } else {
                    WaveStatus::Failed
                });
            }
        }

        if let Some(wave_status) = next_wave_status {
            if let Some(mut wave) = self.store.get_wave(wave_id).await? {
                wave.status = wave_status;
                let _ = self.store.update_wave(&wave).await;
            }
        }

        Ok(())
    }

    pub(super) async fn mark_agent_lost(&self, agent_run: &AgentRun) -> Result<()> {
        let ended_at = OffsetDateTime::now_utc().unix_timestamp();
        let _ = self
            .store
            .end_agent(&agent_run.id, AgentStatus::Failed.as_i32(), ended_at)
            .await;

        if let Some(wave_run_id) = &agent_run.wave_run_id {
            if let Some(mut run) = self.store.get_wave_run(wave_run_id).await? {
                let mut should_fail_wave = false;
                if !matches!(run.status, WaveRunStatus::Completed | WaveRunStatus::Failed) {
                    run.status = WaveRunStatus::Failed;
                    run.error = Some("container lost during lfd restart.".to_string());
                    run.ended_at = Some(OffsetDateTime::now_utc());
                    self.store.update_wave_run(&run).await?;
                    should_fail_wave = true;
                }

                if should_fail_wave {
                    if let Some(mut wave) = self.store.get_wave(&run.wave_id).await? {
                        wave.status = WaveStatus::Failed;
                        let _ = self.store.update_wave(&wave).await;
                    }
                }
            }
        }

        Ok(())
    }

    pub(super) async fn cleanup_orphaned_containers(
        &self,
        backend: &dyn DockerRecoveryBackend,
    ) -> Result<u32> {
        let active_ids = self.active_container_ids().await;
        let mut removed = 0u32;
        let containers = backend.list_managed_containers().await?;
        for container_id in containers {
            if active_ids.contains(&container_id) {
                continue;
            }
            if let Err(err) = backend.stop_container(&container_id).await {
                warn!(
                    container_id,
                    error = %err,
                    "failed stopping orphaned managed container"
                );
            }
            if let Err(err) = backend.remove_container(&container_id).await {
                warn!(
                    container_id,
                    error = %err,
                    "failed removing orphaned managed container"
                );
                continue;
            }
            info!(container_id, "removed orphaned managed container");
            removed += 1;
        }
        Ok(removed)
    }

    pub(super) async fn recover_startup_with_backend(
        &self,
        backend: &dyn DockerRecoveryBackend,
        output: &OutputHub,
        spawn_reattach: bool,
    ) -> Result<StartupRecovery> {
        let plan = self.plan_rehydration(backend).await?;

        for lost in &plan.lost {
            if let Err(err) = self.mark_agent_lost(lost).await {
                warn!(
                    agent_id = %lost.id,
                    error = %err,
                    "failed marking lost container state"
                );
            }
        }

        for target in plan.reattach.iter().cloned() {
            self.active
                .lock()
                .await
                .insert(target.agent_run.id.to_string(), target.container_id.clone());
            if spawn_reattach {
                std::mem::drop(self.spawn_reattach_task(output.clone(), target));
            }
        }

        let orphaned_containers_removed = self.cleanup_orphaned_containers(backend).await?;
        Ok(StartupRecovery {
            orphaned_runs_failed: 0,
            rehydrated_agents: plan.reattach.len() as u32,
            lost_agents_failed: plan.lost.len() as u32,
            orphaned_containers_removed,
        })
    }
}
