use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::engine::agent::build_agent_command;
use crate::engine::flow::{ConcreteFork, ConcreteItem, ConcreteStep, Step};
use crate::engine::fork::{
    cleanup_fork_worktrees, plan_fork_execution, write_fork_manifest, ForkManifestBranch,
    FORK_SYNTHESIZE_STEP,
};
use crate::engine::worktree::create_worktree;
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
        if self.executor_type == ExecutorType::Docker {
            self.fail_run(
                run,
                wave,
                "fork is not supported by the docker executor yet".to_string(),
            )
            .await?;
            return Ok(());
        }

        let planned = match plan_fork_execution(&fork.branches, &run.snapshot.direction) {
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

        let wave_directions = run.snapshot.direction.clone();
        for execution in fork_runs.iter() {
            let executor = self.clone();
            let scheduler = self.scheduler.clone();
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

                // Write annotation sidecar for fork branch.
                let wave_id_str = fork_wave_id.to_string();
                let ann_context = crate::engine::annotation::write_sidecar(
                    std::path::Path::new(&worktree),
                    crate::engine::annotation::build_wave_envelope(
                        &crate::engine::annotation::WaveEnvelopeParams {
                            step: &step.step.name,
                            flow: "fork",
                            model: &model,
                            directions: &wave_directions,
                            area: None,
                            wave: &wave_id_str,
                            step_index: fork_run.branch_index,
                            total_steps: 0,
                            parent_span_id: None,
                        },
                    ),
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
                        annotation: ann_context,
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

        let mut branch_results: HashMap<String, Result<i32>> = HashMap::new();
        let mut completed = 0usize;
        let total = fork_runs.len();
        debug!(run_id = %run.id, total_branches = total, "waiting for fork results");
        while completed < total {
            let Some((branch_label, result)) = rx.recv().await else {
                break;
            };
            branch_results.insert(branch_label, result);
            completed += 1;
            debug!(run_id = %run.id, completed, total, "fork branch done");
        }
        for handle in handles {
            let _ = handle.await;
        }

        let mut outcomes = Vec::new();
        for execution in &fork_runs {
            let result = branch_results
                .remove(&execution.label)
                .unwrap_or_else(|| Err(anyhow!("fork branch result missing")));
            let exit_code = match result {
                Ok(code) => code,
                Err(err) => {
                    warn!(
                        run_id = %run.id,
                        branch = %execution.label,
                        error = %err,
                        "fork branch errored"
                    );
                    1
                }
            };
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

        let manifest_path = match write_fork_manifest(Path::new(&run.worktree), &outcomes) {
            Ok(path) => path,
            Err(err) => {
                self.cleanup_fork(run, &fork_runs, None).await;
                self.fail_run(run, wave, format!("failed writing fork manifest: {err}"))
                    .await?;
                return Ok(());
            }
        };

        let synth_step = ConcreteStep {
            step: Step::named(FORK_SYNTHESIZE_STEP),
            flow_parents: fork.flow_parents.clone(),
        };
        let synth_exit = match self.run_step(wave, run, &synth_step).await {
            Ok(code) => code,
            Err(err) => {
                self.cleanup_fork(run, &fork_runs, Some(&manifest_path))
                    .await;
                self.fail_run(run, wave, format!("synthesize step failed: {err}"))
                    .await?;
                return Ok(());
            }
        };

        self.cleanup_fork(run, &fork_runs, Some(&manifest_path))
            .await;

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
        self.advance_run_step(run, plan, &wave.id).await?;
        Ok(())
    }

    async fn cleanup_fork(
        &self,
        run: &WaveRun,
        fork_runs: &[ForkBranchExecution],
        manifest_path: Option<&Path>,
    ) {
        let worktrees: Vec<PathBuf> = fork_runs
            .iter()
            .map(|execution| PathBuf::from(&execution.run.worktree))
            .collect();
        cleanup_fork_worktrees(manifest_path, &worktrees);
        let _ = self.store.delete_fork_runs(&run.id, run.step_index).await;
    }
}
