//! A selection's saved inputs, rechecked atomically before recording its result.
use rusqlite::{params, Connection, TransactionBehavior};

use super::{read_account_limit_row, read_provider_account, SqliteStore, PROVIDER_ACCOUNT_COLUMNS};
use crate::provider_auth::Provider;
use crate::store::rows::now_unix;
use crate::store::{AccountLimitRow, ProviderAccount, ProviderAccountId, StoreError, StoreResult};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AccountSelectionReading {
    provider: Provider,
    candidates: Vec<ProviderAccountId>,
    pub accounts: Vec<ProviderAccount>,
    pub limits: Vec<AccountLimitRow>,
    newest_selection: i64,
}

fn read_in(
    conn: &Connection,
    provider: Provider,
    candidates: &[ProviderAccountId],
) -> StoreResult<AccountSelectionReading> {
    let ids = super::children::ids_json(candidates.iter().map(ProviderAccountId::as_str));
    let mut statement = conn.prepare(&format!(
        "SELECT {PROVIDER_ACCOUNT_COLUMNS} FROM provider_accounts
         WHERE provider=?1 AND account_id IN (SELECT value FROM json_each(?2))
         ORDER BY account_id"
    ))?;
    let accounts = statement
        .query_map(params![provider.as_str(), ids], |row| {
            Ok(read_provider_account(row))
        })?
        .map(|row| row?)
        .collect::<StoreResult<Vec<_>>>()?;
    let mut statement = conn.prepare(
        "SELECT provider, account_id, window, used_percent, resets_at, plan, observed_at, source, model, limited_until
         FROM provider_account_limits
         WHERE provider=?1 AND account_id IN (SELECT value FROM json_each(?2))
         ORDER BY account_id, window",
    )?;
    let limits = statement
        .query_map(params![provider.as_str(), ids], read_account_limit_row)?
        .map(|row| row?)
        .collect::<StoreResult<Vec<_>>>()?;
    let newest_selection = conn.query_row(
        "SELECT COALESCE(MAX(last_selected_at), 0) FROM provider_accounts WHERE provider=?1",
        [provider.as_str()],
        |row| row.get(0),
    )?;
    Ok(AccountSelectionReading {
        provider,
        candidates: candidates.to_vec(),
        accounts,
        limits,
        newest_selection,
    })
}

