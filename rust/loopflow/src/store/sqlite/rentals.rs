//! Pools, on-demands and rentals. Admission counts live rentals and writes the
//! new one inside one immediate transaction, so two rents never both pass the
//! machine limit.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use time::OffsetDateTime;

use crate::durable::{MachineId, TaskId};
use crate::rental::{
    self, Hardware, OnDemand, Ownership, Pool, PoolMachine, Provider, Quote, Rental, RentalId,
    RentalState, Template, Usd,
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
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pools WHERE name=?1)
             OR EXISTS(SELECT 1 FROM ondemands WHERE name=?1)",
        [name],
        |row| row.get(0),
    )?)
}

fn require_name(name: &str) -> StoreResult<()> {
    if name.trim().is_empty() || name.trim() != name {
        return Err(invalid("name must be nonempty without surrounding spaces"));
    }
    Ok(())
}

const POOL_COLUMNS: &str = "name,off,created_at";

fn map_pool(row: &rusqlite::Row<'_>) -> StoreResult<Pool> {
    Ok(Pool {
        name: row.get(0)?,
        off: row.get(1)?,
        created_at: at(row.get(2)?)?,
    })
}

const ONDEMAND_COLUMNS: &str =
    "name,provider,template,max_machines,max_usd_per_hour,hours,created_at";

fn map_ondemand(row: &rusqlite::Row<'_>) -> StoreResult<OnDemand> {
    let provider: String = row.get(1)?;
    let template: String = row.get(2)?;
    Ok(OnDemand {
        name: row.get(0)?,
        provider: Provider::parse(&provider)
            .ok_or_else(|| invalid(format!("unknown provider {provider}")))?,
        template: serde_json::from_str::<Template>(&template)?,
        max_machines: row.get(3)?,
        max_usd_per_hour: usd(row.get(4)?)?,
        hours: row.get(5)?,
        created_at: at(row.get(6)?)?,
    })
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

/// Unfinished Tasks whose current owner is this Machine.
pub(super) fn tasks_held_in(conn: &Connection, machine: &MachineId) -> StoreResult<Vec<TaskId>> {
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
        if ownership.owner.and_then(|owner| owner.machine.id).as_ref() == Some(machine) {
            held.push(task);
        }
    }
    Ok(held)
}

impl SqliteStore {
    // -- Pools -------------------------------------------------------------

    pub fn add_pool(&self, name: &str) -> StoreResult<Pool> {
        require_name(name)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if name_taken_in(&tx, name)? {
            return Err(invalid(format!("{name} already names a pool or on-demand")));
        }
        tx.execute(
            "INSERT INTO pools(name,off,created_at) VALUES(?1,0,?2)",
            params![name, now_unix()],
        )?;
        let pool = tx.query_row(
            &format!("SELECT {POOL_COLUMNS} FROM pools WHERE name=?1"),
            [name],
            |row| Ok(map_pool(row)),
        )??;
        tx.commit()?;
        Ok(pool)
    }

