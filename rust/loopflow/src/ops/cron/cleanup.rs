//! Cleanup scheduling is a receipt projection, never deletion authority.
use std::path::{Path, PathBuf};

use super::{
    new_receipt, parse_schedule, prune_minute_receipts, read_receipts, receipt_root, write_receipt,
    CronHost, CronOutcome, CronReceipt, CronSource, CronSpec, CronTargetKind,
};
use crate::ops::wt::cleanup::CleanupProgress;
use crate::ops::{OpsError, OpsResult};
use crate::store::sqlite::SqliteStore;

#[derive(Debug)]
pub(crate) struct CleanupReceipt {
    root: PathBuf,
    receipt: CronReceipt,
}

impl CleanupReceipt {
    pub(crate) fn begin(store: &SqliteStore, repo: &Path) -> OpsResult<Self> {
        let store = store
            .bounded_reader(std::time::Duration::from_secs(2))
            .map_err(|e| OpsError::Message(e.to_string()))?;
        let machine = store
            .local_machine()
            .map_err(|e| OpsError::Message(e.to_string()))?;
        let home = store
            .home_dir()
            .map_err(|e| OpsError::Message(e.to_string()))?;
        let spec = CronSpec {
            wave: String::new(),
            flow: format!("{}-cleanup", super::repository_cron_key(repo, &machine.id)),
            target_kind: CronTargetKind::Repository,
            schedule: parse_schedule("every-minute")?,
            working_directory: repo.to_path_buf(),
            lf_path: std::env::current_exe()?,
            host: CronHost {
                machine_id: machine.id,
                lf_home: home,
                path_env: String::new(),
            },
        };
        let root = receipt_root(&spec.host.lf_home);
        let prior = read_receipts(&root, "", Some(&spec.flow))?
            .into_iter()
            .filter_map(|receipt| receipt.cleanup)
            .max_by_key(|progress| progress.sequence);
        let mut progress = prior.unwrap_or_else(CleanupProgress::initial);
        progress.sequence += 1;
        let mut receipt = new_receipt(&spec, &spec.host.machine_id, CronSource::Manual);
        receipt.cleanup = Some(progress);
        write_receipt(&root, &receipt)?;
        prune_minute_receipts(&root, &spec, &receipt.id)?;
        Ok(Self { root, receipt })
    }

    pub(crate) fn progress(&self) -> CleanupProgress {
        self.receipt
            .cleanup
            .clone()
            .expect("cleanup receipt has progress")
    }

    pub(crate) fn save(&mut self, progress: CleanupProgress) -> OpsResult<()> {
        self.receipt.cleanup = Some(progress);
        write_receipt(&self.root, &self.receipt)
    }

    pub(crate) fn finish(
        &mut self,
        progress: CleanupProgress,
        error: Option<String>,
    ) -> OpsResult<()> {
        let error = error.or_else(|| {
            (progress.failed > 0).then(|| format!("{} checkout removals failed", progress.failed))
        });
        self.receipt.finished_at = Some(chrono::Utc::now().timestamp());
        self.receipt.outcome = if error.is_some() {
            CronOutcome::Failed
        } else {
            CronOutcome::Succeeded
        };
        self.receipt.exit_code = Some(if error.is_some() { 1 } else { 0 });
        self.receipt.error = error;
        self.save(progress)
    }
}
