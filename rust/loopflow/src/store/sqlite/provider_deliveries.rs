//! SQLite persistence for the provider delivery inbox (`provider_deliveries`).
//!
//! The inbox is the ingress gate for `lfd`: it deduplicates *deliveries* by
//! `(delivery_id, provider)` so a Linear retry or a post-restart redelivery
//! processes at most once. The domain tables deduplicate *events*; this table
//! deduplicates the wire. See `migrations/0.11.021_provider_deliveries.sql`.

use rusqlite::params;

use crate::store::provider_deliveries::{DeliveryCompletion, DeliveryRecord, DeliveryStatus};
use crate::store::{StoreError, StoreResult};

impl super::SqliteStore {
    /// Record the arrival of a delivery. `INSERT OR IGNORE` against the
    /// `(delivery_id, provider)` primary key is the dedup gate: a fresh delivery
    /// inserts and returns `inserted = true`; a retry returns the existing row's
    /// status so the caller can decide whether to re-process.
    ///
    /// A duplicate left `pending` means a prior attempt crashed mid-flight — the
    /// caller re-processes. A duplicate in any terminal state is a true
    /// duplicate and is dropped.
    pub fn record_delivery(
        &self,
        delivery_id: &str,
        provider: &str,
        event_kind: Option<&str>,
        received_at: i64,
    ) -> StoreResult<DeliveryRecord> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO provider_deliveries
                (delivery_id, provider, event_kind, status, received_at)
             VALUES (?1, ?2, ?3, 'pending', ?4)",
            params![delivery_id, provider, event_kind, received_at],
        )? == 1;
        if inserted {
            return Ok(DeliveryRecord {
                inserted: true,
                existing_status: None,
            });
        }
        let status: String = conn.query_row(
            "SELECT status FROM provider_deliveries
             WHERE delivery_id = ?1 AND provider = ?2",
            params![delivery_id, provider],
            |row| row.get(0),
        )?;
        Ok(DeliveryRecord {
            inserted: false,
            existing_status: Some(DeliveryStatus::from_db(&status)),
        })
    }

    /// Stamp the processing outcome onto a delivery row. Called after
    /// `webhook::ingest_event` returns; the row transitions out of `pending`.
    pub fn complete_delivery(&self, completion: &DeliveryCompletion) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE provider_deliveries
             SET status = ?3, target_kind = ?4, target_id = ?5,
                 outcome = ?6, processed_at = ?7
             WHERE delivery_id = ?1 AND provider = ?2",
            params![
                completion.delivery_id,
                completion.provider,
                completion.status.as_str(),
                completion.target_kind,
                completion.target_id,
                completion.outcome,
                completion.processed_at
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Total deliveries recorded, for the `/status` ops endpoint.
    pub fn delivery_count(&self) -> StoreResult<i64> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(
            conn.query_row("SELECT COUNT(*) FROM provider_deliveries", [], |row| {
                row.get(0)
            })?,
        )
    }
}
