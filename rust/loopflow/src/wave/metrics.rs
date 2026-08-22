//! Project-owned live metric contracts, observations, and derived readings.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};

const CONTRACT_SCHEMA: &str = "loopflow.metric/v1";
const FUTURE_SOURCE_ALLOWANCE: Duration = Duration::minutes(5);
const MAX_METRIC_DURATION_SECONDS: i64 = 10 * 365 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MetricIdentity {
    pub wave_id: String,
    pub metric_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricStage {
    Installed,
    Graduated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricTarget {
    AtLeast { value: f64 },
    AtMost { value: f64 },
}

impl MetricTarget {
    fn value(&self) -> f64 {
        match self {
            Self::AtLeast { value } | Self::AtMost { value } => *value,
        }
    }

    fn is_met(&self, observed: f64) -> bool {
        match self {
            Self::AtLeast { value } => observed >= *value,
            Self::AtMost { value } => observed <= *value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricDuration {
    source: String,
    elapsed: Duration,
}

impl MetricDuration {
    pub fn parse(value: &str) -> Result<Self, MetricError> {
        let (amount, seconds_per_unit) = if let Some(amount) = value.strip_suffix('m') {
            (amount, 60)
        } else if let Some(amount) = value.strip_suffix('h') {
            (amount, 60 * 60)
        } else if let Some(amount) = value.strip_suffix('d') {
            (amount, 24 * 60 * 60)
        } else {
            return Err(MetricError::InvalidDuration(value.to_string()));
        };
        let amount = amount
            .parse::<i64>()
            .map_err(|_| MetricError::InvalidDuration(value.to_string()))?;
        if amount <= 0 {
            return Err(MetricError::InvalidDuration(value.to_string()));
        }
        let seconds = amount
            .checked_mul(seconds_per_unit)
            .ok_or_else(|| MetricError::InvalidDuration(value.to_string()))?;
        if seconds > MAX_METRIC_DURATION_SECONDS {
            return Err(MetricError::InvalidDuration(value.to_string()));
        }
        Ok(Self {
            source: value.to_string(),
            elapsed: Duration::seconds(seconds),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

#[derive(Debug, Clone)]
pub struct MetricContractDefinition {
    pub identity: MetricIdentity,
    pub name: String,
    pub project_id: String,
    pub stage: MetricStage,
    pub instrument: String,
    pub unit: String,
    pub target: MetricTarget,
    pub window: MetricDuration,
    pub freshness_policy: MetricDuration,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct MetricContract {
    pub identity: MetricIdentity,
    pub contract_revision: String,
    pub name: String,
    pub description: String,
    pub project_id: String,
    pub stage: MetricStage,
    pub instrument: String,
    pub unit: String,
    pub target: MetricTarget,
    pub window: MetricDuration,
    pub freshness_policy: MetricDuration,
}

impl MetricContract {
    pub fn new(definition: MetricContractDefinition) -> Result<Self, MetricError> {
        if !definition.target.value().is_finite() {
            return Err(MetricError::NonFiniteTarget);
        }
        let description = normalize_body(&definition.body);
        let revision = ContractRevisionContent {
            schema: CONTRACT_SCHEMA,
            id: &definition.identity.metric_id,
            instrument: &definition.instrument,
            unit: &definition.unit,
            target: &definition.target,
            window: definition.window.as_str(),
            freshness: definition.freshness_policy.as_str(),
            body: &description,
        };
        let canonical = serde_json::to_vec(&revision)
            .map_err(|error| MetricError::RevisionEncoding(error.to_string()))?;
        let contract_revision = hex::encode(Sha256::digest(canonical));
        Ok(Self {
            identity: definition.identity,
            contract_revision,
            name: definition.name,
            description,
            project_id: definition.project_id,
            stage: definition.stage,
            instrument: definition.instrument,
            unit: definition.unit,
            target: definition.target,
            window: definition.window,
            freshness_policy: definition.freshness_policy,
        })
    }
}

/// A valid contract paired with the Markdown file that authored it.
#[derive(Debug, Clone)]
pub struct LoadedMetricContract {
    pub path: PathBuf,
    pub contract: MetricContract,
}

/// The valid contracts and independent configuration problems discovered for a
/// Wave. Invalid siblings never suppress a valid contract.
#[derive(Debug, Clone)]
pub struct MetricContractDiscovery {
    pub contracts: Vec<LoadedMetricContract>,
    pub contract_issues: Vec<MetricContractIssueDto>,
}

/// Load the reviewed Markdown contracts in one Wave's `metrics/` directory.
///
/// The caller supplies the current PM Project ids. Instrument binding belongs
/// to observation storage, where it can be checked against this exact metric
/// identity rather than a misleading global set of names.
pub fn discover_metric_contracts(
    metrics_dir: &Path,
    wave_id: &str,
    project_ids: &BTreeSet<String>,
) -> Result<MetricContractDiscovery, MetricError> {
    let metadata = match fs::symlink_metadata(metrics_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MetricContractDiscovery {
                contracts: Vec::new(),
                contract_issues: Vec::new(),
            });
        }
        Err(error) => {
            return Err(MetricError::ContractDirectoryRead {
                path: metrics_dir.display().to_string(),
                message: error.to_string(),
            });
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(MetricError::SymlinkedContractDirectory(
            metrics_dir.display().to_string(),
        ));
    }
    if !metadata.is_dir() {
        return Err(MetricError::ContractDirectoryNotDirectory(
            metrics_dir.display().to_string(),
        ));
    }
    if let Some(repository_root) = contract_repository_root(metrics_dir) {
        let canonical_root =
            repository_root
                .canonicalize()
                .map_err(|error| MetricError::ContractDirectoryRead {
                    path: repository_root.display().to_string(),
                    message: error.to_string(),
                })?;
        let canonical_metrics =
            metrics_dir
                .canonicalize()
                .map_err(|error| MetricError::ContractDirectoryRead {
                    path: metrics_dir.display().to_string(),
                    message: error.to_string(),
                })?;
        if !canonical_metrics.starts_with(&canonical_root) {
            return Err(MetricError::ContractDirectoryOutsideRepository {
                path: metrics_dir.display().to_string(),
                repository: repository_root.display().to_string(),
            });
        }
    }

    let entries =
        fs::read_dir(metrics_dir).map_err(|error| MetricError::ContractDirectoryRead {
            path: metrics_dir.display().to_string(),
            message: error.to_string(),
        })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| MetricError::ContractDirectoryRead {
            path: metrics_dir.display().to_string(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut sources = Vec::new();
    let mut contract_issues = Vec::new();
    for path in paths {
        match load_metric_contract(&path, wave_id) {
            Ok(contract) => sources.push(LoadedMetricContract { path, contract }),
            Err(error) => contract_issues.push(MetricContractIssueDto::MalformedContract {
                path: contract_issue_path(metrics_dir, &path),
                message: error.to_string(),
            }),
        }
    }

    let mut contracts = Vec::new();
    for source in sources {
        if !project_ids.contains(&source.contract.project_id) {
            contract_issues.push(MetricContractIssueDto::UnresolvedOwner {
                wave_id: source.contract.identity.wave_id.clone(),
                metric_id: source.contract.identity.metric_id.clone(),
                project_id: source.contract.project_id.clone(),
            });
            continue;
        }
        contracts.push(source);
    }

    contract_issues.sort_by_key(contract_issue_sort_key);
    Ok(MetricContractDiscovery {
        contracts,
        contract_issues,
    })
}

fn contract_issue_path(metrics_dir: &Path, path: &Path) -> String {
    let repository_root = contract_repository_root(metrics_dir);
    repository_root
        .and_then(|root| path.strip_prefix(root).ok())
        .or_else(|| {
            metrics_dir
                .parent()
                .and_then(|parent| path.strip_prefix(parent).ok())
        })
        .unwrap_or(path)
        .display()
        .to_string()
}

fn contract_repository_root(metrics_dir: &Path) -> Option<&Path> {
    metrics_dir
        .parent()
        .and_then(Path::parent)
        .filter(|parent| parent.file_name().is_some_and(|name| name == "wave"))
        .and_then(Path::parent)
}

/// Parse one reviewed metric contract. Contract discovery turns failures into
/// a per-file issue so one malformed Markdown file cannot hide its siblings.
pub fn load_metric_contract(path: &Path, wave_id: &str) -> Result<MetricContract, MetricError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| MetricError::ContractRead {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(MetricError::SymlinkedContract(path.display().to_string()));
    }
    if !metadata.is_file() {
        return Err(MetricError::ContractNotFile(path.display().to_string()));
    }
    let content = fs::read_to_string(path).map_err(|error| MetricError::ContractRead {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let (frontmatter, markdown) = crate::engine::flow::split_frontmatter(&content)
        .ok_or(MetricError::MissingContractFrontmatter)?;
    let frontmatter: MetricContractFrontmatter = serde_yaml_ng::from_str(&frontmatter)
        .map_err(|error| MetricError::InvalidContractFrontmatter(error.to_string()))?;
    if frontmatter.schema != 1 {
        return Err(MetricError::UnsupportedContractSchema(frontmatter.schema));
    }
    if wave_id.trim().is_empty() {
        return Err(MetricError::EmptyWaveId);
    }
    require_contract_field("id", &frontmatter.id)?;
    require_contract_field("project_id", &frontmatter.project_id)?;
    require_contract_field("instrument", &frontmatter.instrument)?;
    require_contract_field("unit", &frontmatter.unit)?;

    let file_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| MetricError::InvalidContractFilename(path.display().to_string()))?;
    if file_name != frontmatter.id {
        return Err(MetricError::ContractIdDoesNotMatchFilename {
            id: frontmatter.id,
            filename: file_name.to_string(),
        });
    }
    let (name, body) = parse_metric_markdown(&markdown)?;

    MetricContract::new(MetricContractDefinition {
        identity: MetricIdentity {
            wave_id: wave_id.to_string(),
            metric_id: frontmatter.id,
        },
        name,
        project_id: frontmatter.project_id,
        stage: frontmatter.stage,
        instrument: frontmatter.instrument,
        unit: frontmatter.unit,
        target: frontmatter.target.into_target(),
        window: MetricDuration::parse(&frontmatter.window)?,
        freshness_policy: MetricDuration::parse(&frontmatter.freshness)?,
        body,
    })
}

fn require_contract_field(field: &'static str, value: &str) -> Result<(), MetricError> {
    if value.trim().is_empty() {
        return Err(MetricError::EmptyContractField(field));
    }
    Ok(())
}

fn parse_metric_markdown(markdown: &str) -> Result<(String, String), MetricError> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut fence = None;
    let mut headings = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some("```");
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some("~~~");
            continue;
        }
        if let Some(heading) = line.strip_prefix("# ") {
            headings.push((index, heading));
        }
    }
    let [(heading_index, heading)] = headings.as_slice() else {
        return Err(MetricError::ExpectedExactlyOneH1);
    };
    if lines[..*heading_index]
        .iter()
        .any(|line| !line.trim().is_empty())
    {
        return Err(MetricError::HeadingMustPrecedeBody);
    }
    let name = heading.trim();
    if name.is_empty() {
        return Err(MetricError::EmptyMetricName);
    }
    Ok((name.to_string(), lines[*heading_index + 1..].join("\n")))
}

fn contract_issue_sort_key(issue: &MetricContractIssueDto) -> (u8, String, String) {
    match issue {
        MetricContractIssueDto::MalformedContract { path, .. } => (0, path.clone(), String::new()),
        MetricContractIssueDto::UnresolvedOwner {
            wave_id, metric_id, ..
        } => (1, wave_id.clone(), metric_id.clone()),
        MetricContractIssueDto::InstrumentMismatch {
            wave_id, metric_id, ..
        } => (2, wave_id.clone(), metric_id.clone()),
        MetricContractIssueDto::InvalidGraduation {
            wave_id, metric_id, ..
        } => (3, wave_id.clone(), metric_id.clone()),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetricContractFrontmatter {
    schema: u8,
    id: String,
    project_id: String,
    stage: MetricStage,
    instrument: String,
    unit: String,
    target: MetricTargetFrontmatter,
    window: String,
    freshness: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MetricTargetFrontmatter {
    AtLeast(AtLeastTargetFrontmatter),
    AtMost(AtMostTargetFrontmatter),
}

impl MetricTargetFrontmatter {
    fn into_target(self) -> MetricTarget {
        match self {
            Self::AtLeast(target) => MetricTarget::AtLeast {
                value: target.at_least,
            },
            Self::AtMost(target) => MetricTarget::AtMost {
                value: target.at_most,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AtLeastTargetFrontmatter {
    at_least: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AtMostTargetFrontmatter {
    at_most: f64,
}

#[derive(Serialize)]
struct ContractRevisionContent<'a> {
    schema: &'static str,
    id: &'a str,
    instrument: &'a str,
    unit: &'a str,
    target: &'a MetricTarget,
    window: &'a str,
    freshness: &'a str,
    body: &'a str,
}

fn normalize_body(body: &str) -> String {
    body.replace("\r\n", "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricObservation {
    Observed {
        identity: MetricIdentity,
        contract_revision: String,
        instrument: String,
        observation_id: String,
        value: f64,
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
        complete: bool,
    },
    Unavailable {
        identity: MetricIdentity,
        contract_revision: String,
        instrument: String,
        observation_id: String,
        #[serde(with = "time::serde::rfc3339")]
        source_as_of: OffsetDateTime,
        reason: String,
    },
}

impl MetricObservation {
    pub(crate) fn identity(&self) -> &MetricIdentity {
        match self {
            Self::Observed { identity, .. } | Self::Unavailable { identity, .. } => identity,
        }
    }

    pub(crate) fn observation_id(&self) -> &str {
        match self {
            Self::Observed { observation_id, .. } | Self::Unavailable { observation_id, .. } => {
                observation_id
            }
        }
    }

    pub(crate) fn contract_revision(&self) -> &str {
        match self {
            Self::Observed {
                contract_revision, ..
            }
            | Self::Unavailable {
                contract_revision, ..
            } => contract_revision,
        }
    }

    pub(crate) fn instrument(&self) -> &str {
        match self {
            Self::Observed { instrument, .. } | Self::Unavailable { instrument, .. } => instrument,
        }
    }

    pub(crate) fn source_time(&self) -> OffsetDateTime {
        match self {
            Self::Observed {
                source_window_end, ..
            } => *source_window_end,
            Self::Unavailable { source_as_of, .. } => *source_as_of,
        }
    }

    pub(crate) fn qualifies_graduation(&self, contract: &MetricContract) -> bool {
        matches!(
            self,
            Self::Observed {
                identity,
                contract_revision,
                instrument,
                source_window_start,
                source_window_end,
                complete: true,
                ..
            } if identity == &contract.identity
                && contract_revision == &contract.contract_revision
                && instrument == &contract.instrument
                && *source_window_end - *source_window_start == contract.window.elapsed()
        )
    }

    /// The content-addressed id an observation must carry.
    pub fn expected_observation_id(&self) -> Result<String, MetricError> {
        Ok(hex::encode(self.digest()?))
    }

    fn digest(&self) -> Result<[u8; 32], MetricError> {
        let payload = match self {
            Self::Observed {
                identity,
                contract_revision,
                instrument,
                value,
                source_window_start,
                source_window_end,
                complete,
                ..
            } => MetricObservationPayload::Observed {
                identity,
                contract_revision,
                instrument,
                value: *value,
                source_window_start,
                source_window_end,
                complete: *complete,
            },
            Self::Unavailable {
                identity,
                contract_revision,
                instrument,
                source_as_of,
                reason,
                ..
            } => MetricObservationPayload::Unavailable {
                identity,
                contract_revision,
                instrument,
                source_as_of,
                reason,
            },
        };
        let payload = serde_json::to_vec(&payload)
            .map_err(|error| MetricError::ObservationEncoding(error.to_string()))?;
        Ok(Sha256::digest(payload).into())
    }

    pub(crate) fn validate(
        &self,
        contract: &MetricContract,
        received_at: OffsetDateTime,
    ) -> Result<(), MetricError> {
        if self.identity() != &contract.identity {
            return Err(MetricError::ObservationIdentityMismatch {
                expected: contract.identity.clone(),
                observed: self.identity().clone(),
            });
        }
        let expected_observation_id = self.expected_observation_id()?;
        if self.observation_id() != expected_observation_id {
            return Err(MetricError::InvalidObservationId {
                expected: expected_observation_id,
                observed: self.observation_id().to_string(),
            });
        }
        if self.instrument() != contract.instrument {
            return Err(MetricError::InstrumentMismatch {
                expected: contract.instrument.clone(),
                observed: self.instrument().to_string(),
            });
        }
        let latest_allowed = received_at
            .checked_add(FUTURE_SOURCE_ALLOWANCE)
            .ok_or(MetricError::TimeOverflow)?;
        if self.source_time() > latest_allowed {
            return Err(MetricError::FutureSourceTime {
                source_time: self.source_time(),
                latest_allowed,
            });
        }
        match self {
            Self::Observed {
                value,
                source_window_start,
                source_window_end,
                ..
            } => {
                if !value.is_finite() {
                    return Err(MetricError::NonFiniteObservation);
                }
                if source_window_start > source_window_end {
                    return Err(MetricError::InvalidWindow {
                        start: *source_window_start,
                        end: *source_window_end,
                    });
                }
            }
            Self::Unavailable { reason, .. } if reason.trim().is_empty() => {
                return Err(MetricError::EmptyUnavailableReason);
            }
            Self::Unavailable { .. } => {}
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MetricObservationPayload<'a> {
    Observed {
        identity: &'a MetricIdentity,
        contract_revision: &'a str,
        instrument: &'a str,
        value: f64,
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: &'a OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: &'a OffsetDateTime,
        complete: bool,
    },
    Unavailable {
        identity: &'a MetricIdentity,
        contract_revision: &'a str,
        instrument: &'a str,
        #[serde(with = "time::serde::rfc3339")]
        source_as_of: &'a OffsetDateTime,
        reason: &'a str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationAcceptance {
    Accepted,
    Duplicate,
}

/// The bounded persisted evidence required to derive one contract's reading.
#[derive(Debug, Clone, Default)]
pub struct MetricObservationEvidence {
    pub(crate) current: Option<MetricObservation>,
    pub(crate) instrumented: bool,
    pub(crate) graduation_qualified: bool,
}

impl MetricObservationEvidence {
    fn from_history(
        contract: &MetricContract,
        observations: &[MetricObservation],
    ) -> Result<Self, MetricError> {
        Ok(Self {
            current: select_current(contract, observations)?.cloned(),
            instrumented: observations.iter().any(|observation| {
                observation.identity() == &contract.identity
                    && observation.contract_revision() == contract.contract_revision
            }),
            graduation_qualified: observations
                .iter()
                .any(|observation| observation.qualifies_graduation(contract)),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPortfolioDto {
    pub metrics: Vec<MetricReadingDto>,
    pub contract_issues: Vec<MetricContractIssueDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricReadingDto {
    pub identity: MetricIdentity,
    pub contract_revision: String,
    pub name: String,
    pub description: String,
    pub project_id: String,
    pub stage: MetricStage,
    /// Whether any accepted observation measured this exact contract revision.
    pub instrumented: bool,
    pub instrument: String,
    pub unit: String,
    pub target: MetricTarget,
    pub window: String,
    pub freshness_policy: String,
    pub freshness: MetricFreshnessDto,
    pub evidence: MetricEvidenceDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricFreshnessDto {
    Never,
    Fresh {
        #[serde(with = "time::serde::rfc3339")]
        source_time: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        expires_at: OffsetDateTime,
    },
    Stale {
        #[serde(with = "time::serde::rfc3339")]
        source_time: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        expires_at: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricEvidenceDto {
    Met {
        value: f64,
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    Missed {
        value: f64,
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    Unknown {
        cause: MetricUnknownCauseDto,
    },
    Unavailable {
        reason: String,
        #[serde(with = "time::serde::rfc3339")]
        source_as_of: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricUnknownCauseDto {
    Never,
    RevisionMismatch {
        expected_contract_revision: String,
        observed_contract_revision: String,
        #[serde(with = "time::serde::rfc3339")]
        source_time: OffsetDateTime,
    },
    Incomplete {
        value: f64,
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    WindowMismatch {
        value: f64,
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    StaleObservation {
        value: f64,
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    StaleUnavailable {
        reason: String,
        #[serde(with = "time::serde::rfc3339")]
        source_as_of: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricContractIssueDto {
    MalformedContract {
        path: String,
        message: String,
    },
    UnresolvedOwner {
        wave_id: String,
        metric_id: String,
        project_id: String,
    },
    InstrumentMismatch {
        wave_id: String,
        metric_id: String,
        contract_instrument: String,
        registered_instrument: String,
    },
    InvalidGraduation {
        wave_id: String,
        metric_id: String,
        contract_revision: String,
        reason: String,
    },
}

pub fn derive_metric_reading(
    contract: &MetricContract,
    observations: &[MetricObservation],
    evaluation_time: OffsetDateTime,
) -> Result<MetricReadingDto, MetricError> {
    let persisted = MetricObservationEvidence::from_history(contract, observations)?;
    derive_metric_reading_from_evidence(contract, &persisted, evaluation_time)
}

fn derive_metric_reading_from_evidence(
    contract: &MetricContract,
    persisted: &MetricObservationEvidence,
    evaluation_time: OffsetDateTime,
) -> Result<MetricReadingDto, MetricError> {
    let (freshness, evidence) = match persisted.current.as_ref() {
        None => (
            MetricFreshnessDto::Never,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::Never,
            },
        ),
        Some(observation) => derive_evidence(contract, observation, evaluation_time)?,
    };
    Ok(MetricReadingDto {
        identity: contract.identity.clone(),
        contract_revision: contract.contract_revision.clone(),
        name: contract.name.clone(),
        description: contract.description.clone(),
        project_id: contract.project_id.clone(),
        stage: contract.stage,
        instrumented: persisted.instrumented,
        instrument: contract.instrument.clone(),
        unit: contract.unit.clone(),
        target: contract.target.clone(),
        window: contract.window.as_str().to_string(),
        freshness_policy: contract.freshness_policy.as_str().to_string(),
        freshness,
        evidence,
    })
}

pub fn derive_metric_portfolio(
    entries: &[(&MetricContract, &[MetricObservation])],
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto, MetricError> {
    let mut metrics = Vec::new();
    let mut contract_issues = Vec::new();
    for (contract, observations) in entries {
        let persisted = MetricObservationEvidence::from_history(contract, observations)?;
        let reading = derive_metric_reading_from_evidence(contract, &persisted, evaluation_time)?;
        if contract.stage == MetricStage::Graduated && !persisted.graduation_qualified {
            contract_issues.push(MetricContractIssueDto::InvalidGraduation {
                wave_id: contract.identity.wave_id.clone(),
                metric_id: contract.identity.metric_id.clone(),
                contract_revision: contract.contract_revision.clone(),
                reason: "no_qualifying_observation".to_string(),
            });
            continue;
        }
        metrics.push(reading);
    }
    Ok(MetricPortfolioDto {
        metrics,
        contract_issues,
    })
}

/// Join reviewed contracts to registered producers and accepted observations.
/// Invalid contracts and producer mismatches remain isolated from valid siblings.
pub fn compose_metric_portfolio(
    discovery: MetricContractDiscovery,
    registered_instruments: &BTreeMap<MetricIdentity, String>,
    observations: &BTreeMap<MetricIdentity, MetricObservationEvidence>,
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto, MetricError> {
    let mut metrics = Vec::new();
    let mut contract_issues = discovery.contract_issues;
    for source in discovery.contracts {
        let contract = source.contract;
        if let Some(registered_instrument) = registered_instruments.get(&contract.identity) {
            if registered_instrument != &contract.instrument {
                contract_issues.push(MetricContractIssueDto::InstrumentMismatch {
                    wave_id: contract.identity.wave_id.clone(),
                    metric_id: contract.identity.metric_id.clone(),
                    contract_instrument: contract.instrument.clone(),
                    registered_instrument: registered_instrument.clone(),
                });
                continue;
            }
        }
        let persisted = observations
            .get(&contract.identity)
            .cloned()
            .unwrap_or_default();
        let reading = derive_metric_reading_from_evidence(&contract, &persisted, evaluation_time)?;
        if contract.stage == MetricStage::Graduated && !persisted.graduation_qualified {
            contract_issues.push(MetricContractIssueDto::InvalidGraduation {
                wave_id: contract.identity.wave_id.clone(),
                metric_id: contract.identity.metric_id.clone(),
                contract_revision: contract.contract_revision.clone(),
                reason: "no_qualifying_observation".to_string(),
            });
            continue;
        }
        metrics.push(reading);
    }
    contract_issues.sort_by_key(contract_issue_sort_key);
    Ok(MetricPortfolioDto {
        metrics,
        contract_issues,
    })
}

fn select_current<'a>(
    contract: &MetricContract,
    observations: &'a [MetricObservation],
) -> Result<Option<&'a MetricObservation>, MetricError> {
    let mut current: Option<(&MetricObservation, [u8; 32])> = None;
    for observation in observations {
        if observation.identity() != &contract.identity {
            continue;
        }
        let digest = observation.digest()?;
        let replace =
            current.as_ref().is_none_or(|(selected, selected_digest)| {
                match observation.source_time().cmp(&selected.source_time()) {
                    Ordering::Greater => true,
                    Ordering::Equal => digest > *selected_digest,
                    Ordering::Less => false,
                }
            });
        if replace {
            current = Some((observation, digest));
        }
    }
    Ok(current.map(|(observation, _)| observation))
}

fn derive_evidence(
    contract: &MetricContract,
    observation: &MetricObservation,
    evaluation_time: OffsetDateTime,
) -> Result<(MetricFreshnessDto, MetricEvidenceDto), MetricError> {
    let source_time = observation.source_time();
    if observation.contract_revision() != contract.contract_revision {
        return Ok((
            MetricFreshnessDto::Never,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::RevisionMismatch {
                    expected_contract_revision: contract.contract_revision.clone(),
                    observed_contract_revision: observation.contract_revision().to_string(),
                    source_time,
                },
            },
        ));
    }
    let expires_at = source_time
        .checked_add(contract.freshness_policy.elapsed())
        .ok_or(MetricError::TimeOverflow)?;
    let fresh = evaluation_time < expires_at;
    let freshness = if fresh {
        MetricFreshnessDto::Fresh {
            source_time,
            expires_at,
        }
    } else {
        MetricFreshnessDto::Stale {
            source_time,
            expires_at,
        }
    };
    let evidence = match observation {
        MetricObservation::Observed {
            value,
            source_window_start,
            source_window_end,
            complete,
            ..
        } if !fresh => MetricEvidenceDto::Unknown {
            cause: MetricUnknownCauseDto::StaleObservation {
                value: *value,
                source_window_start: *source_window_start,
                source_window_end: *source_window_end,
            },
        },
        MetricObservation::Observed {
            value,
            source_window_start,
            source_window_end,
            complete: false,
            ..
        } => MetricEvidenceDto::Unknown {
            cause: MetricUnknownCauseDto::Incomplete {
                value: *value,
                source_window_start: *source_window_start,
                source_window_end: *source_window_end,
            },
        },
        MetricObservation::Observed {
            value,
            source_window_start,
            source_window_end,
            complete: true,
            ..
        } if *source_window_end - *source_window_start != contract.window.elapsed() => {
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::WindowMismatch {
                    value: *value,
                    source_window_start: *source_window_start,
                    source_window_end: *source_window_end,
                },
            }
        }
        MetricObservation::Observed {
            value,
            source_window_start,
            source_window_end,
            complete: true,
            ..
        } if contract.target.is_met(*value) => MetricEvidenceDto::Met {
            value: *value,
            source_window_start: *source_window_start,
            source_window_end: *source_window_end,
        },
        MetricObservation::Observed {
            value,
            source_window_start,
            source_window_end,
            complete: true,
            ..
        } => MetricEvidenceDto::Missed {
            value: *value,
            source_window_start: *source_window_start,
            source_window_end: *source_window_end,
        },
        MetricObservation::Unavailable {
            source_as_of,
            reason,
            ..
        } if fresh => MetricEvidenceDto::Unavailable {
            reason: reason.clone(),
            source_as_of: *source_as_of,
        },
        MetricObservation::Unavailable {
            source_as_of,
            reason,
            ..
        } => MetricEvidenceDto::Unknown {
            cause: MetricUnknownCauseDto::StaleUnavailable {
                reason: reason.clone(),
                source_as_of: *source_as_of,
            },
        },
    };
    Ok((freshness, evidence))
}

#[derive(Debug, thiserror::Error)]
pub enum MetricError {
    #[error("failed to enumerate metric contracts under {path:?}: {message}")]
    ContractDirectoryRead { path: String, message: String },
    #[error("metric contract directory must be reviewed in-repository, not a symlink: {0}")]
    SymlinkedContractDirectory(String),
    #[error("metric contract directory path is not a directory: {0}")]
    ContractDirectoryNotDirectory(String),
    #[error("metric contract directory {path:?} resolves outside repository {repository:?}")]
    ContractDirectoryOutsideRepository { path: String, repository: String },
    #[error("failed to read metric contract {path:?}: {message}")]
    ContractRead { path: String, message: String },
    #[error("metric contract must be a reviewed regular file, not a symlink: {0}")]
    SymlinkedContract(String),
    #[error("metric contract path is not a regular file: {0}")]
    ContractNotFile(String),
    #[error("metric contract is missing YAML frontmatter")]
    MissingContractFrontmatter,
    #[error("invalid metric contract frontmatter: {0}")]
    InvalidContractFrontmatter(String),
    #[error("unsupported metric contract schema {0}; expected 1")]
    UnsupportedContractSchema(u8),
    #[error("metric contract wave id must not be empty")]
    EmptyWaveId,
    #[error("metric contract field {0:?} must not be empty")]
    EmptyContractField(&'static str),
    #[error("metric contract filename is not valid UTF-8: {0}")]
    InvalidContractFilename(String),
    #[error("metric contract id {id:?} does not match filename {filename:?}")]
    ContractIdDoesNotMatchFilename { id: String, filename: String },
    #[error("metric contract Markdown must contain exactly one H1")]
    ExpectedExactlyOneH1,
    #[error("metric contract H1 must precede all non-empty Markdown body content")]
    HeadingMustPrecedeBody,
    #[error("metric contract H1 must name the metric")]
    EmptyMetricName,
    #[error("invalid metric duration {0:?}; use a positive number followed by m, h, or d")]
    InvalidDuration(String),
    #[error("metric target must be finite")]
    NonFiniteTarget,
    #[error("metric observation value must be finite")]
    NonFiniteObservation,
    #[error("metric observation window starts after it ends: {start} > {end}")]
    InvalidWindow {
        start: OffsetDateTime,
        end: OffsetDateTime,
    },
    #[error("metric source time {source_time} exceeds the clock-skew boundary {latest_allowed}")]
    FutureSourceTime {
        source_time: OffsetDateTime,
        latest_allowed: OffsetDateTime,
    },
    #[error("metric instrument mismatch: expected {expected:?}, observed {observed:?}")]
    InstrumentMismatch { expected: String, observed: String },
    #[error("metric observation identity mismatch: expected {expected:?}, observed {observed:?}")]
    ObservationIdentityMismatch {
        expected: MetricIdentity,
        observed: MetricIdentity,
    },
    #[error("unavailable metric evidence requires a reason")]
    EmptyUnavailableReason,
    #[error("metric observation id must equal its content digest: expected {expected}, observed {observed}")]
    InvalidObservationId { expected: String, observed: String },
    #[error("metric time calculation overflowed")]
    TimeOverflow,
    #[error("failed to encode metric contract revision: {0}")]
    RevisionEncoding(String),
    #[error("failed to encode metric observation: {0}")]
    ObservationEncoding(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::hours(hour)
    }

    fn definition() -> MetricContractDefinition {
        MetricContractDefinition {
            identity: MetricIdentity {
                wave_id: "product".to_string(),
                metric_id: "task-loop-trust".to_string(),
            },
            name: "Task loops earn trust".to_string(),
            project_id: "project-a".to_string(),
            stage: MetricStage::Installed,
            instrument: "lifecycle-scorecard".to_string(),
            unit: "ratio".to_string(),
            target: MetricTarget::AtLeast { value: 1.0 },
            window: MetricDuration::parse("7d").unwrap(),
            freshness_policy: MetricDuration::parse("6h").unwrap(),
            body: "Count only settled review loops.\n".to_string(),
        }
    }

    fn contract() -> MetricContract {
        MetricContract::new(definition()).unwrap()
    }

    fn contract_markdown(id: &str, project_id: &str, instrument: &str) -> String {
        format!(
            "---\nschema: 1\nid: {id}\nproject_id: {project_id}\nstage: installed\ninstrument: {instrument}\nunit: ratio\ntarget:\n  at_least: 1\nwindow: 7d\nfreshness: 6h\n---\n\n# Task loops earn trust\n\nCount only settled review loops.\n"
        )
    }

    fn observed(contract: &MetricContract, value: f64, end: OffsetDateTime) -> MetricObservation {
        let mut observation = MetricObservation::Observed {
            identity: contract.identity.clone(),
            contract_revision: contract.contract_revision.clone(),
            instrument: "lifecycle-scorecard".to_string(),
            observation_id: String::new(),
            value,
            source_window_start: end - Duration::days(7),
            source_window_end: end,
            complete: true,
        };
        recompute_observation_id(&mut observation);
        observation
    }

    fn unavailable(contract: &MetricContract, as_of: OffsetDateTime) -> MetricObservation {
        let mut observation = MetricObservation::Unavailable {
            identity: contract.identity.clone(),
            contract_revision: contract.contract_revision.clone(),
            instrument: contract.instrument.clone(),
            observation_id: String::new(),
            source_as_of: as_of,
            reason: "source timeout".to_string(),
        };
        recompute_observation_id(&mut observation);
        observation
    }

    fn recompute_observation_id(observation: &mut MetricObservation) {
        let observation_id = observation.expected_observation_id().unwrap();
        match observation {
            MetricObservation::Observed {
                observation_id: id, ..
            }
            | MetricObservation::Unavailable {
                observation_id: id, ..
            } => *id = observation_id,
        }
    }

    #[test]
    fn contract_revision_tracks_measurement_meaning_not_presentation_or_ownership() {
        let original = contract();
        let mut moved = definition();
        moved.name = "Renamed metric".to_string();
        moved.project_id = "project-b".to_string();
        moved.stage = MetricStage::Graduated;
        assert_eq!(
            MetricContract::new(moved).unwrap().contract_revision,
            original.contract_revision
        );

        let mut changed_target = definition();
        changed_target.target = MetricTarget::AtLeast { value: 0.9 };
        assert_ne!(
            MetricContract::new(changed_target)
                .unwrap()
                .contract_revision,
            original.contract_revision
        );

        let mut changed_rule = definition();
        changed_rule.body = "Count all review loops.".to_string();
        assert_ne!(
            MetricContract::new(changed_rule).unwrap().contract_revision,
            original.contract_revision
        );
    }

    #[test]
    fn duration_overflow_is_rejected_for_every_supported_unit() {
        for (suffix, seconds_per_unit) in [("m", 60_i64), ("h", 3_600), ("d", 86_400)] {
            let largest = MAX_METRIC_DURATION_SECONDS / seconds_per_unit;
            assert!(MetricDuration::parse(&format!("{largest}{suffix}")).is_ok());
            assert!(matches!(
                MetricDuration::parse(&format!("{}{suffix}", largest + 1)),
                Err(MetricError::InvalidDuration(_))
            ));
            assert!(matches!(
                MetricDuration::parse(&format!("{}{suffix}", i64::MAX)),
                Err(MetricError::InvalidDuration(_))
            ));
        }
        for malformed in ["", "7", "1s", "1💥", "💥"] {
            assert!(matches!(
                MetricDuration::parse(malformed),
                Err(MetricError::InvalidDuration(_))
            ));
        }
    }

    #[test]
    fn target_and_freshness_boundaries_are_inclusive_and_exact() {
        let contract = contract();
        let evidence = observed(&contract, 1.0, at(200));
        let fresh = derive_metric_reading(
            &contract,
            std::slice::from_ref(&evidence),
            at(206) - Duration::SECOND,
        )
        .unwrap();
        assert!(fresh.instrumented);
        assert!(matches!(fresh.freshness, MetricFreshnessDto::Fresh { .. }));
        assert!(matches!(fresh.evidence, MetricEvidenceDto::Met { .. }));

        let stale = derive_metric_reading(&contract, &[evidence], at(206)).unwrap();
        assert!(matches!(stale.freshness, MetricFreshnessDto::Stale { .. }));
        assert!(matches!(
            stale.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::StaleObservation { .. }
            }
        ));
    }

    #[test]
    fn prior_revision_and_wrong_windows_are_unknown_without_reinterpretation() {
        let contract = contract();
        let mut prior = observed(&contract, 1.0, at(200));
        let MetricObservation::Observed {
            contract_revision, ..
        } = &mut prior
        else {
            unreachable!();
        };
        *contract_revision = "0".repeat(64);
        recompute_observation_id(&mut prior);
        let reading = derive_metric_reading(&contract, &[prior], at(201)).unwrap();
        assert!(matches!(
            reading.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::RevisionMismatch { .. }
            }
        ));
        assert!(!reading.instrumented);

        let mut prior_definition = definition();
        prior_definition.freshness_policy = MetricDuration::parse("1h").unwrap();
        let prior_contract = MetricContract::new(prior_definition).unwrap();
        let prior = observed(&prior_contract, 1.0, at(200));
        let reading = derive_metric_reading(&contract, &[prior], at(206)).unwrap();
        assert!(matches!(reading.freshness, MetricFreshnessDto::Never));
        assert!(matches!(
            reading.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::RevisionMismatch { .. }
            }
        ));

        let mut wrong_window = observed(&contract, 1.0, at(200));
        let MetricObservation::Observed {
            source_window_start,
            ..
        } = &mut wrong_window
        else {
            unreachable!();
        };
        *source_window_start = at(200) - Duration::days(6);
        recompute_observation_id(&mut wrong_window);
        let reading = derive_metric_reading(&contract, &[wrong_window], at(201)).unwrap();
        assert!(matches!(
            reading.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::WindowMismatch { .. }
            }
        ));
    }

    #[test]
    fn validation_rejects_bad_time_and_tampered_content_ids() {
        let contract = contract();
        let first = observed(&contract, 1.0, at(200));
        first.validate(&contract, at(200)).unwrap();
        let distinct = observed(&contract, 0.5, at(200));
        distinct.validate(&contract, at(200)).unwrap();

        let mut tampered = observed(&contract, 0.5, at(201));
        let MetricObservation::Observed { observation_id, .. } = &mut tampered else {
            unreachable!();
        };
        *observation_id = "producer-chosen-id".to_string();
        assert!(matches!(
            tampered.validate(&contract, at(201)),
            Err(MetricError::InvalidObservationId { .. })
        ));

        let mut other_metric = observed(&contract, 0.5, at(201));
        let MetricObservation::Observed { identity, .. } = &mut other_metric else {
            unreachable!();
        };
        identity.metric_id = "other-metric".to_string();
        recompute_observation_id(&mut other_metric);
        assert!(matches!(
            other_metric.validate(&contract, at(201)),
            Err(MetricError::ObservationIdentityMismatch { .. })
        ));

        let future = observed(
            &contract,
            1.0,
            at(200) + Duration::minutes(5) + Duration::SECOND,
        );
        assert!(matches!(
            future.validate(&contract, at(200)),
            Err(MetricError::FutureSourceTime { .. })
        ));
    }

    #[test]
    fn source_time_and_content_digest_select_current_evidence() {
        let contract = contract();
        let missed = observed(&contract, 0.5, at(200));
        let met = observed(&contract, 1.0, at(200));
        let expected_value = if missed.observation_id() > met.observation_id() {
            0.5
        } else {
            1.0
        };
        let forward =
            derive_metric_reading(&contract, &[missed.clone(), met.clone()], at(201)).unwrap();
        let reverse = derive_metric_reading(&contract, &[met, missed], at(201)).unwrap();
        for reading in [forward, reverse] {
            let value = match reading.evidence {
                MetricEvidenceDto::Met { value, .. } | MetricEvidenceDto::Missed { value, .. } => {
                    value
                }
                evidence => panic!("expected current numeric evidence, got {evidence:?}"),
            };
            assert_eq!(value, expected_value);
        }
    }

    #[test]
    fn projection_ignores_observations_addressed_to_other_metrics() {
        let contract = contract();
        let mut other_metric = observed(&contract, 1.0, at(200));
        let MetricObservation::Observed { identity, .. } = &mut other_metric else {
            unreachable!();
        };
        identity.metric_id = "other-metric".to_string();
        recompute_observation_id(&mut other_metric);

        let reading = derive_metric_reading(&contract, &[other_metric], at(201)).unwrap();
        assert!(!reading.instrumented);
        assert!(matches!(
            reading.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::Never
            }
        ));
    }

    #[test]
    fn graduation_without_qualifying_evidence_is_a_contract_issue() {
        let mut definition = definition();
        definition.stage = MetricStage::Graduated;
        let contract = MetricContract::new(definition).unwrap();
        let portfolio = derive_metric_portfolio(&[(&contract, &[])], at(200)).unwrap();
        assert!(portfolio.metrics.is_empty());
        assert!(matches!(
            portfolio.contract_issues.as_slice(),
            [MetricContractIssueDto::InvalidGraduation { .. }]
        ));
    }

    #[test]
    fn graduated_metric_retains_official_state_when_evidence_later_goes_stale() {
        let mut definition = definition();
        definition.stage = MetricStage::Graduated;
        let contract = MetricContract::new(definition).unwrap();
        let evidence = observed(&contract, 1.0, at(200));

        let portfolio = derive_metric_portfolio(&[(&contract, &[evidence])], at(206)).unwrap();

        assert!(portfolio.contract_issues.is_empty());
        assert!(matches!(
            portfolio.metrics.as_slice(),
            [MetricReadingDto {
                freshness: MetricFreshnessDto::Stale { .. },
                evidence: MetricEvidenceDto::Unknown {
                    cause: MetricUnknownCauseDto::StaleObservation { .. }
                },
                ..
            }]
        ));
    }

    #[test]
    fn graduated_metric_retains_official_state_during_a_current_source_failure() {
        let mut definition = definition();
        definition.stage = MetricStage::Graduated;
        let contract = MetricContract::new(definition).unwrap();
        let observations = [
            observed(&contract, 1.0, at(200)),
            unavailable(&contract, at(201)),
        ];

        let portfolio = derive_metric_portfolio(&[(&contract, &observations)], at(202)).unwrap();

        assert!(portfolio.contract_issues.is_empty());
        assert!(matches!(
            portfolio.metrics.as_slice(),
            [MetricReadingDto {
                evidence: MetricEvidenceDto::Unavailable { reason, .. },
                ..
            }] if reason == "source timeout"
        ));
    }

    #[test]
    fn wire_uses_kind_discriminators_and_requires_variant_fields() {
        let contract = contract();
        let reading =
            derive_metric_reading(&contract, &[observed(&contract, 1.0, at(200))], at(201))
                .unwrap();
        let json = serde_json::to_value(reading).unwrap();
        assert_eq!(json["description"], "Count only settled review loops.");
        assert_eq!(json["target"]["kind"], "at_least");
        assert_eq!(json["instrumented"], true);
        assert_eq!(json["freshness"]["kind"], "fresh");
        assert_eq!(json["evidence"]["kind"], "met");

        let missing_value = serde_json::json!({"kind": "met", "source_window_start": "1970-01-01T00:00:00Z", "source_window_end": "1970-01-01T00:00:00Z"});
        assert!(serde_json::from_value::<MetricEvidenceDto>(missing_value).is_err());

        let mut missing_instrumented = json;
        missing_instrumented
            .as_object_mut()
            .unwrap()
            .remove("instrumented");
        assert!(serde_json::from_value::<MetricReadingDto>(missing_instrumented).is_err());
    }

    #[test]
    fn discovers_valid_markdown_contracts_with_current_project() {
        let directory = tempfile::tempdir().unwrap();
        let metrics_dir = directory.path().join("metrics");
        fs::create_dir(&metrics_dir).unwrap();
        fs::write(
            metrics_dir.join("task-loop-trust.md"),
            contract_markdown("task-loop-trust", "project-a", "lifecycle-scorecard"),
        )
        .unwrap();

        let projects = BTreeSet::from(["project-a".to_string()]);
        let discovery = discover_metric_contracts(&metrics_dir, "product", &projects).unwrap();

        assert!(discovery.contract_issues.is_empty());
        assert_eq!(discovery.contracts.len(), 1);
        let contract = &discovery.contracts[0].contract;
        assert_eq!(contract.identity.wave_id, "product");
        assert_eq!(contract.identity.metric_id, "task-loop-trust");
        assert_eq!(contract.name, "Task loops earn trust");
        assert_eq!(contract.project_id, "project-a");
    }

    #[test]
    fn fenced_examples_do_not_create_metric_headings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task-loop-trust.md");
        let markdown = contract_markdown("task-loop-trust", "project-a", "lifecycle-scorecard")
            .replace(
                "Count only settled review loops.",
                "Count only settled review loops.\n\n```sh\n# example output\n```",
            );
        fs::write(&path, markdown).unwrap();

        let contract = load_metric_contract(&path, "product").unwrap();

        assert!(contract.description.contains("# example output"));
    }

    #[test]
    fn malformed_and_unresolved_contracts_do_not_hide_valid_siblings() {
        let directory = tempfile::tempdir().unwrap();
        let metrics_dir = directory.path().join("wave/product/metrics");
        fs::create_dir_all(&metrics_dir).unwrap();
        fs::write(
            metrics_dir.join("task-loop-trust.md"),
            contract_markdown("task-loop-trust", "project-a", "lifecycle-scorecard"),
        )
        .unwrap();
        fs::write(
            metrics_dir.join("wrong-name.md"),
            contract_markdown("different-id", "project-a", "lifecycle-scorecard"),
        )
        .unwrap();
        fs::write(
            metrics_dir.join("unowned.md"),
            contract_markdown("unowned", "missing-project", "lifecycle-scorecard"),
        )
        .unwrap();

        let projects = BTreeSet::from(["project-a".to_string()]);
        let discovery = discover_metric_contracts(&metrics_dir, "product", &projects).unwrap();

        assert_eq!(discovery.contracts.len(), 1);
        assert!(discovery.contract_issues.iter().any(|issue| {
            matches!(issue, MetricContractIssueDto::MalformedContract { path, .. } if path == "wave/product/metrics/wrong-name.md")
        }));
        assert!(discovery.contract_issues.iter().any(|issue| {
            matches!(issue, MetricContractIssueDto::UnresolvedOwner { metric_id, project_id, .. } if metric_id == "unowned" && project_id == "missing-project")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_contract_cannot_source_meaning_outside_reviewed_metrics() {
        let directory = tempfile::tempdir().unwrap();
        let metrics_dir = directory.path().join("wave/product/metrics");
        fs::create_dir_all(&metrics_dir).unwrap();
        let external_contract = directory.path().join("external.md");
        fs::write(
            &external_contract,
            contract_markdown("external", "project-a", "lifecycle-scorecard"),
        )
        .unwrap();
        std::os::unix::fs::symlink(&external_contract, metrics_dir.join("external.md")).unwrap();

        let projects = BTreeSet::from(["project-a".to_string()]);
        let discovery = discover_metric_contracts(&metrics_dir, "product", &projects).unwrap();

        assert!(discovery.contracts.is_empty());
        assert!(matches!(
            discovery.contract_issues.as_slice(),
            [MetricContractIssueDto::MalformedContract { path, message }]
                if path == "wave/product/metrics/external.md" && message.contains("symlink")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_metrics_directory_is_not_a_reviewed_contract_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        let metrics_parent = repo.join("wave/product");
        let external_metrics = directory.path().join("external-metrics");
        fs::create_dir_all(&metrics_parent).unwrap();
        fs::create_dir(&external_metrics).unwrap();
        fs::write(
            external_metrics.join("task-loop-trust.md"),
            contract_markdown("task-loop-trust", "project-a", "lifecycle-scorecard"),
        )
        .unwrap();
        let metrics_dir = metrics_parent.join("metrics");
        std::os::unix::fs::symlink(&external_metrics, &metrics_dir).unwrap();

        let error = discover_metric_contracts(
            &metrics_dir,
            "product",
            &BTreeSet::from(["project-a".to_string()]),
        )
        .unwrap_err();

        assert!(matches!(error, MetricError::SymlinkedContractDirectory(_)));
    }

    #[cfg(unix)]
    #[test]
    fn contract_directory_must_resolve_inside_the_repository() {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        let external_wave = directory.path().join("external-wave");
        fs::create_dir(&repo).unwrap();
        fs::create_dir_all(external_wave.join("product/metrics")).unwrap();
        std::os::unix::fs::symlink(&external_wave, repo.join("wave")).unwrap();
        let metrics_dir = repo.join("wave/product/metrics");

        let error = discover_metric_contracts(
            &metrics_dir,
            "product",
            &BTreeSet::from(["project-a".to_string()]),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            MetricError::ContractDirectoryOutsideRepository { .. }
        ));
    }
}
