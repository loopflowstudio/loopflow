//! Pools, on-demands and rentals. Admission counts live rentals and writes the
//! new one inside one immediate transaction, so two rents never both pass the
//! machine limit.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use time::OffsetDateTime;

use crate::durable::{MachineId, TaskId, TaskOwnership};
use crate::rental::{
    self, Hardware, OnDemand, Ownership, Pool, PoolMachine, Provider, Rental, RentalId,
    RentalState, Usd,
};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn at(seconds: i64) -> StoreResult<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp(seconds).map_err(invalid)
}

fn usd(text: String) -> StoreResult<Usd> {
    Usd::parse(&text).map_err(invalid)
}

fn name_taken_in(conn: &Connection, name: &str) -> StoreResult<bool> {
    let document = crate::config::placement::read(conn, None).map_err(invalid)?;
    Ok(["pools", "ondemands"].iter().any(|key| {
        document
            .get(key)
            .and_then(|value| value.get(name))
            .is_some()
    }))
}

fn require_name(name: &str) -> StoreResult<()> {
    if name.trim().is_empty() || name.trim() != name {
        return Err(invalid("name must be nonempty without surrounding spaces"));
    }
    Ok(())
}

fn declared_pools(conn: &Connection) -> StoreResult<Vec<Pool>> {
    crate::config::placement::global_settings(conn)
        .map_err(invalid)?
        .pools
        .into_iter()
        .map(|(name, declaration)| {
            let created_at = conn
                .query_row(
                    "SELECT created_at FROM pools WHERE name=?1",
                    [&name],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(0);
            Ok(Pool {
                name,
                off: declaration.off,
                created_at: at(created_at)?,
            })
        })
        .collect()
}

fn declared_ondemands(conn: &Connection) -> StoreResult<Vec<OnDemand>> {
    crate::config::placement::global_settings(conn)
        .map_err(invalid)?
        .ondemands
        .into_iter()
        .map(|(name, declaration)| {
            let created_at = conn
                .query_row(
                    "SELECT created_at FROM ondemands WHERE name=?1",
                    [&name],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(0);
            Ok(OnDemand {
                name,
                provider: declaration.provider,
                hardware: declaration.hardware,
                max_machines: declaration.max_machines,
                max_usd_per_hour: declaration.max_usd_per_hour,
                hours: declaration.hours,
                created_at: at(created_at)?,
            })
        })
        .collect()
}

const RENTAL_COLUMNS: &str = "id,ondemand,provider,provider_name,provider_id,machine_id,by_hand,
    hardware,usd_per_hour,cap_usd,deadline,state,created_at,ended_at,cost_usd";

fn map_rental(row: &rusqlite::Row<'_>) -> StoreResult<Rental> {
    let provider: String = row.get(2)?;
    let hardware: String = row.get(7)?;
    let state: String = row.get(11)?;
    Ok(Rental {
        id: RentalId::from_raw(row.get::<_, String>(0)?),
        ondemand: row.get(1)?,
        provider: Provider::parse(&provider)
            .ok_or_else(|| invalid(format!("unknown provider {provider}")))?,
        provider_name: row.get(3)?,
        provider_id: row.get(4)?,
        machine_id: row.get::<_, Option<String>>(5)?.map(MachineId::from_raw),
        by_hand: row.get(6)?,
        hardware: serde_json::from_str::<Hardware>(&hardware)?,
        usd_per_hour: usd(row.get(8)?)?,
        cap_usd: usd(row.get(9)?)?,
        deadline: at(row.get(10)?)?,
        state: RentalState::parse(&state)
            .ok_or_else(|| invalid(format!("unknown rental state {state}")))?,
        created_at: at(row.get(12)?)?,
        ended_at: row.get::<_, Option<i64>>(13)?.map(at).transpose()?,
        cost_usd: row.get::<_, Option<String>>(14)?.map(usd).transpose()?,
    })
}

fn collect<T>(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    map: fn(&rusqlite::Row<'_>) -> StoreResult<T>,
) -> StoreResult<Vec<T>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map(params, |row| Ok(map(row)))?;
    rows.map(|row| row?).collect()
}

fn rental_in(conn: &Connection, id: &RentalId) -> StoreResult<Rental> {
    conn.query_row(
        &format!("SELECT {RENTAL_COLUMNS} FROM rentals WHERE id=?1"),
        [id.as_str()],
        |row| Ok(map_rental(row)),
    )
    .optional()?
    .ok_or(StoreError::NotFound)?
}

/// Unfinished Tasks whose current owner is this Machine, each with the
/// ownership that says so.
pub(super) fn tasks_held_in(
    conn: &Connection,
    machine: &MachineId,
) -> StoreResult<Vec<(TaskId, TaskOwnership)>> {
    let candidates = {
        let mut statement = conn.prepare(
            "SELECT DISTINCT o.task_id FROM task_ownership o JOIN tasks t ON t.id=o.task_id
             WHERE o.machine_id=?1 AND t.planning_completed=0 AND t.abandoned_at IS NULL
             ORDER BY o.task_id",
        )?;
        let rows = statement.query_map([machine.as_str()], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let mut held = Vec::new();
    for task in candidates {
        let task = TaskId::from_raw(task);
        let ownership = super::task_ownership::ownership_in(conn, &task)?;
        if ownership
            .owner
            .as_ref()
            .and_then(|owner| owner.machine.id.as_ref())
            == Some(machine)
        {
            held.push((task, ownership));
        }
    }
    Ok(held)
}

fn task_ids(held: Vec<(TaskId, TaskOwnership)>) -> Vec<TaskId> {
    held.into_iter().map(|(task, _)| task).collect()
}

impl SqliteStore {
    // -- Pools -------------------------------------------------------------

    pub fn add_pool(&self, name: &str) -> StoreResult<Pool> {
        require_name(name)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        if name_taken_in(&conn, name)? {
            return Err(invalid(format!("{name} already names a pool or on-demand")));
        }
        crate::config::placement::write(
            &conn,
            None,
            &["pools", name],
            Some(
                serde_yaml_ng::to_value(crate::config::placement::PoolDeclaration { off: false })
                    .map_err(invalid)?,
            ),
        )
        .map_err(invalid)?;
        conn.execute(
            "INSERT OR IGNORE INTO pools(name,created_at) VALUES(?1,?2)",
            params![name, now_unix()],
        )?;
        declared_pools(&conn)?
            .into_iter()
            .find(|pool| pool.name == name)
            .ok_or(StoreError::NotFound)
    }

    pub fn pools(&self) -> StoreResult<Vec<Pool>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        declared_pools(&conn)
    }

    pub fn pool(&self, name: &str) -> StoreResult<Option<Pool>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(declared_pools(&conn)?
            .into_iter()
            .find(|pool| pool.name == name))
    }

    pub fn set_pool_off(&self, name: &str, off: bool) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if !declared_pools(&conn)?.iter().any(|pool| pool.name == name) {
            return Err(StoreError::NotFound);
        }
        crate::config::placement::write(&conn, None, &["pools", name, "off"], Some(off.into()))
            .map_err(invalid)?;
        Ok(())
    }

    /// Forget a pool. Its machines stay added, now in no pool.
    pub fn remove_pool(&self, name: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !crate::config::placement::write(&tx, None, &["pools", name], None).map_err(invalid)? {
            return Err(StoreError::NotFound);
        }
        tx.execute("UPDATE pool_machines SET pool=NULL WHERE pool=?1", [name])?;
        tx.execute("DELETE FROM pools WHERE name=?1", [name])?;
        tx.commit()?;
        Ok(())
    }

    /// The pool's labeled machines with the unfinished Tasks each owns.
    pub fn pool_machines(&self, name: &str) -> StoreResult<Vec<PoolMachine>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let ids = {
            let mut statement = snapshot.prepare(
                "SELECT machine_id FROM pool_machines WHERE pool=?1 ORDER BY machine_id",
            )?;
            let rows = statement.query_map([name], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        ids.into_iter()
            .map(|id| {
                let id = MachineId::from_raw(id);
                let machine = super::durable::map_machine_by_id(&snapshot, &id)?
                    .ok_or(StoreError::NotFound)?;
                Ok(PoolMachine {
                    ownership: ownership_in(&snapshot, &id)?,
                    tasks: task_ids(tasks_held_in(&snapshot, &id)?),
                    machine,
                })
            })
            .collect()
    }

    // -- Owned machines ------------------------------------------------------

    pub fn set_machine_ownership(
        &self,
        machine: &MachineId,
        ownership: &Ownership,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let rented: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM rentals WHERE machine_id=?1)",
            [machine.as_str()],
            |row| row.get(0),
        )?;
        if rented {
            return Err(invalid("a rented machine has no pool, cost or expiry"));
        }
        if let Some(pool) = &ownership.pool {
            if !declared_pools(&tx)?
                .iter()
                .any(|candidate| &candidate.name == pool)
            {
                return Err(StoreError::NotFound);
            }
            tx.execute(
                "INSERT OR IGNORE INTO pools(name,created_at) VALUES(?1,?2)",
                params![pool, now_unix()],
            )?;
        }
        let known: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM machines WHERE id=?1)",
            [machine.as_str()],
            |row| row.get(0),
        )?;
        if !known {
            return Err(StoreError::NotFound);
        }
        tx.execute(
            "INSERT INTO pool_machines(machine_id,pool,cost_usd,expires_at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(machine_id) DO UPDATE SET pool=excluded.pool,
                 cost_usd=excluded.cost_usd,expires_at=excluded.expires_at",
            params![
                machine.as_str(),
                ownership.pool,
                ownership.cost_usd.to_decimal(),
                ownership.expires_at.map(OffsetDateTime::unix_timestamp)
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Whether any Task was ever handed to or claimed on this Machine.
    pub fn machine_ever_held(&self, machine: &MachineId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_ownership WHERE machine_id=?1)",
            [machine.as_str()],
            |row| row.get(0),
        )?)
    }

    /// When the latest Task ever handed to or claimed on this Machine was
    /// completed or abandoned.
    pub fn work_finished_at(&self, machine: &MachineId) -> StoreResult<Option<OffsetDateTime>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT DISTINCT t.id,t.planning_completed_at,t.abandoned_at
             FROM task_ownership o JOIN tasks t ON t.id=o.task_id WHERE o.machine_id=?1",
        )?;
        let rows = statement.query_map([machine.as_str()], |row| {
            Ok((
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })?;
        let mut latest = None;
        for row in rows {
            let finished = match row? {
                (_, Some(abandoned)) => at(abandoned)?,
                (Some(completed), None) => OffsetDateTime::parse(
                    &completed,
                    &time::format_description::well_known::Rfc3339,
                )
                .map_err(invalid)?,
                (None, None) => continue,
            };
            latest = latest.max(Some(finished));
        }
        Ok(latest)
    }

    /// Unfinished Tasks `machine` owns, each with when its claim reached it.
    pub fn tasks_held_by(&self, machine: &MachineId) -> StoreResult<Vec<(TaskId, i64)>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        tasks_held_in(&snapshot, machine)?
            .into_iter()
            .map(|(task, ownership)| {
                let since = ownership.since(machine).ok_or_else(|| {
                    StoreError::InvalidData(format!(
                        "Task {task} is owned on {machine} with no log entry naming it"
                    ))
                })?;
                Ok((task, since))
            })
            .collect()
    }

    // -- On-demands ----------------------------------------------------------

    pub fn add_ondemand(&self, ondemand: &OnDemand) -> StoreResult<()> {
        require_name(&ondemand.name)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        if name_taken_in(&conn, &ondemand.name)? {
            return Err(invalid(format!(
                "{} already names a pool or on-demand",
                ondemand.name
            )));
        }
        let declaration = crate::config::placement::OnDemandDeclaration {
            provider: ondemand.provider,
            hardware: ondemand.hardware.clone(),
            max_machines: ondemand.max_machines,
            max_usd_per_hour: ondemand.max_usd_per_hour,
            hours: ondemand.hours,
        };
        crate::config::placement::write(
            &conn,
            None,
            &["ondemands", &ondemand.name],
            Some(serde_yaml_ng::to_value(declaration).map_err(invalid)?),
        )
        .map_err(invalid)?;
        conn.execute(
            "INSERT OR IGNORE INTO ondemands(name,created_at) VALUES(?1,?2)",
            params![ondemand.name, ondemand.created_at.unix_timestamp()],
        )?;
        Ok(())
    }

    pub fn ondemands(&self) -> StoreResult<Vec<OnDemand>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        declared_ondemands(&conn)
    }

    pub fn ondemand(&self, name: &str) -> StoreResult<Option<OnDemand>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        ondemand_in(&conn, name)
    }

    /// Change the limit; current machines stay.
    pub fn set_ondemand_max(&self, name: &str, max: u32) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if ondemand_in(&conn, name)?.is_none() {
            return Err(StoreError::NotFound);
        }
        crate::config::placement::write(
            &conn,
            None,
            &["ondemands", name, "max_machines"],
            Some(max.into()),
        )
        .map_err(invalid)?;
        Ok(())
    }

    /// Forget an on-demand; its rentals' history stays.
    pub fn remove_ondemand(&self, name: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let live = live_in(&conn, name)?;
        if live > 0 {
            return Err(invalid(format!(
                "{name} still has {live} live machine(s); `lf machine end` gives them back"
            )));
        }
        if !crate::config::placement::write(&conn, None, &["ondemands", name], None)
            .map_err(invalid)?
        {
            return Err(StoreError::NotFound);
        }
        conn.execute("DELETE FROM ondemands WHERE name=?1", [name])?;
        Ok(())
    }

    // -- Rentals -------------------------------------------------------------

    /// Admit and record a rental as `requested`, before the provider is asked.
    pub fn admit_rental(
        &self,
        ondemand: &str,
        hours: Option<f64>,
        max_usd: Option<Usd>,
        by_hand: bool,
    ) -> StoreResult<Rental> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ondemand = ondemand_in(&tx, ondemand)?.ok_or(StoreError::NotFound)?;
        let live = live_in(&tx, &ondemand.name)?;
        let now = at(now_unix())?;
        let admission = rental::admit(&ondemand, live, hours, max_usd, now).map_err(invalid)?;
        let id = RentalId::new();
        tx.execute(
            "INSERT INTO rentals(id,ondemand,provider,provider_name,by_hand,hardware,usd_per_hour,
                cap_usd,deadline,state,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'requested',?10)",
            params![
                id.as_str(),
                ondemand.name,
                ondemand.provider.as_str(),
                Rental::provider_name_for(&id),
                by_hand,
                serde_json::to_string(&admission.hardware)?,
                admission.usd_per_hour.to_decimal(),
                admission.cap_usd.to_decimal(),
                admission.deadline.unix_timestamp(),
                now.unix_timestamp()
            ],
        )?;
        let rental = rental_in(&tx, &id)?;
        tx.commit()?;
        Ok(rental)
    }

    /// Rentals of this on-demand that count against its limit.
    pub fn live_rentals(&self, ondemand: &str) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        live_in(&conn, ondemand)
    }

    pub fn rental(&self, id: &RentalId) -> StoreResult<Rental> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        rental_in(&conn, id)
    }

    /// Every rental, newest first; only one on-demand's when named.
    pub fn rentals(&self, ondemand: Option<&str>) -> StoreResult<Vec<Rental>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        collect(
            &conn,
            &format!(
                "SELECT {RENTAL_COLUMNS} FROM rentals WHERE ?1 IS NULL OR ondemand=?1
                 ORDER BY created_at DESC, id"
            ),
            [ondemand],
            map_rental,
        )
    }

    pub fn rental_for_machine(&self, machine: &MachineId) -> StoreResult<Option<Rental>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("SELECT {RENTAL_COLUMNS} FROM rentals WHERE machine_id=?1"),
            [machine.as_str()],
            |row| Ok(map_rental(row)),
        )
        .optional()?
        .transpose()
    }

    /// The provider confirmed (or the inventory found) the pod.
    pub fn confirm_rental(
        &self,
        id: &RentalId,
        provider_id: &str,
        usd_per_hour: Usd,
    ) -> StoreResult<Rental> {
        self.update_rental(
            id,
            "UPDATE rentals SET provider_id=?2,usd_per_hour=?3,state='booting'
             WHERE id=?1 AND state IN ('requested','unconfirmed')",
            params![id.as_str(), provider_id, usd_per_hour.to_decimal()],
        )
    }

    /// Once the rental is no longer live, Tasks its Machine still owns come back
    /// to this Machine in the same write, so none forwards to a gone pod;
    /// returns them.
    pub fn set_rental_state(
        &self,
        id: &RentalId,
        state: RentalState,
    ) -> StoreResult<(Rental, Vec<TaskId>)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ended = (!state.is_live()).then(now_unix);
        tx.execute(
            "UPDATE rentals SET state=?2,ended_at=COALESCE(ended_at,?3) WHERE id=?1",
            params![id.as_str(), state.as_str(), ended],
        )?;
        let rental = rental_in(&tx, id)?;
        let returned = match (&rental.machine_id, state.is_live()) {
            (Some(machine), false) => super::task_ownership::hand_back_in(&tx, machine)?,
            _ => Vec::new(),
        };
        tx.commit()?;
        Ok((rental, returned))
    }

    /// Reachable and added: the rental is running as this Machine.
    pub fn attach_rental(&self, id: &RentalId, machine: &MachineId) -> StoreResult<Rental> {
        self.update_rental(
            id,
            "UPDATE rentals SET machine_id=?2,state='running' WHERE id=?1 AND state='booting'",
            params![id.as_str(), machine.as_str()],
        )
    }

    pub fn set_rental_cost(&self, id: &RentalId, cost: Usd) -> StoreResult<Rental> {
        self.update_rental(
            id,
            "UPDATE rentals SET cost_usd=?2 WHERE id=?1",
            params![id.as_str(), cost.to_decimal()],
        )
    }

    fn update_rental(
        &self,
        id: &RentalId,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> StoreResult<Rental> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(sql, params)? == 0 {
            let current = rental_in(&conn, id)?;
            return Err(invalid(format!(
                "rental {id} is {}; that change does not apply",
                current.state
            )));
        }
        rental_in(&conn, id)
    }
}

fn ondemand_in(conn: &Connection, name: &str) -> StoreResult<Option<OnDemand>> {
    Ok(declared_ondemands(conn)?
        .into_iter()
        .find(|ondemand| ondemand.name == name))
}

fn live_in(conn: &Connection, ondemand: &str) -> StoreResult<u32> {
    Ok(conn.query_row(
        "SELECT count(*) FROM rentals WHERE ondemand=?1 AND state NOT IN ('ended','lost')",
        [ondemand],
        |row| row.get(0),
    )?)
}

fn ownership_in(conn: &Connection, machine: &MachineId) -> StoreResult<Ownership> {
    let (pool, cost, expires): (Option<String>, Option<String>, Option<i64>) = conn
        .query_row(
            "SELECT p.pool,p.cost_usd,p.expires_at FROM machines m
             LEFT JOIN pool_machines p ON p.machine_id=m.id WHERE m.id=?1",
            [machine.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    Ok(Ownership {
        pool,
        cost_usd: cost.map(usd).transpose()?.unwrap_or(Usd::ZERO),
        expires_at: expires.map(at).transpose()?,
    })
}

#[cfg(test)]
#[path = "rentals_tests.rs"]
mod tests;
