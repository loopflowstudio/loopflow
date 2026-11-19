use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::store::Store;
use crate::work::wave::metrics::{
    compose_metric_portfolio, discover_metric_contracts, load_metric_contract,
    MetricContractDiscovery, MetricContractIssueDto, MetricObservation, MetricPortfolioDto,
    ObservationAcceptance,
};
use crate::work::wave::Wave;

pub async fn wave_metric_portfolio(
    store: &Store,
    wave: &Wave,
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    let targets = store
        .sqlite
        .new_task_project(wave.id())?
        .map(|joined| joined.project.metric_targets)
        .unwrap_or_default();
    project_metric_portfolio(store, wave, &targets, evaluation_time).await
}

pub(crate) async fn project_metric_portfolio(
    store: &Store,
    wave: &Wave,
    targets: &[crate::work::wave::metrics::MetricTarget],
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    let mut discovery = discover_wave_contracts(wave)?;
    for target in targets {
        if !discovery
            .contracts
            .iter()
            .any(|source| source.contract.identity.metric_id == target.metric_id)
        {
            discovery
                .contract_issues
                .push(MetricContractIssueDto::UnresolvedTarget {
                    wave_id: wave.id().to_string(),
                    metric_id: target.metric_id.clone(),
                });
        }
    }
    compose_persisted_portfolio(store, discovery, Some(targets), evaluation_time).await
}

pub(crate) async fn stored_wave_metric_portfolio(
    store: &Store,
    wave: &Wave,
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    wave_metric_portfolio(store, wave, evaluation_time).await
}

pub(crate) fn metric_prompt_section(tag: &str, portfolio: Result<MetricPortfolioDto>) -> String {
    match portfolio {
        Ok(portfolio) => format!(
            "<lf:{tag}>\n{}\n</lf:{tag}>",
            serde_json::to_string(&portfolio).expect("metric portfolio DTO always serializes")
        ),
        Err(error) => format!("<lf:{tag}>\nUnavailable: {error}\n</lf:{tag}>"),
    }
}

fn discover_wave_contracts(wave: &Wave) -> Result<MetricContractDiscovery> {
    let metrics_dir = metric_contract_repo(wave, std::env::var_os("LOOPFLOW_DEV_WAVE_REPO"))
        .join(".lf/wave")
        .join(wave.slug())
        .join("metrics");
    discover_metric_contracts(&metrics_dir, wave.id().as_str()).map_err(|error| anyhow!(error))
}

fn metric_contract_repo(wave: &Wave, dev_repo: Option<OsString>) -> PathBuf {
    dev_repo
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(wave.repo()))
}

