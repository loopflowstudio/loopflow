//! Cleanup scheduling is a receipt projection, never deletion authority.
use std::path::{Path, PathBuf};

use super::{
    new_receipt, parse_schedule, receipt_root, CronHost, CronOutcome, CronReceipt, CronSource,
    CronSpec, CronTargetKind,
};
use crate::ops::wt::cleanup::{io, CleanupProgress};
use crate::ops::{OpsError, OpsResult};
use crate::store::sqlite::SqliteStore;

#[derive(Debug)]
pub(crate) struct CleanupReceipt {
    root: PathBuf,
    receipt: CronReceipt,
    writable: bool,
}

impl CleanupReceipt {
    pub(crate) fn begin(store: &SqliteStore, repo: &Path) -> OpsResult<Self> {
        let (machine_id, home): (crate::durable::MachineId, PathBuf) = io::read(
            io::Read::ReceiptContext(store.path().map_err(|e| OpsError::Message(e.to_string()))?),
        )?;
        let spec = CronSpec {
            wave: String::new(),
            flow: format!("{}-cleanup", super::repository_cron_key(repo, &machine_id)),
            target_kind: CronTargetKind::Repository,
            schedule: parse_schedule("every-minute")?,
            working_directory: repo.to_path_buf(),
            lf_path: std::env::current_exe()?,
            host: CronHost {
                machine_id,
                lf_home: home,
                path_env: String::new(),
            },
        };
        let root = receipt_root(&spec.host.lf_home);
        let receipts: Vec<CronReceipt> = io::read(io::Read::Receipts {
            root: root.clone(),
            flow: spec.flow.clone(),
        })?;
        let prior = receipts
            .into_iter()
            .filter_map(|receipt| receipt.cleanup)
            .max_by_key(|progress| progress.sequence);
        let mut progress = prior.unwrap_or_else(CleanupProgress::initial);
        progress.sequence += 1;
        let mut receipt = new_receipt(&spec, &spec.host.machine_id, CronSource::Manual);
        receipt.cleanup = Some(progress);
        io::schedule(io::Schedule::Receipt {
            root: root.clone(),
            receipt: Box::new(receipt.clone()),
        })?;
        io::schedule(io::Schedule::Prune {
            root: root.clone(),
            flow: spec.flow,
            current: receipt.id.clone(),
        })?;
        Ok(Self {
            root,
            receipt,
            writable: true,
        })
    }

    pub(crate) fn progress(&self) -> CleanupProgress {
        self.receipt
            .cleanup
            .clone()
            .expect("cleanup receipt has progress")
    }

    pub(crate) fn save(&mut self, progress: CleanupProgress) -> OpsResult<()> {
        // A failed write may have committed. Never race a timed-out writer with
        // another update to this receipt; the next pass creates a new receipt.
        if !self.writable {
            return Err(OpsError::Message(
                "cleanup receipt write interrupted; retry next pass".into(),
            ));
        }
        self.receipt.cleanup = Some(progress);
        let result = io::schedule(io::Schedule::Receipt {
            root: self.root.clone(),
            receipt: Box::new(self.receipt.clone()),
        });
        self.writable = result.is_ok();
        result
    }

    pub(crate) fn finish(
        &mut self,
        progress: CleanupProgress,
        error: Option<String>,
    ) -> OpsResult<()> {
        let error = error.or_else(|| {
            (progress.failed > 0).then(|| format!("{} cleanup operations failed", progress.failed))
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
