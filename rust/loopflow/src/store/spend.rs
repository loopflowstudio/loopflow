use super::{run_sqlite, Store, StoreResult};
use crate::spend::{
    DependencyId, DependencyInspection, Inventory, Invoice, InvoiceRevision, Report,
};

impl Store {
    pub async fn import_spend_inventory(&self, mut inventory: Inventory) -> StoreResult<()> {
        for dependency in &mut inventory.dependencies {
            for consumer in &mut dependency.consumers {
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
}

impl Store {
    pub async fn spend_access(
        &self,
        environment: crate::spend::EnvironmentId,
    ) -> StoreResult<Option<crate::spend::AccessInspection>> {
        run_sqlite(&self.sqlite, move |store| store.spend_access(&environment)).await
    }
    pub async fn verify_spend_access(
        &self,
        environment: crate::spend::EnvironmentId,
        period: String,
    ) -> StoreResult<crate::spend::AccessInspection> {
        use crate::spend::{AccessObservation, AccessOutcome};
        let inspection = self
            .spend_access(environment.clone())
            .await?
            .ok_or(super::StoreError::NotFound)?;
        let local = self.local_home().await?;
        let is_local = inspection.environment.home_id.as_ref() == Some(&local.id);
        let mut observations = Vec::new();
        for requirement in &inspection.requirements {
            let credential = inspection
                .credentials
                .iter()
                .find(|c| requirement.credential.as_ref() == Some(&c.id));
            let (outcome, gap) = if !is_local {
                (
                    AccessOutcome::Unavailable,
                    Some("environment is not bound to this Home; no remote probe performed".into()),
                )
            } else if requirement.tool.as_deref() == Some("auth.report") {
                match self.spend_report(period.clone(), None, None).await {
                    Ok(_) => (AccessOutcome::Success, Some("local report read succeeded; isolated provisioning is separate evidence".into())),
                    Err(_) => (AccessOutcome::Unavailable, Some("local report unavailable".into())),
                }
            } else {
                (
                    AccessOutcome::Unavailable,
                    Some("provider billing probe integration missing; no secret fetched".into()),
                )
            };
            observations.push(AccessObservation {
                environment: environment.clone(),
                executed_home: local.id.clone(),
                requirement: requirement.id.clone(),
                requirement_revision: requirement.revision.clone(),
                credential: requirement.credential.clone(),
                credential_version: credential.and_then(|c| c.version.clone()),
                operation: "billing_read".into(),
                observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                outcome,
                scope_evidence: None,
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
}

fn normalize_consumer(consumer: &mut crate::spend::Consumer) -> StoreResult<()> {
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
