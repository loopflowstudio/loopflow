use std::path::Path;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::engine::agent::build_agent_command;
use crate::engine::flow::{ConcreteFork, ConcreteItem, ConcreteStep};
use crate::engine::fork::plan_fork_execution;
use crate::engine::worktree::{create_worktree, remove_worktree};
use crate::lfd::config::ExecutorType;
use crate::lfd::id::LfdId;
use crate::lfd::store::{ForkRun, ForkRunStatus};
use crate::lfd::types::{Wave, WaveRun};

use super::launch::AgentLaunchRequest;
use super::WaveExecutor;
use crate::lfd::executor::helpers::{build_step_prompt, fork_worktree_path};

#[derive(Debug, Clone)]
struct ForkBranchExecution {
    run: ForkRun,
    step: ConcreteStep,
    label: String,
}

impl WaveExecutor {
    pub(super) async fn run_choose(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        let selected = match plan_fork_execution(
            &fork.select,
            &fork.branches,
            &run.snapshot.direction,
            None,
        ) {
            Ok(mut branches) => branches
                .pop()
                .ok_or_else(|| anyhow::anyhow!("fork branch missing after selection"))?,
            Err(err) => {
                self.fail_run(run, wave, err).await?;
                return Ok(());
            }
        };

        let exit_code = self.run_step(wave, run, &selected.step).await?;
        if exit_code != 0 {
            self.fail_run(
                run,
                wave,
                format!("fork step {} failed", selected.step.step.name),
            )
            .await?;
            return Ok(());
        }

        self.advance_run_step(run, plan, &wave.id).await?;
        Ok(())
    }