impl SqliteStore {
    pub(crate) fn account_selection_reading(
        &self,
        provider: Provider,
        candidates: &[ProviderAccountId],
    ) -> StoreResult<AccountSelectionReading> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let reading = read_in(&tx, provider, candidates)?;
        tx.commit()?;
        Ok(reading)
    }

    /// `None` means the saved inputs changed: choose again from a fresh reading.
    /// An unchanged empty choice commits without writing a selection.
    pub(crate) fn record_account_selection(
        &self,
        reading: &AccountSelectionReading,
        selected: Option<&ProviderAccountId>,
    ) -> StoreResult<Option<i64>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        if selected.is_none() {
            let tx = conn.savepoint()?;
            let unchanged = read_in(&tx, reading.provider, &reading.candidates)? == *reading;
            tx.commit()?;
            return Ok(unchanged.then(|| now_unix().max(reading.newest_selection + 1)));
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if read_in(&tx, reading.provider, &reading.candidates)? != *reading {
            return Ok(None);
        }
        let at = now_unix().max(reading.newest_selection + 1);
        if let Some(selected) = selected {
            if !reading
                .accounts
                .iter()
                .any(|account| account.account_id == *selected)
            {
                return Err(StoreError::InvalidData(
                    "selected account is absent from the reading".into(),
                ));
            }
            tx.execute(
                "UPDATE provider_accounts SET last_selected_at=?3, updated_at=?3
                 WHERE provider=?1 AND account_id=?2",
                params![reading.provider.as_str(), selected.as_str(), at],
            )?;
        }
        tx.commit()?;
        Ok(Some(at))
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use crate::provider_auth::Provider;
    use crate::store::{AccountLimitWindow, ProviderAccountId};

    fn seed(store: &SqliteStore, id: &str) -> ProviderAccountId {
        let id = ProviderAccountId::parse(id).unwrap();
        store.conn.lock().unwrap().execute(
            "INSERT INTO provider_accounts(provider,account_id,credential_state,created_at,updated_at,last_selected_at)
             VALUES('claude',?1,'connected',1,1,7)", [id.as_str()]
        ).unwrap();
        id
    }

    #[test]
    fn a_changed_other_candidate_invalidates_the_choice_and_an_empty_choice() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = crate::store::opening::open_ephemeral(&path).unwrap();
        let writer = crate::store::opening::open_ephemeral(&path).unwrap();
        let first = seed(&store, "first");
        let second = seed(&store, "second");
        let candidates = [first.clone(), second.clone()];
        let reading = store
            .account_selection_reading(Provider::Claude, &candidates)
            .unwrap();
        writer
            .upsert_provider_account_limits(
                "claude",
                &second,
                &[AccountLimitWindow {
                    window: "seven_day".into(),
                    used_percent: 100,
                    resets_at: Some(1_900_000_000),
                    plan: None,
                    model: None,
                    limited_until: Some(1_900_000_005),
                }],
                "stream",
            )
            .unwrap();
        assert_eq!(
            store
                .record_account_selection(&reading, Some(&first))
                .unwrap(),
            None
        );
        assert_eq!(
            store.record_account_selection(&reading, None).unwrap(),
            None
        );
        let fresh = store
            .account_selection_reading(Provider::Claude, &candidates)
            .unwrap();
        assert!(store
            .record_account_selection(&fresh, Some(&first))
            .unwrap()
            .is_some());
        assert_eq!(
            store
                .provider_account_row("claude", &second)
                .unwrap()
                .unwrap()
                .last_selected_at,
            Some(7)
        );
    }

    #[test]
    fn concurrent_launches_recompute_and_keep_selection_times_monotonic() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = crate::store::opening::open_ephemeral(&path).unwrap();
        let writer = crate::store::opening::open_ephemeral(&path).unwrap();
        let first = seed(&store, "first");
        let second = seed(&store, "second");
        let candidates = [first.clone()];
        let reading = store
            .account_selection_reading(Provider::Claude, &candidates)
            .unwrap();
        let competing = writer
            .account_selection_reading(Provider::Claude, std::slice::from_ref(&second))
            .unwrap();
        let at = writer
            .record_account_selection(&competing, Some(&second))
            .unwrap()
            .unwrap();
        assert_eq!(
            store
                .record_account_selection(&reading, Some(&first))
                .unwrap(),
            None
        );
        let fresh = store
            .account_selection_reading(Provider::Claude, &candidates)
            .unwrap();
        assert!(
            store
                .record_account_selection(&fresh, Some(&first))
                .unwrap()
                .unwrap()
                > at
        );
    }

    #[test]
    fn an_arriving_candidate_invalidates_an_empty_reading_inside_a_pinned_snapshot() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let candidate = ProviderAccountId::parse("late").unwrap();
        let reading = {
            let _snapshot = store.read_snapshot().unwrap();
            store
                .account_selection_reading(Provider::Claude, std::slice::from_ref(&candidate))
                .unwrap()
        };
        assert!(reading.accounts.is_empty());
        seed(&store, "late");
        assert_eq!(
            store.record_account_selection(&reading, None).unwrap(),
            None
        );
    }

    #[test]
    fn selection_reads_use_indexed_lookups_at_machine_scale() {
        use crate::store::query_plans::statements;

        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.conn.lock().unwrap().execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<10000)
             INSERT INTO provider_accounts(provider,account_id,credential_state,created_at,updated_at)
             SELECT 'claude',printf('account-%05d',i),'connected',1,1 FROM n;
             INSERT INTO provider_account_limits(provider,account_id,window,used_percent,observed_at,source)
             SELECT provider,account_id,'seven_day',20,1,'poll' FROM provider_accounts;"
        ).unwrap();
        let candidates = [
            ProviderAccountId::parse("account-00001").unwrap(),
            ProviderAccountId::parse("account-10000").unwrap(),
        ];
        let traced = statements(&store, || {
            let reading = store
                .account_selection_reading(Provider::Claude, &candidates)
                .unwrap();
            assert_eq!(reading.accounts.len(), 2);
            assert_eq!(reading.limits.len(), 2);
            assert!(store
                .record_account_selection(&reading, Some(&candidates[0]))
                .unwrap()
                .is_some());
        });
        let conn = store.conn.lock().unwrap();
        for sql in traced.iter().filter(|sql| sql.starts_with("SELECT")) {
            let mut explain = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let parameters = vec![rusqlite::types::Null; explain.parameter_count()];
            let plan = explain
                .query_map(rusqlite::params_from_iter(parameters), |row| {
                    row.get::<_, String>(3)
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                plan.iter()
                    .any(|line| line.contains("SEARCH") && line.contains("INDEX")),
                "{plan:?}"
            );
            assert!(
                !plan
                    .iter()
                    .any(|line| line.starts_with("SCAN provider_accounts")
                        || line.starts_with("SCAN provider_account_limits")),
                "{plan:?}"
            );
        }
    }

    #[test]
    fn a_current_empty_choice_takes_no_write_lock() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let reading = store
            .account_selection_reading(Provider::Claude, &[])
            .unwrap();
        let outcome = crate::store::query_plans::assert_no_empty_writes(&store, || {
            store.record_account_selection(&reading, None).unwrap()
        });
        assert!(outcome.is_some());
    }
}
