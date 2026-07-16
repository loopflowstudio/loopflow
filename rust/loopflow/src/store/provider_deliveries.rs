//! Delivery inbox types and async store wrappers for `lfd` ingress.
//!
//! The durable inbox lives in `provider_deliveries`; these are the typed
//! handles and the async boundary over the sync SQLite methods in
//! `store/sqlite/provider_deliveries.rs`.

use crate::store::{run_sqlite, Store, StoreResult};

/// The kind of provider event a delivery carried, recorded for ops inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryEventKind {
    IssueEdit,
    Comment,
    Ignored,
}

impl DeliveryEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IssueEdit => "issue_edit",
            Self::Comment => "comment",
            Self::Ignored => "ignored",
        }
    }
}

/// The outcome state of a delivery row. `pending` means processing started but
/// has not yet stamped a terminal state — a redelivery in that state re-processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    Pending,
    Processed,
    Ignored,
    NoTarget,
    Error,
}

impl DeliveryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processed => "processed",
            Self::Ignored => "ignored",
            Self::NoTarget => "no_target",
            Self::Error => "error",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "processed" => Self::Processed,
            "ignored" => Self::Ignored,
            "no_target" => Self::NoTarget,
            "error" => Self::Error,
            _ => Self::Pending,
        }
    }
}

/// What `record_delivery` found: a freshly inserted row, or a duplicate whose
/// prior status decides whether to re-process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryRecord {
    /// `true` when this delivery was newly recorded.
    pub inserted: bool,
    /// The existing row's status when `inserted` is false. `Some(Pending)` means
    /// a prior attempt crashed mid-flight — re-process. A terminal status is a
    /// true duplicate.
    pub existing_status: Option<DeliveryStatus>,
}

/// The terminal outcome to stamp onto a delivery row, bundled so the call site
/// reads as one write rather than seven loose arguments.
#[derive(Debug, Clone)]
pub struct DeliveryCompletion {
    pub delivery_id: String,
    pub provider: String,
    pub status: DeliveryStatus,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub outcome: Option<String>,
    pub processed_at: i64,
}

impl Store {
    /// Record a delivery's arrival. See [`DeliveryRecord`] for the dedup contract.
    pub async fn record_delivery(
        &self,
        delivery_id: String,
        provider: String,
        event_kind: Option<String>,
        received_at: i64,
    ) -> StoreResult<DeliveryRecord> {
        run_sqlite(&self.sqlite, move |store| {
            store.record_delivery(&delivery_id, &provider, event_kind.as_deref(), received_at)
        })
        .await
    }

    /// Stamp the processing outcome onto a delivery row.
    pub async fn complete_delivery(&self, completion: DeliveryCompletion) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| {
            store.complete_delivery(&completion)
        })
        .await
    }

    /// Total deliveries recorded.
    pub async fn delivery_count(&self) -> StoreResult<i64> {
        run_sqlite(&self.sqlite, |store| store.delivery_count()).await
    }
}
