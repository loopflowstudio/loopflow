use std::collections::BTreeMap;

use time::OffsetDateTime;

use crate::controller::wave::metrics::{
    MetricContract, MetricIdentity, MetricObservation, MetricObservationEvidence,
    ObservationAcceptance,
};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn metric_storage_available(&self) -> StoreResult<bool> {
        run_sqlite(&self.sqlite, move |store| store.metric_storage_available()).await
    }

    pub async fn register_metric_instrument(
        &self,
        identity: &MetricIdentity,
        instrument: &str,
        registered_at: OffsetDateTime,
    ) -> StoreResult<()> {
        let identity = identity.clone();
        let instrument = instrument.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.register_metric_instrument(&identity, &instrument, registered_at)
        })
        .await
    }

    pub async fn accept_metric_observation(
        &self,
        contract: &MetricContract,
        observation: MetricObservation,
        received_at: OffsetDateTime,
    ) -> StoreResult<ObservationAcceptance> {
        let contract = contract.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.accept_metric_observation(&contract, &observation, received_at)
        })
        .await
    }

    pub async fn metric_instruments(
        &self,
        identities: &[MetricIdentity],
    ) -> StoreResult<BTreeMap<MetricIdentity, String>> {
        let identities = identities.to_vec();
        run_sqlite(&self.sqlite, move |store| {
            store.metric_instruments(&identities)
        })
        .await
    }

    pub async fn metric_observation_evidence(
        &self,
        contracts: &[MetricContract],
    ) -> StoreResult<BTreeMap<MetricIdentity, MetricObservationEvidence>> {
        let contracts = contracts.to_vec();
        run_sqlite(&self.sqlite, move |store| {
            store.metric_observation_evidence(&contracts)
        })
        .await
    }
}