    pub fn pools(&self) -> StoreResult<Vec<Pool>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        collect(
            &conn,
            &format!("SELECT {POOL_COLUMNS} FROM pools ORDER BY name"),
            [],
            map_pool,
        )
    }

    pub fn pool(&self, name: &str) -> StoreResult<Option<Pool>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("SELECT {POOL_COLUMNS} FROM pools WHERE name=?1"),
            [name],
            |row| Ok(map_pool(row)),
        )
        .optional()?
        .transpose()
    }

    pub fn set_pool_off(&self, name: &str, off: bool) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute("UPDATE pools SET off=?2 WHERE name=?1", params![name, off])? == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Forget a pool. Its machines stay added, now in no pool.
    pub fn remove_pool(&self, name: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE machines SET pool=NULL WHERE pool=?1", [name])?;
        if tx.execute("DELETE FROM pools WHERE name=?1", [name])? == 0 {
            return Err(StoreError::NotFound);
        }
        tx.commit()?;
        Ok(())
    }

    /// The pool's labeled machines with the unfinished Tasks each owns.
    pub fn pool_machines(&self, name: &str) -> StoreResult<Vec<PoolMachine>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let ids = {
            let mut statement = snapshot.prepare(
                "SELECT id FROM machines WHERE pool=?1 AND label IS NOT NULL ORDER BY label",
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
                    tasks: tasks_held_in(&snapshot, &id)?,
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
        let changed = tx.execute(
            "UPDATE machines SET pool=?2,cost_usd=?3,expires_at=?4 WHERE id=?1",
            params![
                machine.as_str(),
                ownership.pool,
                ownership.cost_usd.to_decimal(),
                ownership.expires_at.map(OffsetDateTime::unix_timestamp)
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
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

    pub fn tasks_held_by(&self, machine: &MachineId) -> StoreResult<Vec<TaskId>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        tasks_held_in(&snapshot, machine)
    }

    // -- On-demands ----------------------------------------------------------

    pub fn add_ondemand(&self, ondemand: &OnDemand) -> StoreResult<()> {
        require_name(&ondemand.name)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if name_taken_in(&tx, &ondemand.name)? {
            return Err(invalid(format!(
                "{} already names a pool or on-demand",
                ondemand.name
            )));
        }
        tx.execute(
            "INSERT INTO ondemands(name,provider,template,max_machines,max_usd_per_hour,hours,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                ondemand.name,
                ondemand.provider.as_str(),
                serde_json::to_string(&ondemand.template)?,
                ondemand.max_machines,
                ondemand.max_usd_per_hour.to_decimal(),
                ondemand.hours,
                ondemand.created_at.unix_timestamp()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn ondemands(&self) -> StoreResult<Vec<OnDemand>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        collect(
            &conn,
            &format!("SELECT {ONDEMAND_COLUMNS} FROM ondemands ORDER BY name"),
            [],
            map_ondemand,
        )
    }

    pub fn ondemand(&self, name: &str) -> StoreResult<Option<OnDemand>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        ondemand_in(&conn, name)
    }

    /// Change the limit; current machines stay.
    pub fn set_ondemand_max(&self, name: &str, max: u32) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE ondemands SET max_machines=?2 WHERE name=?1",
            params![name, max],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
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
        if conn.execute("DELETE FROM ondemands WHERE name=?1", [name])? == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    // -- Rentals -------------------------------------------------------------

    /// Admit and record a rental as `requested`, before the provider is asked.
    pub fn admit_rental(
        &self,
        ondemand: &str,
        quote: &Quote,
        hours: Option<f64>,
        max_usd: Option<Usd>,
        by_hand: bool,
    ) -> StoreResult<Rental> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ondemand = ondemand_in(&tx, ondemand)?.ok_or(StoreError::NotFound)?;
        let live = live_in(&tx, &ondemand.name)?;
        let now = at(now_unix())?;
        let admission =
            rental::admit(&ondemand, live, quote, hours, max_usd, now).map_err(invalid)?;
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

    pub fn set_rental_state(&self, id: &RentalId, state: RentalState) -> StoreResult<Rental> {
        if state == RentalState::Ended {
            return Err(invalid(
                "an ending rental hands back its Tasks: set_rental_ended",
            ));
        }
        let ended = (!state.is_live()).then(now_unix);
        self.update_rental(
            id,
            "UPDATE rentals SET state=?2,ended_at=COALESCE(ended_at,?3) WHERE id=?1",
            params![id.as_str(), state.as_str(), ended],
        )
    }

    /// The pod is gone. Tasks its Machine still owns come back to this Machine
    /// in the same write, so none forwards to a deleted pod; returns them.
    pub fn set_rental_ended(&self, id: &RentalId) -> StoreResult<(Rental, Vec<TaskId>)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE rentals SET state='ended',ended_at=COALESCE(ended_at,?2) WHERE id=?1",
            params![id.as_str(), now_unix()],
        )?;
        let rental = rental_in(&tx, id)?;
        let returned = match &rental.machine_id {
            Some(machine) => super::task_ownership::hand_back_in(&tx, machine)?,
            None => Vec::new(),
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
    conn.query_row(
        &format!("SELECT {ONDEMAND_COLUMNS} FROM ondemands WHERE name=?1"),
        [name],
        |row| Ok(map_ondemand(row)),
    )
    .optional()?
    .transpose()
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
            "SELECT pool,cost_usd,expires_at FROM machines WHERE id=?1",
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