async fn compose_persisted_portfolio(
    store: &Store,
    discovery: MetricContractDiscovery,
    targets: Option<&[crate::work::wave::metrics::MetricTarget]>,
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    if discovery.contracts.is_empty() {
        return compose_metric_portfolio(
            discovery,
            &Default::default(),
            &Default::default(),
            targets,
            evaluation_time,
        )
        .map_err(|error| anyhow!(error));
    }
    if !store
        .metric_storage_available()
        .await
        .map_err(|error| anyhow!("failed to inspect metric storage: {error}"))?
    {
        return compose_metric_portfolio(
            discovery,
            &Default::default(),
            &Default::default(),
            targets,
            evaluation_time,
        )
        .map_err(|error| anyhow!(error));
    }
    let contracts = discovery
        .contracts
        .iter()
        .map(|source| source.contract.clone())
        .collect::<Vec<_>>();
    let identities = contracts
        .iter()
        .map(|contract| contract.identity.clone())
        .collect::<Vec<_>>();
    let instruments = store
        .metric_instruments(&identities)
        .await
        .map_err(|error| anyhow!("failed to read metric instruments: {error}"))?;
    let observations = store
        .metric_observation_evidence(&contracts)
        .await
        .map_err(|error| anyhow!("failed to read metric observations: {error}"))?;
    compose_metric_portfolio(
        discovery,
        &instruments,
        &observations,
        targets,
        evaluation_time,
    )
    .map_err(|error| anyhow!(error))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricProducerObservation {
    Observed {
        wave: String,
        metric_id: String,
        instrument: String,
        value: f64,
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
        complete: bool,
    },
    Unavailable {
        wave: String,
        metric_id: String,
        instrument: String,
        #[serde(with = "time::serde::rfc3339")]
        source_as_of: OffsetDateTime,
        reason: String,
    },
}

impl MetricProducerObservation {
    fn address(&self) -> (&str, &str, &str) {
        match self {
            Self::Observed {
                wave,
                metric_id,
                instrument,
                ..
            }
            | Self::Unavailable {
                wave,
                metric_id,
                instrument,
                ..
            } => (wave, metric_id, instrument),
        }
    }

    fn bind(
        self,
        contract: &crate::work::wave::metrics::MetricContract,
    ) -> Result<MetricObservation> {
        let mut observation = match self {
            Self::Observed {
                value,
                source_window_start,
                source_window_end,
                complete,
                ..
            } => MetricObservation::Observed {
                identity: contract.identity.clone().into(),
                contract_revision: contract.contract_revision.clone(),
                instrument: contract.instrument.clone(),
                observation_id: String::new(),
                value,
                source_window_start,
                source_window_end,
                complete,
            },
            Self::Unavailable {
                source_as_of,
                reason,
                ..
            } => MetricObservation::Unavailable {
                identity: contract.identity.clone().into(),
                contract_revision: contract.contract_revision.clone(),
                instrument: contract.instrument.clone(),
                observation_id: String::new(),
                source_as_of,
                reason,
            },
        };
        let observation_id = observation
            .expected_observation_id()
            .map_err(|error| anyhow!(error))?;
        match &mut observation {
            MetricObservation::Observed {
                observation_id: target,
                ..
            }
            | MetricObservation::Unavailable {
                observation_id: target,
                ..
            } => *target = observation_id,
        }
        Ok(observation)
    }
}

pub async fn publish_metric_observations(
    store: &Store,
    repo: &Path,
    observations: Vec<MetricProducerObservation>,
    received_at: OffsetDateTime,
) -> Result<Vec<String>> {
    if observations.is_empty() {
        return Ok(Vec::new());
    }
    if !store
        .metric_storage_available()
        .await
        .map_err(|error| anyhow!("failed to inspect metric storage: {error}"))?
    {
        return Ok(vec![
            "metric storage awaits the next released migration; observations were not persisted"
                .to_string(),
        ]);
    }

    let waves = store
        .list_waves(Some(&repo.display().to_string()))
        .await
        .map_err(|error| anyhow!("failed to list repository Waves: {error}"))?;
    let mut results = Vec::new();
    for input in observations {
        let (wave_name, metric_id, instrument) = {
            let (wave_name, metric_id, instrument) = input.address();
            (
                wave_name.to_string(),
                metric_id.to_string(),
                instrument.to_string(),
            )
        };
        let wave = waves
            .iter()
            .find(|wave| wave.slug() == wave_name)
            .with_context(|| {
                format!("producer metric {wave_name}/{metric_id} has no registered Wave")
            })?;
        if metric_id.contains('/')
            || metric_id.contains('\\')
            || metric_id == "."
            || metric_id == ".."
        {
            bail!("producer metric id {metric_id:?} is not a direct contract filename");
        }
        let contract_path = repo
            .join(".lf/wave")
            .join(&wave_name)
            .join("metrics")
            .join(format!("{metric_id}.md"));
        let contract =
            load_metric_contract(&contract_path, wave.id().as_str()).map_err(|error| {
                anyhow!(
                    "load producer contract {}: {error}",
                    contract_path.display()
                )
            })?;
        if contract.instrument != instrument {
            bail!(
                "producer metric {wave_name}/{metric_id} emitted instrument {instrument:?}, contract declares {:?}",
                contract.instrument
            );
        }
        let observation = input.bind(&contract)?;
        observation
            .validate(&contract, received_at)
            .map_err(|error| anyhow!("validate {wave_name}/{metric_id} observation: {error}"))?;

        let registered = store
            .metric_instruments(std::slice::from_ref(&contract.identity))
            .await
            .map_err(|error| anyhow!("read producer binding: {error}"))?;
        match registered.get(&contract.identity) {
            Some(existing) if existing != &instrument => bail!(
                "producer metric {wave_name}/{metric_id} is already bound to {existing:?}, not {instrument:?}"
            ),
            Some(_) => {}
            None => store
                .register_metric_instrument(&contract.identity, &instrument, received_at)
                .await
                .map_err(|error| anyhow!("register producer binding: {error}"))?,
        }

        let acceptance = store
            .accept_metric_observation(&contract, observation, received_at)
            .await
            .map_err(|error| anyhow!("accept {wave_name}/{metric_id} observation: {error}"))?;
        let result = match acceptance {
            ObservationAcceptance::Accepted => "accepted",
            ObservationAcceptance::Duplicate => "duplicate",
        };
        results.push(format!("{wave_name}/{metric_id}: {result}"));
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use tempfile::tempdir;
    use time::Duration;

    use crate::store::StorageConfig;
    use crate::work::wave::metrics::{MetricEvidenceDto, MetricStage};
    use loopflow_model::id::WaveId;

    use super::*;

    fn contract_markdown(
        id: &str,
        _project_id: &str,
        stage: MetricStage,
        instrument: &str,
    ) -> String {
        let stage = match stage {
            MetricStage::Installed => "installed",
            MetricStage::Graduated => "graduated",
        };
        format!(
            "---\nschema: 1\nid: {id}\nstage: {stage}\ninstrument: {instrument}\nunit: ratio\nwindow: 7d\nfreshness: 6h\n---\n\n# {id}\n\nCount complete Task loops.\n"
        )
    }

    #[test]
    fn desktop_dev_build_reads_contracts_from_the_launched_checkout() {
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            "/src/loopflow".to_string(),
        );

        assert_eq!(
            metric_contract_repo(
                &wave,
                Some(OsString::from("/src/loopflow.define-live-metrics"))
            ),
            PathBuf::from("/src/loopflow.define-live-metrics")
        );
        assert_eq!(
            metric_contract_repo(&wave, None),
            PathBuf::from(wave.repo())
        );
    }

    #[tokio::test]
    async fn source_build_keeps_installed_metric_visible() {
        let directory = tempdir().unwrap();
        let repo = directory.path().join("repo");
        let metrics_dir = repo.join(".lf/wave/product/metrics");
        fs::create_dir_all(&metrics_dir).unwrap();
        fs::write(
            metrics_dir.join("task-loop-trust.md"),
            contract_markdown(
                "task-loop-trust",
                "project-api",
                MetricStage::Installed,
                "lifecycle-scorecard",
            ),
        )
        .unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();

        let portfolio = wave_metric_portfolio(
            &store,
            &wave,
            OffsetDateTime::UNIX_EPOCH + Duration::days(30),
        )
        .await
        .unwrap();

        assert_eq!(portfolio.metrics.len(), 1);
        assert!(!portfolio.metrics[0].instrumented);
        assert!(matches!(
            portfolio.metrics[0].evidence,
            MetricEvidenceDto::Unknown { .. }
        ));
    }
}
