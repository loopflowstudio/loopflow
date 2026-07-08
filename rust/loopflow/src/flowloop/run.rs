use std::path::PathBuf;
use std::sync::Arc;

use time::OffsetDateTime;

use crate::lfd::executor::{create_run_for_placement, Placement};
use crate::lfd::id::LfdId;
use crate::lfd::types::{Run, RunStatus};
use crate::lfdb::{open_existing_store, SharedStore};
use crate::ops::{OpsError, OpsResult};

/// A flowloop's registry-backed run: a fresh worktree plus the tokio
/// runtime and store handle it needs to update the run as the flowloop
/// progresses. Every looped flow shares this lifecycle.
pub(crate) struct FlowloopRun {
    pub runtime: tokio::runtime::Runtime,
    pub store: SharedStore,
    pub run: Run,
}

impl FlowloopRun {
    /// Resolve the wave, create a fresh worktree, and register the run.
    pub fn start(wave_name: &str, flow: &str, task: String) -> OpsResult<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|err| OpsError::Message(format!("failed to build flowloop runtime: {err}")))?;
        let store: SharedStore = Arc::new(runtime.block_on(async {
            open_existing_store().await.ok_or_else(|| {
                OpsError::Message(
                    "no run registry on this machine - start or register the wave first"
                        .to_string(),
                )
            })
        })?);

        let run = runtime.block_on(async {
            let wave = store
                .get_wave_by_name(wave_name)
                .await
                .map_err(|err| OpsError::Message(format!("failed to read wave registry: {err}")))?
                .ok_or_else(|| OpsError::Message(format!("wave '{wave_name}' not found")))?;
            let run_id = LfdId::new();
            let mut run = create_run_for_placement(&store, &wave, &run_id, &Placement::Fresh, None)
                .await
                .map_err(|err| {
                    OpsError::Message(format!("failed to create flowloop worktree: {err}"))
                })?;
            run.flow = flow.to_string();
            run.task = Some(task);
            store.update_run(&run).await.map_err(|err| {
                OpsError::Message(format!("failed to update flowloop run: {err}"))
            })?;
            Ok::<Run, OpsError>(run)
        })?;

        Ok(Self {
            runtime,
            store,
            run,
        })
    }

    pub fn worktree(&self) -> PathBuf {
        PathBuf::from(&self.run.worktree)
    }

    /// Record the loop's outcome on the run, then return it unchanged.
    pub fn finish(mut self, result: OpsResult<()>) -> OpsResult<()> {
        self.runtime.block_on(async {
            self.run.status = if result.is_ok() {
                RunStatus::Completed
            } else {
                RunStatus::Failed
            };
            self.run.ended_at = Some(OffsetDateTime::now_utc());
            self.run.error = result.as_ref().err().map(ToString::to_string);
            self.store
                .update_run(&self.run)
                .await
                .map_err(|err| OpsError::Message(format!("failed to finish flowloop run: {err}")))
        })?;
        result
    }
}
