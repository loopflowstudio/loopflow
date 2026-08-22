use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use time::OffsetDateTime;

#[cfg(test)]
use crate::id::WaveId;
use crate::store::{StoreError, StoreResult};
use crate::wave::metrics::{
    MetricContract, MetricIdentity, MetricObservation, MetricObservationEvidence,
    ObservationAcceptance,
};

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn metric_storage_available(&self) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT
                 EXISTS(SELECT 1 FROM sqlite_schema
                        WHERE type='table' AND name='metric_instruments')
                 AND EXISTS(SELECT 1 FROM sqlite_schema
                            WHERE type='table' AND name='metric_observations')",
            [],
            |row| row.get(0),
        )
        .map_err(StoreError::from)
    }

    pub(crate) fn register_metric_instrument(
        &self,
        identity: &MetricIdentity,
        instrument: &str,
        registered_at: OffsetDateTime,
    ) -> StoreResult<()> {
        if instrument.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "metric instrument must not be empty".to_string(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let inserted = conn.execute(
            "INSERT INTO metric_instruments (wave_id, metric_id, instrument, registered_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(wave_id, metric_id) DO NOTHING",
            params![
                identity.wave_id,
                identity.metric_id,
                instrument,
                registered_at.unix_timestamp()
            ],
        )?;
        if inserted == 1 {
            return Ok(());
        }
        let registered = conn.query_row(
            "SELECT instrument FROM metric_instruments
             WHERE wave_id=?1 AND metric_id=?2",
            params![identity.wave_id, identity.metric_id],
            |row| row.get::<_, String>(0),
        )?;
        if registered == instrument {
            Ok(())
        } else {
            Err(StoreError::InvalidData(format!(
                "metric {}/{} is already bound to {:?}, not {:?}",
                identity.wave_id, identity.metric_id, registered, instrument
            )))
        }
    }

    pub(crate) fn accept_metric_observation(
        &self,
        contract: &MetricContract,
        observation: &MetricObservation,
        received_at: OffsetDateTime,
    ) -> StoreResult<ObservationAcceptance> {
        observation
            .validate(contract, received_at)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let payload = serde_json::to_string(observation)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let source_time = observation.source_time().to_offset(time::UtcOffset::UTC);
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let registered = transaction
            .query_row(
                "SELECT instrument FROM metric_instruments
                 WHERE wave_id=?1 AND metric_id=?2",
                params![contract.identity.wave_id, contract.identity.metric_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match registered {
            None => {
                return Err(StoreError::InvalidData(format!(
                    "metric {}/{} has no registered instrument",
                    contract.identity.wave_id, contract.identity.metric_id
                )))
            }
            Some(instrument) if instrument != contract.instrument => {
                return Err(StoreError::InvalidData(format!(
                    "metric {}/{} contract instrument {:?} does not match registered instrument {:?}",
                    contract.identity.wave_id,
                    contract.identity.metric_id,
                    contract.instrument,
                    instrument
                )))
            }
            Some(_) => {}
        }
        let inserted = transaction.execute(
            "INSERT INTO metric_observations (
                observation_id, wave_id, metric_id, contract_revision, instrument,
                source_time_seconds, source_time_nanoseconds, received_at,
                graduation_qualifying, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(observation_id) DO NOTHING",
            params![
                observation.observation_id(),
                observation.identity().wave_id,
                observation.identity().metric_id,
                observation.contract_revision(),
                observation.instrument(),
                source_time.unix_timestamp(),
                source_time.nanosecond(),
                received_at.unix_timestamp(),
                observation.qualifies_graduation(contract),
                payload,
            ],
        )?;
        if inserted == 1 {
            transaction.commit()?;
            return Ok(ObservationAcceptance::Accepted);
        }
        let existing = transaction.query_row(
            "SELECT payload FROM metric_observations WHERE observation_id=?1",
            [observation.observation_id()],
            |row| row.get::<_, String>(0),
        )?;
        transaction.commit()?;
        if existing == payload {
            Ok(ObservationAcceptance::Duplicate)
        } else {
            Err(StoreError::InvalidData(format!(
                "observation id {:?} is already bound to different content",
                observation.observation_id()
            )))
        }
    }

    pub(crate) fn metric_instruments(
        &self,
        identities: &[MetricIdentity],
    ) -> StoreResult<BTreeMap<MetricIdentity, String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut instruments = BTreeMap::new();
        let mut statement = conn.prepare(
            "SELECT instrument FROM metric_instruments
             WHERE wave_id=?1 AND metric_id=?2",
        )?;
        for identity in identities {
            if let Some(instrument) = statement
                .query_row(params![identity.wave_id, identity.metric_id], |row| {
                    row.get::<_, String>(0)
                })
                .optional()?
            {
                instruments.insert(identity.clone(), instrument);
            }
        }
        Ok(instruments)
    }

    pub(crate) fn metric_observation_evidence(
        &self,
        contracts: &[MetricContract],
    ) -> StoreResult<BTreeMap<MetricIdentity, MetricObservationEvidence>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT
                (
                    SELECT payload FROM metric_observations
                    WHERE wave_id=?1 AND metric_id=?2
                    ORDER BY source_time_seconds DESC, source_time_nanoseconds DESC,
                             observation_id DESC
                    LIMIT 1
                ),
                EXISTS(
                    SELECT 1 FROM metric_observations
                    WHERE wave_id=?1 AND metric_id=?2 AND contract_revision=?3
                ),
                EXISTS(
                    SELECT 1 FROM metric_observations
                    WHERE wave_id=?1 AND metric_id=?2 AND contract_revision=?3
                      AND graduation_qualifying=1
                )",
        )?;
        let mut evidence = BTreeMap::new();
        for contract in contracts {
            let identity = &contract.identity;
            let (current, instrumented, graduation_qualified) = statement.query_row(
                params![
                    identity.wave_id,
                    identity.metric_id,
                    contract.contract_revision
                ],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, bool>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                },
            )?;
            let current = current
                .map(|payload| {
                    serde_json::from_str(&payload).map_err(|error| {
                        StoreError::InvalidData(format!(
                            "invalid stored metric observation: {error}"
                        ))
                    })
                })
                .transpose()?;
            evidence.insert(
                identity.clone(),
                MetricObservationEvidence {
                    current,
                    instrumented,
                    graduation_qualified,
                },
            );
        }
        Ok(evidence)
    }

    #[cfg(test)]
    fn metric_observation_count(&self, wave_id: &WaveId) -> StoreResult<usize> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let count = conn.query_row(
            "SELECT COUNT(*) FROM metric_observations WHERE wave_id=?1",
            [wave_id.as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        usize::try_from(count)
            .map_err(|error| StoreError::InvalidData(format!("invalid metric row count: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use tempfile::tempdir;
    use time::Duration;

    use crate::id::WaveId;
    use crate::wave::metrics::{
        MetricContractDefinition, MetricDuration, MetricStage, MetricTarget,
    };
    use crate::wave::Wave;

    use super::*;

    fn contract(identity: MetricIdentity) -> MetricContract {
        MetricContract::new(MetricContractDefinition {
            identity,
            name: "Task loops earn trust".to_string(),
            project_id: "project-1".to_string(),
            stage: MetricStage::Installed,
            instrument: "lifecycle-scorecard".to_string(),
            unit: "ratio".to_string(),
            target: MetricTarget::AtLeast { value: 1.0 },
            window: MetricDuration::parse("7d").unwrap(),
            freshness_policy: MetricDuration::parse("6h").unwrap(),
            body: "Count settled Task loops.".to_string(),
        })
        .unwrap()
    }

    fn observed(contract: &MetricContract, end: OffsetDateTime) -> MetricObservation {
        let mut observation = MetricObservation::Observed {
            identity: contract.identity.clone(),
            contract_revision: contract.contract_revision.clone(),
            instrument: contract.instrument.clone(),
            observation_id: String::new(),
            value: 1.0,
            source_window_start: end - Duration::days(7),
            source_window_end: end,
            complete: true,
        };
        let id = observation.expected_observation_id().unwrap();
        let MetricObservation::Observed { observation_id, .. } = &mut observation else {
            unreachable!()
        };
        *observation_id = id;
        observation
    }

    #[test]
    fn persists_identity_bound_observations_idempotently() {
        let directory = tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("registry.db")).unwrap();
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        let identity = MetricIdentity {
            wave_id: wave.id().as_str().to_string(),
            metric_id: "task-loop-trust".to_string(),
        };
        let contract = contract(identity.clone());
        let received_at = OffsetDateTime::from_unix_timestamp(2_000_000).unwrap();
        store
            .register_metric_instrument(&identity, &contract.instrument, received_at)
            .unwrap();
        let observation = observed(&contract, received_at);

        assert_eq!(
            store
                .accept_metric_observation(&contract, &observation, received_at)
                .unwrap(),
            ObservationAcceptance::Accepted
        );
        assert_eq!(
            store
                .accept_metric_observation(&contract, &observation, received_at)
                .unwrap(),
            ObservationAcceptance::Duplicate
        );
        assert_eq!(store.metric_observation_count(wave.id()).unwrap(), 1);
        let evidence = store
            .metric_observation_evidence(std::slice::from_ref(&contract))
            .unwrap();
        let persisted = evidence.get(&identity).unwrap();
        assert_eq!(persisted.current.as_ref(), Some(&observation));
        assert!(persisted.instrumented);
        assert!(persisted.graduation_qualified);
        assert_eq!(
            store
                .metric_instruments(std::slice::from_ref(&identity))
                .unwrap()
                .get(&identity),
            Some(&contract.instrument)
        );
    }

    #[test]
    fn concurrent_idempotent_acceptance_has_one_durable_winner() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("registry.db");
        let setup = SqliteStore::new(&path).unwrap();
        setup
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        setup.create_wave(&wave).unwrap();
        let identity = MetricIdentity {
            wave_id: wave.id().as_str().to_string(),
            metric_id: "task-loop-trust".to_string(),
        };
        let contract = contract(identity.clone());
        let received_at = OffsetDateTime::from_unix_timestamp(2_000_000).unwrap();
        setup
            .register_metric_instrument(&identity, &contract.instrument, received_at)
            .unwrap();
        let observation = observed(&contract, received_at);
        drop(setup);

        let barrier = Arc::new(Barrier::new(2));
        let mut workers = Vec::new();
        for store in [
            SqliteStore::new(&path).unwrap(),
            SqliteStore::new(&path).unwrap(),
        ] {
            let barrier = barrier.clone();
            let contract = contract.clone();
            let observation = observation.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                store.accept_metric_observation(&contract, &observation, received_at)
            }));
        }

        let results = workers
            .into_iter()
            .map(|worker| worker.join().unwrap().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, ObservationAcceptance::Accepted))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, ObservationAcceptance::Duplicate))
                .count(),
            1
        );
        let store = SqliteStore::new(&path).unwrap();
        assert_eq!(store.metric_observation_count(wave.id()).unwrap(), 1);
    }

    #[test]
    fn portfolio_evidence_is_bounded_by_current_contracts_not_history() {
        let directory = tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("registry.db")).unwrap();
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        let identity = MetricIdentity {
            wave_id: wave.id().as_str().to_string(),
            metric_id: "task-loop-trust".to_string(),
        };
        let contract = contract(identity.clone());
        let received_at = OffsetDateTime::from_unix_timestamp(3_000_000).unwrap();
        store
            .register_metric_instrument(&identity, &contract.instrument, received_at)
            .unwrap();
        let mut newest = None;
        for offset in 0..128 {
            let observation = observed(&contract, received_at - Duration::hours(127 - offset));
            store
                .accept_metric_observation(&contract, &observation, received_at)
                .unwrap();
            newest = Some(observation);
        }

        let evidence = store
            .metric_observation_evidence(std::slice::from_ref(&contract))
            .unwrap();

        assert_eq!(store.metric_observation_count(wave.id()).unwrap(), 128);
        assert_eq!(evidence.len(), 1);
        let persisted = evidence.get(&identity).unwrap();
        assert_eq!(persisted.current.as_ref(), newest.as_ref());
        assert!(persisted.instrumented);
        assert!(persisted.graduation_qualified);
    }

    #[test]
    fn concurrent_writes_cannot_split_current_from_revision_evidence() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("registry.db");
        let setup = SqliteStore::new(&path).unwrap();
        setup
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        setup.create_wave(&wave).unwrap();
        let received_at = OffsetDateTime::from_unix_timestamp(3_000_000).unwrap();
        let contracts = (0..64)
            .map(|index| {
                contract(MetricIdentity {
                    wave_id: wave.id().as_str().to_string(),
                    metric_id: format!("metric-{index}"),
                })
            })
            .collect::<Vec<_>>();
        for contract in &contracts {
            setup
                .register_metric_instrument(&contract.identity, &contract.instrument, received_at)
                .unwrap();
        }
        drop(setup);

        let barrier = Arc::new(Barrier::new(2));
        let writer = {
            let barrier = barrier.clone();
            let contracts = contracts.clone();
            let store = SqliteStore::new(&path).unwrap();
            std::thread::spawn(move || {
                barrier.wait();
                for contract in contracts {
                    let observation = observed(&contract, received_at);
                    store
                        .accept_metric_observation(&contract, &observation, received_at)
                        .unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            })
        };
        let reader = SqliteStore::new(&path).unwrap();
        barrier.wait();

        loop {
            let evidence = reader.metric_observation_evidence(&contracts).unwrap();
            for record in evidence.values() {
                let current = record.current.is_some();
                assert_eq!(record.instrumented, current);
                assert_eq!(record.graduation_qualified, current);
            }
            if writer.is_finished() {
                break;
            }
        }
        writer.join().unwrap();
    }

    #[test]
    fn current_evidence_orders_normalized_source_time_not_rfc3339_offset_text() {
        let directory = tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("registry.db")).unwrap();
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        let identity = MetricIdentity {
            wave_id: wave.id().as_str().to_string(),
            metric_id: "task-loop-trust".to_string(),
        };
        let contract = contract(identity.clone());
        let format = &time::format_description::well_known::Rfc3339;
        let earlier = observed(
            &contract,
            OffsetDateTime::parse("2026-08-21T12:00:00+14:00", format).unwrap(),
        );
        let later = observed(
            &contract,
            OffsetDateTime::parse("2026-08-21T01:00:00-10:00", format).unwrap(),
        );
        let received_at = OffsetDateTime::parse("2026-08-21T12:00:00Z", format).unwrap();
        store
            .register_metric_instrument(&identity, &contract.instrument, received_at)
            .unwrap();
        store
            .accept_metric_observation(&contract, &earlier, received_at)
            .unwrap();
        store
            .accept_metric_observation(&contract, &later, received_at)
            .unwrap();

        let evidence = store
            .metric_observation_evidence(std::slice::from_ref(&contract))
            .unwrap();

        assert!(later.source_time() > earlier.source_time());
        assert_eq!(evidence[&identity].current.as_ref(), Some(&later));
    }

    #[test]
    fn current_evidence_orders_fractional_source_times_numerically() {
        let directory = tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("registry.db")).unwrap();
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        let identity = MetricIdentity {
            wave_id: wave.id().as_str().to_string(),
            metric_id: "task-loop-trust".to_string(),
        };
        let contract = contract(identity.clone());
        let format = &time::format_description::well_known::Rfc3339;
        let earlier = observed(
            &contract,
            OffsetDateTime::parse("2026-08-21T12:00:00Z", format).unwrap(),
        );
        let later = observed(
            &contract,
            OffsetDateTime::parse("2026-08-21T12:00:00.5Z", format).unwrap(),
        );
        let received_at = OffsetDateTime::parse("2026-08-21T12:01:00Z", format).unwrap();
        store
            .register_metric_instrument(&identity, &contract.instrument, received_at)
            .unwrap();
        store
            .accept_metric_observation(&contract, &earlier, received_at)
            .unwrap();
        store
            .accept_metric_observation(&contract, &later, received_at)
            .unwrap();

        let evidence = store
            .metric_observation_evidence(std::slice::from_ref(&contract))
            .unwrap();

        assert!(later.source_time() > earlier.source_time());
        assert_eq!(evidence[&identity].current.as_ref(), Some(&later));
    }

    #[test]
    fn refuses_observation_when_registered_producer_disagrees() {
        let directory = tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("registry.db")).unwrap();
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        let identity = MetricIdentity {
            wave_id: wave.id().as_str().to_string(),
            metric_id: "task-loop-trust".to_string(),
        };
        let contract = contract(identity.clone());
        let received_at = OffsetDateTime::from_unix_timestamp(2_000_000).unwrap();
        store
            .register_metric_instrument(&identity, "retired-scorecard", received_at)
            .unwrap();

        let error = store
            .accept_metric_observation(&contract, &observed(&contract, received_at), received_at)
            .unwrap_err();
        assert!(error.to_string().contains("registered instrument"));
        assert_eq!(store.metric_observation_count(wave.id()).unwrap(), 0);
    }

    #[test]
    fn instrument_registration_is_idempotent_but_cannot_rebind() {
        let directory = tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("registry.db")).unwrap();
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        let identity = MetricIdentity {
            wave_id: wave.id().as_str().to_string(),
            metric_id: "task-loop-trust".to_string(),
        };
        let registered_at = OffsetDateTime::from_unix_timestamp(2_000_000).unwrap();

        store
            .register_metric_instrument(&identity, "lifecycle-scorecard", registered_at)
            .unwrap();
        store
            .register_metric_instrument(&identity, "lifecycle-scorecard", registered_at)
            .unwrap();
        let error = store
            .register_metric_instrument(&identity, "replacement-scorecard", registered_at)
            .unwrap_err();

        assert!(error.to_string().contains("already bound"));
        assert_eq!(
            store
                .metric_instruments(std::slice::from_ref(&identity))
                .unwrap()
                .get(&identity)
                .map(String::as_str),
            Some("lifecycle-scorecard")
        );
    }
}
