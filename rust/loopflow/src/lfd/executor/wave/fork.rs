use std::path::Path;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::engine::agent::build_agent_command;
use crate::engine::flow::{expand_direction_names, ConcreteFork, ConcreteItem, ConcreteStep, Step};
use crate::engine::fork::{
    plan_fork_execution, ForkManifest, ForkManifestBranch, FORK_MANIFEST_RELATIVE_PATH,
    FORK_SYNTHESIZE_STEP,
};
use crate::engine::worktree::create_worktree;
use crate::lfd::id::LfdId;
use crate::lfd::store::{ForkRun, ForkRunStatus};
use crate::lfd::types::{Wave, WaveRun};

use super::launch::AgentLaunchRequest;
use super::WaveExecutor;
use crate::lfd::executor::helpers::{
    build_agent_capabilities, build_step_prompt, fork_worktree_path,
};

#[derive(Debug, Clone)]
struct ForkBranchExecution {
    run: ForkRun,
    step: ConcreteStep,
    label: String,
    directions: Vec<String>,
    branch_name: String,
}

impl WaveExecutor {
    pub(super) async fn run_fork(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        let expanded_snapshot_directions =
            expand_direction_names(&run.snapshot.direction, Path::new(&run.snapshot.repo));
        let planned = match plan_fork_execution(&fork.branches, &expanded_snapshot_directions) {
            Ok(branches) => branches,
            Err(err) => {
                self.fail_run(run, wave, err).await?;
                return Ok(());
            }
        };

        let mut fork_runs = Vec::new();
        for branch in planned {
            let index = branch.index;
            let branch_name = format!("{}-fork-{}", run.id, index);
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
                    &branch_name,
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
                directions: branch.directions,
                branch_name,
            });
        }

        let (tx, mut rx) = mpsc::channel(fork_runs.len());
        let mut handles = Vec::new();

        for execution in fork_runs.iter() {
            let executor = self.clone();
            let scheduler = self.scheduler.clone();
            let tx = tx.clone();
            let fork_wave_id = wave.id().clone();
            let wave_run_id = run.id.clone();
            let wave_repo = run.snapshot.repo.clone();
            let worktree = execution.run.worktree.clone();
            let fork_run_id = execution.run.id.clone();
            let branch_label = execution.label.clone();
            let branch_name = execution.branch_name.clone();
            let branch_index = execution.run.branch_index as usize;
            let fork_run = execution.run.clone();
            let step = execution.step.clone();
            let directions = execution.directions.clone();

            let handle = tokio::spawn(async move {
                let _slot_guard = loop {
                    match scheduler.acquire_guard(fork_run_id.as_str()).await {
                        Ok(guard) => break guard,
                        Err(_) => {
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                    }
                };

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
                    directions = ?directions,
                    "building fork branch prompt"
                );
                let prompt = build_step_prompt(
                    &worktree,
                    &step,
                    &directions,
                    None,
                    Some((&executor.store, &fork_wave_id)),
                    None,
                    None,
                )
                .await;
                let (launch, process) = match prompt {
                    Ok(result) => result,
                    Err(err) => {
                        error!(
                            fork_run_id = %fork_run_id,
                            step = %step.step.name,
                            error = %err,
                            "fork branch prompt build failed"
                        );
                        let _ = tx.send((branch_index, 1)).await;
                        return;
                    }
                };
                let capabilities = build_agent_capabilities(&worktree);
                let agent = launch.agent.clone().unwrap_or_else(|| "claude".to_string());
                let cmd = build_agent_command(&launch, &process, &capabilities);
                info!(
                    fork_run_id = %fork_run_id,
                    step = %step.step.name,
                    agent = %agent,
                    cmd_len = cmd.len(),
                    "launching fork branch agent"
                );

                let exit_code = match executor
                    .launch_agent(AgentLaunchRequest {
                        wave_id: fork_wave_id.clone(),
                        wave_run_id: wave_run_id.clone(),
                        branch: Some(branch_name),
                        repo: wave_repo.clone(),
                        worktree: worktree.clone(),
                        step: step.clone(),
                        agent,
                        cmd,
                        output_prefix: Some(format!("[{}] ", branch_label)),
                    })
                    .await
                {
                    Ok(outcome) => outcome.exit_code,
                    Err(err) => {
                        error!(fork_run_id = %fork_run_id, step = %step.step.name, error = %err, "fork branch error");
                        1
                    }
                };

                let status = match exit_code {
                    0 => {
                        info!(fork_run_id = %fork_run_id, step = %step.step.name, "fork branch completed");
                        ForkRunStatus::Completed
                    }
                    code => {
                        warn!(fork_run_id = %fork_run_id, step = %step.step.name, exit_code = code, "fork branch failed");
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
                let _ = tx.send((branch_index, exit_code)).await;
            });

            handles.push(handle);
        }
        drop(tx);

        let total = fork_runs.len();
        let mut branch_results: Vec<Option<i32>> = (0..total).map(|_| None).collect();
        let mut completed = 0usize;
        debug!(run_id = %run.id, total_branches = total, "waiting for fork results");
        while completed < total {
            let Some((branch_index, result)) = rx.recv().await else {
                break;
            };
            if let Some(slot) = branch_results.get_mut(branch_index) {
                *slot = Some(result);
            } else {
                warn!(
                    run_id = %run.id,
                    branch_index,
                    "received fork result for unknown branch index"
                );
            }
            completed += 1;
            debug!(run_id = %run.id, completed, total, "fork branch done");
        }
        for handle in handles {
            let _ = handle.await;
        }

        let mut outcomes = Vec::new();
        for execution in &fork_runs {
            let branch_index = execution.run.branch_index as usize;
            let exit_code = branch_results
                .get_mut(branch_index)
                .and_then(|entry| entry.take())
                .unwrap_or_else(|| {
                    warn!(
                        run_id = %run.id,
                        branch = %execution.label,
                        "fork branch result missing"
                    );
                    1
                });
            outcomes.push(ForkManifestBranch {
                index: execution.run.branch_index as usize,
                step: execution.step.step.name.clone(),
                direction: execution.directions.join(","),
                worktree: execution.run.worktree.clone(),
                branch: execution.branch_name.clone(),
                exit_code,
            });
        }
        let failed = outcomes.iter().filter(|o| o.exit_code != 0).count();

        let manifest = ForkManifest { branches: outcomes };
        let manifest_json = match serde_json::to_vec_pretty(&manifest) {
            Ok(json) => json,
            Err(err) => {
                self.cleanup_fork(run, &fork_runs, false).await;
                self.fail_run(run, wave, format!("failed encoding fork manifest: {err}"))
                    .await?;
                return Ok(());
            }
        };
        if let Err(err) = self
            .runner
            .write_to_workspace(
                Path::new(&run.worktree),
                FORK_MANIFEST_RELATIVE_PATH,
                &manifest_json,
            )
            .await
        {
            self.cleanup_fork(run, &fork_runs, false).await;
            self.fail_run(run, wave, format!("failed writing fork manifest: {err}"))
                .await?;
            return Ok(());
        }

        let synth_step = ConcreteStep {
            step: Step::named(FORK_SYNTHESIZE_STEP),
            flow_parents: fork.flow_parents.clone(),
        };
        let synth_exit = match self.run_step(wave, run, &synth_step).await {
            Ok(code) => code,
            Err(err) => {
                self.cleanup_fork(run, &fork_runs, true).await;
                self.fail_run(run, wave, format!("synthesize step failed: {err}"))
                    .await?;
                return Ok(());
            }
        };

        self.cleanup_fork(run, &fork_runs, true).await;

        if synth_exit != 0 {
            self.fail_run(
                run,
                wave,
                format!("synthesize exited with code {synth_exit}"),
            )
            .await?;
            return Ok(());
        }

        if failed > 0 {
            error!(run_id = %run.id, failed, "fork branches failed");
            self.fail_run(run, wave, format!("{failed} fork branch(es) failed"))
                .await?;
            return Ok(());
        }
        self.advance_run_step(run, plan, wave.id()).await?;
        Ok(())
    }

    async fn cleanup_fork(
        &self,
        run: &WaveRun,
        fork_runs: &[ForkBranchExecution],
        remove_manifest: bool,
    ) {
        if remove_manifest {
            if let Err(err) = self
                .runner
                .remove_from_workspace(Path::new(&run.worktree), FORK_MANIFEST_RELATIVE_PATH)
                .await
            {
                warn!(
                    run_id = %run.id,
                    error = %err,
                    "failed removing fork manifest"
                );
            }
        }

        for execution in fork_runs {
            if let Err(err) = self
                .runner
                .cleanup_ephemeral_worktree(
                    Path::new(&run.snapshot.repo),
                    Path::new(&execution.run.worktree),
                )
                .await
            {
                warn!(
                    run_id = %run.id,
                    worktree = %execution.run.worktree,
                    error = %err,
                    "failed cleaning fork worktree"
                );
            }
        }

        let _ = self.store.delete_fork_runs(&run.id, run.step_index).await;
    }
}
