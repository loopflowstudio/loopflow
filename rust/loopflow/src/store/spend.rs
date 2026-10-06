use super::{run_sqlite, Store, StoreResult};
use crate::spend::aws_cur::AwsCurExport;
use crate::spend::rotation::{Rotation, RotationId, RotationReceipt};
use crate::spend::{
    AccessInspection, AccessObservation, AccessOutcome, Consumer, CredentialId, DependencyHistory,
    DependencyId, DependencyInspection, EnvironmentId, Inventory, Invoice, InvoiceRevision, Report,
    SourceId,
};

impl Store {
    pub async fn record_spend_import(
        &self,
        observation: crate::spend::ImportObservation,
    ) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| {
            store.record_spend_import(&observation)
        })
        .await
    }

    pub async fn import_spend_inventory(&self, mut inventory: Inventory) -> StoreResult<()> {
        for dependency in &mut inventory.dependencies {
            for consumer in &mut dependency.consumers {
                normalize_consumer(consumer)?;
            }
        }
        for requirement in &mut inventory.requirements {
            if let Some(consumer) = &mut requirement.report_consumer {
                normalize_consumer(consumer)?;
            }
        }
        for rule in &mut inventory.rules {
            for assignment in &mut rule.assignments {
                if let Some(consumer) = &mut assignment.consumer {
                    normalize_consumer(consumer)?;
                }
            }
        }
        run_sqlite(&self.sqlite, move |store| {
            store.import_spend_inventory(&inventory)
        })
        .await
    }
    pub async fn import_spend_invoice(&self, invoice: Invoice) -> StoreResult<InvoiceRevision> {
        run_sqlite(&self.sqlite, move |store| {
            store.import_spend_invoice(&invoice)
        })
        .await
    }
    pub async fn spend_report(
        &self,
        period: String,
        repo: Option<String>,
        wave: Option<crate::id::WaveId>,
    ) -> StoreResult<Report> {
        run_sqlite(&self.sqlite, move |store| {
            store.spend_report(&period, repo.as_deref(), wave.as_ref())
        })
        .await
    }
    pub async fn spend_dependency(
        &self,
        id: DependencyId,
        period: String,
    ) -> StoreResult<Option<DependencyInspection>> {
        run_sqlite(&self.sqlite, move |store| {
            store.spend_dependency(&id, &period)
        })
        .await
    }

    pub async fn spend_access(
        &self,
        environment: EnvironmentId,
    ) -> StoreResult<Option<AccessInspection>> {
        run_sqlite(&self.sqlite, move |store| store.spend_access(&environment)).await
    }
    pub async fn verify_spend_access(
        &self,
        environment: EnvironmentId,
        period: String,
        export: Option<std::path::PathBuf>,
    ) -> StoreResult<AccessInspection> {
        let inspection = self
            .spend_access(environment.clone())
            .await?
            .ok_or(super::StoreError::NotFound)?;
        let local = self.local_home().await?;
        let is_local = inspection.environment.home_id.as_ref() == Some(&local.id);
        let remote = match inspection.environment.home_id.as_ref() {
            Some(id) if !is_local => self.home_by_id(id).await?,
            _ => None,
        };
        let mut observations = Vec::new();
        for requirement in &inspection.requirements {
            let credential = inspection
                .credentials
                .iter()
                .find(|c| requirement.credential.as_ref() == Some(&c.id));
            let mut report_receipt = None;
            let mut executed_home = local.id.clone();
            let (outcome, gap) = match (is_local, requirement.tool.as_deref()) {
                (_, Some("auth.export")) => match (export.as_deref(), requirement.report_consumer.as_ref()) {
                    (Some(path), Some(consumer)) => {
                        let result = match inspection.environment.home_id.as_ref() {
                            Some(home) if is_local || remote.is_some() => crate::spend::consumer::consume_bound(
                                path, consumer, &period,
                                Some(crate::spend::consumer::ConsumptionBinding {
                                    home: home.clone(), requirement: requirement.id.clone(), revision: requirement.revision.clone(),
                                }), remote.as_ref(),
                            ).await,
                            _ => Err(crate::spend::consumer::ConsumptionError::Unavailable),
                        };
                        match result {
                            Ok((_, receipt)) => {
                                executed_home = receipt.binding.as_ref().expect("verification supplies a Home binding").home.clone();
                                report_receipt = Some(receipt);
                                (AccessOutcome::Success, Some("isolated container read over a directly collected channel; provider permissions and export provenance unverified".into()))
                            }
                            Err(crate::spend::consumer::ConsumptionError::Denied) => (
                                AccessOutcome::Denied,
                                Some(
                                    "export recipient, period or shape does not match requirement"
                                        .into(),
                                ),
                            ),
                            Err(crate::spend::consumer::ConsumptionError::Unavailable) => (
                                AccessOutcome::Unavailable,
                                Some(
                                    "isolated export consumer unavailable; no receipt collected"
                                        .into(),
                                ),
                            ),
                        }
                    }
                    _ => (
                        AccessOutcome::Unavailable,
                        Some("designated export file required".into()),
                    ),
                },
                (false, _) => (
                    AccessOutcome::Unavailable,
                    Some("remote credential and administrative probes unavailable; no secret fetched".into()),
                ),
                (true, Some("auth.report")) => match self.spend_report(period.clone(), None, None).await {
                    Ok(_) => (AccessOutcome::Success, Some("local report read succeeded; isolated provisioning is separate evidence".into())),
                    Err(_) => (AccessOutcome::Unavailable, Some("local report unavailable".into())),
                },
                (true, None) if requirement.billing_probe == Some(crate::spend::BillingProbe::Runpod) => match credential {
                    Some(credential) => crate::spend::runpod::verify(&credential.reference, &period).await,
                    None => (AccessOutcome::Unavailable, Some("Runpod billing read requires a Doppler credential reference; no secret fetched".into())),
                },
                (true, _) => (
                    AccessOutcome::Unavailable,
                    Some("provider billing probe integration missing; no secret fetched".into()),
                ),
            };
            observations.push(AccessObservation {
                environment: environment.clone(),
                executed_home,
                requirement: requirement.id.clone(),
                requirement_revision: requirement.revision.clone(),
                credential: requirement.credential.clone(),
                credential_version: credential.and_then(|c| c.version.clone()),
                credential_reference: credential.map(|c| c.reference.clone()),
                operation: if requirement.tool.as_deref() == Some("auth.export") {
                    "designated_export_read"
                } else {
                    "billing_read"
                }
                .into(),
                observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                outcome,
                scope_evidence: None,
                report_receipt,
                gap,
            });
        }
        run_sqlite(&self.sqlite, move |store| {
            store.record_spend_access(&observations)
        })
        .await?;
        self.spend_access(environment)
            .await?
            .ok_or(super::StoreError::NotFound)
    }

    pub async fn spend_dependency_history(
        &self,
        id: DependencyId,
    ) -> StoreResult<Vec<DependencyHistory>> {
        run_sqlite(&self.sqlite, move |store| {
            store.spend_dependency_history(&id)
        })
        .await
    }

    pub async fn begin_spend_rotation(
        &self,
        old: CredentialId,
        candidate: CredentialId,
        consumer_inventory_evidence: Option<String>,
    ) -> StoreResult<Rotation> {
        run_sqlite(&self.sqlite, move |store| {
            store.begin_spend_rotation(&old, &candidate, consumer_inventory_evidence)
        })
        .await
    }
    pub async fn spend_rotation(&self, id: RotationId) -> StoreResult<Option<Rotation>> {
        run_sqlite(&self.sqlite, move |store| store.spend_rotation(&id)).await
    }
    pub async fn record_spend_rotation(
        &self,
        id: RotationId,
        receipt: RotationReceipt,
    ) -> StoreResult<Rotation> {
        run_sqlite(&self.sqlite, move |store| {
            store.record_spend_rotation(&id, receipt)
        })
        .await
    }
    pub async fn activate_spend_rotation(&self, id: RotationId) -> StoreResult<Rotation> {
        run_sqlite(&self.sqlite, move |store| {
            store.activate_spend_rotation(&id)
        })
        .await
    }

    pub async fn cancel_spend_rotation(&self, id: RotationId) -> StoreResult<Rotation> {
        run_sqlite(&self.sqlite, move |store| store.cancel_spend_rotation(&id)).await
    }

    pub async fn reconcile_spend_rotation(
        &self,
        id: RotationId,
        consumer_inventory_evidence: Option<String>,
    ) -> StoreResult<Rotation> {
        run_sqlite(&self.sqlite, move |store| {
            store.reconcile_spend_rotation(&id, consumer_inventory_evidence)
        })
        .await
    }

    pub async fn import_spend_aws_cur(
        &self,
        export: AwsCurExport,
        source: SourceId,
        period: String,
    ) -> StoreResult<InvoiceRevision> {
        run_sqlite(&self.sqlite, move |store| {
            store.import_spend_aws_cur(&export, source, &period)
        })
        .await
    }
}

fn normalize_consumer(consumer: &mut Consumer) -> StoreResult<()> {
    let path = std::path::Path::new(&consumer.repo);
    if path.is_dir() {
        consumer.repo = crate::repository::CanonicalRepo::discover(path)
            .map_err(|_| crate::spend::invalid("cannot resolve local repository identity"))?
            .to_string();
    } else if !path.is_absolute() {
        consumer.repo = crate::repository::RepoId::parse(&consumer.repo)
            .map_err(|_| crate::spend::invalid("external repository requires owner/repo identity"))?
            .to_string();
    }
    Ok(())
}
