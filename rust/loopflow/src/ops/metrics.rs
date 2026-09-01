use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::controller::wave::metrics::{
    compose_metric_portfolio, discover_metric_contracts, load_metric_contract,
    MetricContractDiscovery, MetricObservation, MetricPortfolioDto, ObservationAcceptance,
};
use crate::pm::PmProject;
use crate::store::Store;
use crate::work::wave::Wave;

pub(crate) async fn wave_metric_portfolio(
    store: &Store,
    wave: &Wave,
    projects: &[PmProject],
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    let discovery = discover_wave_contracts(wave, projects)?;
    compose_persisted_portfolio(store, discovery, evaluation_time).await
}

pub(crate) async fn project_metric_portfolio(
    store: &Store,
    wave: &Wave,
    projects: &[PmProject],
    project_id: &str,
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    let mut discovery = discover_wave_contracts(wave, projects)?;
    discovery
        .contracts
        .retain(|source| source.contract.project_id == project_id);
    discovery.contract_issues.clear();
    compose_persisted_portfolio(store, discovery, evaluation_time).await
}

pub(crate) async fn stored_wave_metric_portfolio(
    store: &Store,
    wave: &Wave,
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    let projects = stored_projects(store, wave).await?;
    wave_metric_portfolio(store, wave, &projects, evaluation_time).await
}