    pub(super) async fn run_fork(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        if self.executor_type == ExecutorType::Docker {
            self.fail_run(
                run,
                wave,
                "fork(select=all) is not supported by the docker executor yet".to_string(),
            )
            .await?;
            return Ok(());
        }

        let planned = match plan_fork_execution(
            &fork.select,
            &fork.branches,
            &run.snapshot.direction,
            None,
        ) {
            Ok(branches) => branches,
            Err(err) => {
                self.fail_run(run, wave, err).await?;
                return Ok(());
            }
        };

        let mut fork_runs = Vec::new();
        for branch in planned {
            let index = branch.index;
            let fork_worktree = fork_worktree_path(run, index as u32);
            if !Path::new(&fork_worktree).exists() {
                debug!(
                    run_id = %run.id,
                    branch_index = index,
                    step = %branch.step.step.name,
                    worktree = %fork_worktree,
                    "creating fork worktree"
                );
                create_worktree(
                    Path::new(&run.snapshot.repo),
                    Path::new(&fork_worktree),
                    &format!("{}-fork-{}", run.id, index),
                )?;
            }

            let fork_run = ForkRun {
                id: LfdId::new(),
                wave_run_id: run.id.clone(),
                step_index: run.step_index,
                branch_index: index as u32,
                status: ForkRunStatus::Pending,
                worktree: fork_worktree,
            };
            self.store.upsert_fork_run(&fork_run).await?;
            fork_runs.push(ForkBranchExecution {
                run: fork_run,
                step: branch.step,
                label: branch.label,
            });
        }

        let cancel = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(fork_runs.len());
        let mut handles = Vec::new();

        let wave_directions = run.snapshot.direction.clone();
        for execution in fork_runs.iter() {
            let executor = self.clone();
            let scheduler = self.scheduler.clone();
            let cancel = cancel.clone();
            let tx = tx.clone();
            let fork_wave_id = wave.id.clone();
            let wave_run_id = run.id.clone();
            let wave_repo = run.snapshot.repo.clone();
            let worktree = execution.run.worktree.clone();
            let fork_run_id = execution.run.id.clone();
            let branch_label = execution.label.clone();
            let fork_run = execution.run.clone();
            let step = execution.step.clone();
            let wave_directions = wave_directions.clone();

            let handle = tokio::spawn(async move {
                if cancel.is_cancelled() {
                    return;
                }

                let _slot_guard = loop {
                    if cancel.is_cancelled() {
                        return;
                    }
                    match scheduler.acquire_guard(fork_run_id.as_str()).await {
                        Ok(guard) => break guard,
                        Err(_) => {
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                    }
                };

                if cancel.is_cancelled() {
                    return;
                }

                let _ = executor
                    .store
                    .upsert_fork_run(&ForkRun {
                        status: ForkRunStatus::Running,
                        ..fork_run.clone()
                    })
                    .await;

                debug!(
                    fork_run_id = %fork_run_id,
                    step = %step.step.name,
                    worktree = %worktree,
                    directions = ?wave_directions,
                    "building fork branch prompt"
                );
                let prompt = build_step_prompt(
                    &worktree,
                    &step,
                    &wave_directions,
                    None,
                    Some((&executor.store, &fork_wave_id)),
                    None,
                )
                .await;
                let (prompt, model, launch) = match prompt {
                    Ok(result) => result,
                    Err(err) => {
                        error!(
                            fork_run_id = %fork_run_id,
                            step = %step.step.name,
                            error = %err,
                            "fork branch prompt build failed"
                        );
                        let _ = tx.send((branch_label.clone(), Err(err))).await;
                        return;
                    }
                };
                let cmd = build_agent_command(&model, &prompt, &launch);
                info!(
                    fork_run_id = %fork_run_id,
                    step = %step.step.name,
                    model = %model,
                    cmd_len = cmd.len(),
                    "launching fork branch agent"
                );

                let result = executor
                    .launch_agent(AgentLaunchRequest {
                        wave_id: fork_wave_id.clone(),
                        wave_run_id: wave_run_id.clone(),
                        repo: wave_repo.clone(),
                        worktree: worktree.clone(),
                        step: step.clone(),
                        model,
                        cmd,
                        output_prefix: Some(format!("[{}] ", branch_label)),
                    })
                    .await
                    .map(|outcome| outcome.exit_code);

                let status = match &result {
                    Ok(0) => {
                        info!(fork_run_id = %fork_run_id, step = %step.step.name, "fork branch completed");
                        ForkRunStatus::Completed
                    }
                    Ok(code) => {
                        warn!(fork_run_id = %fork_run_id, step = %step.step.name, exit_code = code, "fork branch failed");
                        ForkRunStatus::Failed
                    }
                    Err(err) => {
                        error!(fork_run_id = %fork_run_id, step = %step.step.name, error = %err, "fork branch error");
                        ForkRunStatus::Failed
                    }
                };
                let _ = executor
                    .store
                    .upsert_fork_run(&ForkRun {
                        status,
                        ..fork_run.clone()
                    })
                    .await;
                let _ = tx.send((branch_label.clone(), result)).await;
            });

            handles.push(handle);
        }
        drop(tx);

        let mut failures = None;
        let mut completed = 0usize;
        let total = fork_runs.len();
        debug!(run_id = %run.id, total_branches = total, "waiting for fork results");
        while let Some((branch_label, result)) = rx.recv().await {
            match result {
                Ok(0) => {
                    completed += 1;
                    debug!(run_id = %run.id, completed, total, "fork branch done");
                    if completed == total {
                        break;
                    }
                }
                Ok(code) => {
                    failures = Some(format!(
                        "fork branch {branch_label} exited with code {code}"
                    ));
                    cancel.cancel();
                    break;
                }
                Err(err) => {
                    failures = Some(format!("fork branch {branch_label} error: {err}"));
                    cancel.cancel();
                    break;
                }
            }
        }
        for handle in handles {
            let _ = handle.await;
        }

        if let Some(error) = failures {
            error!(run_id = %run.id, error = %error, "fork failed");
            self.cleanup_fork(run, &fork_runs).await;
            self.fail_run(run, wave, error).await?;
            return Ok(());
        }

        self.cleanup_fork(run, &fork_runs).await;
        self.advance_run_step(run, plan, &wave.id).await?;
        Ok(())
    }

    async fn cleanup_fork(&self, run: &WaveRun, fork_runs: &[ForkBranchExecution]) {
        for execution in fork_runs {
            let worktree_path = Path::new(&execution.run.worktree);
            if worktree_path.join(".git").exists() {
                let _ = remove_worktree(worktree_path, true);
            }
        }
        let _ = self.store.delete_fork_runs(&run.id, run.step_index).await;
    }
}
