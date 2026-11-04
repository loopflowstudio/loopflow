//! Device admission is one local grant, separate from Work and OS peer identity.
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::SqliteStore;
use crate::machine_service::admission::{Device, DeviceId};
use crate::store::StoreResult;

impl SqliteStore {
    pub(crate) fn create_machine_pairing(
        &self,
        hash: &[u8; 32],
        name: &str,
        now: i64,
        expires_at: i64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM machine_pairings WHERE expires_at <= ?1", [now])?;
        tx.execute(
            "INSERT INTO machine_pairings(ticket_hash, name, expires_at) VALUES (?1, ?2, ?3)",
            params![hash.as_slice(), name, expires_at],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Consuming the one-use ticket and creating its device are one transaction.
    pub(crate) fn redeem_machine_pairing(
        &self,
        ticket_hash: &[u8; 32],
        token_hash: &[u8; 32],
        id: &DeviceId,
        now: i64,
    ) -> StoreResult<Option<Device>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let name: Option<String> = tx.query_row(
            "DELETE FROM machine_pairings WHERE ticket_hash = ?1 AND expires_at > ?2 RETURNING name",
            params![ticket_hash.as_slice(), now],
            |row| row.get(0),
        ).optional()?;
        let Some(name) = name else { return Ok(None) };
        tx.execute(
            "INSERT INTO machine_devices(id, name, token_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![id.0, name, token_hash.as_slice(), now],
        )?;
        tx.commit()?;
        Ok(Some(Device {
            id: id.clone(),
            name,
            created_at: now,
            revoked_at: None,
        }))
    }

    pub(crate) fn admitted_machine_device(&self, hash: &[u8; 32]) -> StoreResult<Option<DeviceId>> {
        Ok(self
            .conn
            .lock()
            .expect("store mutex poisoned")
            .query_row(
                "SELECT id FROM machine_devices WHERE token_hash = ?1 AND revoked_at IS NULL",
                [hash.as_slice()],
                |row| row.get::<_, String>(0).map(DeviceId),
            )
            .optional()?)
    }

    pub(crate) fn machine_device_active(&self, id: &DeviceId) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT EXISTS(SELECT 1 FROM machine_devices WHERE id = ?1 AND revoked_at IS NULL)",
            [&id.0],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn revoke_machine_device(&self, id: &DeviceId, now: i64) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE machine_devices SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id.0, now],
        )? > 0)
    }

    pub(crate) fn machine_devices(&self) -> StoreResult<Vec<Device>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, name, created_at, revoked_at FROM machine_devices ORDER BY created_at, id",
        )?;
        let devices = statement
            .query_map([], |row| {
                Ok(Device {
                    id: DeviceId(row.get(0)?),
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    revoked_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(devices)
    }
}