pub(crate) async fn stored_project_metric_portfolio(
    store: &Store,
    wave: &Wave,
    project_id: &str,
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    let projects = stored_projects(store, wave).await?;
    project_metric_portfolio(store, wave, &projects, project_id, evaluation_time).await
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

async fn stored_projects(store: &Store, wave: &Wave) -> Result<Vec<PmProject>> {
    let row = store
        .pm_snapshot(wave.id())
        .await
        .map_err(|error| anyhow!("failed to read PM snapshot: {error}"))?
        .ok_or_else(|| anyhow!("wave/{} has no local PM snapshot", wave.name()))?;
    let planning: crate::pm::PmSnapshot = serde_json::from_str(&row.payload)
        .map_err(|error| anyhow!("wave/{} PM snapshot is invalid: {error}", wave.name()))?;
    Ok(planning.projects)
}

fn discover_wave_contracts(wave: &Wave, projects: &[PmProject]) -> Result<MetricContractDiscovery> {
    let project_ids = projects
        .iter()
        .map(|project| project.id.clone())
        .collect::<BTreeSet<_>>();
    let metrics_dir = metric_contract_repo(wave, std::env::var_os("LOOPFLOW_DEV_WAVE_REPO"))
        .join("wave")
        .join(wave.name())
        .join("metrics");
    discover_metric_contracts(&metrics_dir, wave.id().as_str(), &project_ids)
        .map_err(|error| anyhow!(error))
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
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    if discovery.contracts.is_empty() {
        return compose_metric_portfolio(
            discovery,
            &Default::default(),
            &Default::default(),
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
    compose_metric_portfolio(discovery, &instruments, &observations, evaluation_time)
        .map_err(|error| anyhow!(error))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum MetricProducerObservation {
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
        contract: &crate::controller::wave::metrics::MetricContract,
    ) -> Result<MetricObservation> {
        let mut observation = match self {
            Self::Observed {
                value,
                source_window_start,
                source_window_end,
                complete,
                ..
            } => MetricObservation::Observed {
                identity: contract.identity.clone(),
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
                identity: contract.identity.clone(),
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

pub(crate) async fn publish_metric_observations(
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
            .find(|wave| wave.name() == wave_name)
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
            .join("wave")
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

    use crate::controller::wave::metrics::{
        load_metric_contract, MetricEvidenceDto, MetricObservation, MetricStage,
    };
    use crate::id::WaveId;
    use crate::pm::{PmKr, ProjectFlowPlan};
    use crate::store::StorageConfig;

    use super::*;

    fn project(id: &str, slug: &str) -> PmProject {
        PmProject {
            id: id.to_string(),
            slug: slug.to_string(),
            name: slug.replace('-', " "),
            summary: String::new(),
            definition: format!("Advance {slug}."),
            flows: Some(ProjectFlowPlan::empty()),
            krs: vec![PmKr {
                text: "Proof holds for one week".to_string(),
                holds: false,
            }],
            initiative_ids: vec!["initiative-1".to_string()],
            team_ids: vec!["team-1".to_string()],
        }
    }

    fn contract_markdown(
        id: &str,
        project_id: &str,
        stage: MetricStage,
        instrument: &str,
    ) -> String {
        let stage = match stage {
            MetricStage::Installed => "installed",
            MetricStage::Graduated => "graduated",
        };
        format!(
            "---\nschema: 1\nid: {id}\nproject_id: {project_id}\nstage: {stage}\ninstrument: {instrument}\nunit: ratio\ntarget:\n  at_least: 1\nwindow: 7d\nfreshness: 6h\n---\n\n# {id}\n\nCount complete Task loops.\n"
        )
    }

    fn observed(
        contract: &crate::controller::wave::metrics::MetricContract,
        end: OffsetDateTime,
    ) -> MetricObservation {
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
    async fn shared_portfolio_joins_persisted_evidence_without_changing_krs() {
        let directory = tempdir().unwrap();
        let repo = directory.path().join("repo");
        let metrics_dir = repo.join("wave/product/metrics");
        fs::create_dir_all(&metrics_dir).unwrap();
        let trust_path = metrics_dir.join("task-loop-trust.md");
        fs::write(
            &trust_path,
            contract_markdown(
                "task-loop-trust",
                "project-api",
                MetricStage::Graduated,
                "lifecycle-scorecard",
            ),
        )
        .unwrap();
        fs::write(
            metrics_dir.join("surface-parity.md"),
            contract_markdown(
                "surface-parity",
                "project-surface",
                MetricStage::Installed,
                "surface-scorecard",
            ),
        )
        .unwrap();

        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let projects = vec![
            project("project-api", "loopflow-api"),
            project("project-surface", "mac-surface"),
        ];
        let unchanged_krs = projects
            .iter()
            .map(|project| project.krs.clone())
            .collect::<Vec<_>>();
        let contract = load_metric_contract(&trust_path, wave.id().as_str()).unwrap();
        let source_time = OffsetDateTime::UNIX_EPOCH + Duration::days(30);
        store
            .register_metric_instrument(&contract.identity, &contract.instrument, source_time)
            .await
            .unwrap();
        store
            .accept_metric_observation(&contract, observed(&contract, source_time), source_time)
            .await
            .unwrap();

        let portfolio =
            wave_metric_portfolio(&store, &wave, &projects, source_time + Duration::hours(1))
                .await
                .unwrap();
        assert_eq!(portfolio.metrics.len(), 2);
        assert!(portfolio.contract_issues.is_empty());
        assert!(portfolio.metrics.iter().any(|metric| {
            metric.project_id == "project-api"
                && matches!(metric.evidence, MetricEvidenceDto::Met { .. })
        }));
        assert!(portfolio.metrics.iter().any(|metric| {
            metric.project_id == "project-surface"
                && matches!(metric.evidence, MetricEvidenceDto::Unknown { .. })
        }));

        let owned = project_metric_portfolio(
            &store,
            &wave,
            &projects,
            "project-api",
            source_time + Duration::hours(1),
        )
        .await
        .unwrap();
        assert_eq!(owned.metrics.len(), 1);
        assert_eq!(owned.metrics[0].project_id, "project-api");
        assert_eq!(owned.metrics[0].description, "Count complete Task loops.");
        let prompt = metric_prompt_section("project-owned-metrics", Ok(owned));
        assert!(prompt.contains("\"description\":\"Count complete Task loops.\""));
        assert_eq!(
            projects
                .iter()
                .map(|project| project.krs.clone())
                .collect::<Vec<_>>(),
            unchanged_krs
        );
    }

    #[tokio::test]
    async fn source_build_without_draft_storage_keeps_installed_metric_visible() {
        let directory = tempdir().unwrap();
        let repo = directory.path().join("repo");
        let metrics_dir = repo.join("wave/product/metrics");
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
            &[project("project-api", "loopflow-api")],
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

    #[tokio::test]
    async fn lifecycle_producer_persists_the_reading_consumers_project() {
        let directory = tempdir().unwrap();
        let repo = directory.path().join("repo");
        let metrics_dir = repo.join("wave/product/metrics");
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
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let source_time = OffsetDateTime::UNIX_EPOCH + Duration::days(30);
        let result = publish_metric_observations(
            &store,
            &repo,
            vec![MetricProducerObservation::Observed {
                wave: "product".to_string(),
                metric_id: "task-loop-trust".to_string(),
                instrument: "lifecycle-scorecard".to_string(),
                value: 1.0,
                source_window_start: source_time - Duration::days(7),
                source_window_end: source_time,
                complete: true,
            }],
            source_time,
        )
        .await
        .unwrap();

        let portfolio = wave_metric_portfolio(
            &store,
            &wave,
            &[project("project-api", "loopflow-api")],
            source_time + Duration::hours(1),
        )
        .await
        .unwrap();

        assert_eq!(result, ["product/task-loop-trust: accepted"]);
        assert!(portfolio.metrics[0].instrumented);
        assert!(matches!(
            portfolio.metrics[0].evidence,
            MetricEvidenceDto::Met { value: 1.0, .. }
        ));
    }
}
