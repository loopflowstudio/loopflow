//! Wave-owned live metric contracts, observations, and derived readings.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};

const CONTRACT_SCHEMA: &str = "loopflow.metric/v1";
const FUTURE_SOURCE_ALLOWANCE: Duration = Duration::minutes(5);
pub const MAX_METRIC_DURATION_SECONDS: i64 = 10 * 365 * 24 * 60 * 60;

/// Canonical Work association; metric names remain case sensitive.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(from = "MetricObservationIdentity")]
pub struct MetricIdentity {
    wave_id: String,
    pub metric_id: String,
}

impl MetricIdentity {
    pub fn new(wave_id: &str, metric_id: impl Into<String>) -> Self {
        Self {
            wave_id: loopflow_model::id::canonical_uuid(wave_id),
            metric_id: metric_id.into(),
        }
    }

    pub fn wave_id(&self) -> &str {
        &self.wave_id
    }
}

/// Original hash input. Its Wave spelling is evidence, never a lookup key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetricObservationIdentity {
    pub wave_id: String,
    pub metric_id: String,
}

impl From<MetricObservationIdentity> for MetricIdentity {
    fn from(identity: MetricObservationIdentity) -> Self {
        Self::new(&identity.wave_id, identity.metric_id)
    }
}

impl From<MetricIdentity> for MetricObservationIdentity {
    fn from(identity: MetricIdentity) -> Self {
        Self {
            wave_id: identity.wave_id,
            metric_id: identity.metric_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricStage {
    Installed,
    Graduated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetricTarget {
    pub metric_id: String,
    pub target: Threshold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Threshold {
    AtLeast { value: f64 },
    AtMost { value: f64 },
}

impl Threshold {
    pub fn value(&self) -> f64 {
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
    pub stage: MetricStage,
    pub instrument: String,
    pub unit: String,
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
    pub stage: MetricStage,
    pub instrument: String,
    pub unit: String,
    pub window: MetricDuration,
    pub freshness_policy: MetricDuration,
}

impl MetricContract {
    pub fn new(definition: MetricContractDefinition) -> Result<Self, MetricError> {
        let description = normalize_body(&definition.body);
        let revision = ContractRevisionContent {
            schema: CONTRACT_SCHEMA,
            id: &definition.identity.metric_id,
            instrument: &definition.instrument,
            unit: &definition.unit,
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
            stage: definition.stage,
            instrument: definition.instrument,
            unit: definition.unit,
            window: definition.window,
            freshness_policy: definition.freshness_policy,
        })
    }
}

pub fn require_contract_field(field: &'static str, value: &str) -> Result<(), MetricError> {
    if value.trim().is_empty() {
        return Err(MetricError::EmptyContractField(field));
    }
    Ok(())
}

pub fn parse_metric_markdown(markdown: &str) -> Result<(String, String), MetricError> {
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

#[derive(Serialize)]
struct ContractRevisionContent<'a> {
    schema: &'static str,
    id: &'a str,
    instrument: &'a str,
    unit: &'a str,
    window: &'a str,
    freshness: &'a str,
    body: &'a str,
}

pub fn normalize_body(body: &str) -> String {
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
        identity: MetricObservationIdentity,
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
        identity: MetricObservationIdentity,
        contract_revision: String,
        instrument: String,
        observation_id: String,
        #[serde(with = "time::serde::rfc3339")]
        source_as_of: OffsetDateTime,
        reason: String,
    },
}

impl MetricObservation {
    pub fn metric_identity(&self) -> MetricIdentity {
        match self {
            Self::Observed { identity, .. } | Self::Unavailable { identity, .. } => {
                MetricIdentity::new(&identity.wave_id, &identity.metric_id)
            }
        }
    }

    pub fn observation_id(&self) -> &str {
        match self {
            Self::Observed { observation_id, .. } | Self::Unavailable { observation_id, .. } => {
                observation_id
            }
        }
    }

    pub fn contract_revision(&self) -> &str {
        match self {
            Self::Observed {
                contract_revision, ..
            }
            | Self::Unavailable {
                contract_revision, ..
            } => contract_revision,
        }
    }

    pub fn instrument(&self) -> &str {
        match self {
            Self::Observed { instrument, .. } | Self::Unavailable { instrument, .. } => instrument,
        }
    }

    pub fn source_time(&self) -> OffsetDateTime {
        match self {
            Self::Observed {
                source_window_end, ..
            } => *source_window_end,
            Self::Unavailable { source_as_of, .. } => *source_as_of,
        }
    }

    pub fn qualifies_graduation(&self, contract: &MetricContract) -> bool {
        matches!(
            self,
            Self::Observed {
                contract_revision,
                instrument,
                source_window_start,
                source_window_end,
                complete: true,
                ..
            } if self.metric_identity() == contract.identity
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

    pub fn validate(
        &self,
        contract: &MetricContract,
        received_at: OffsetDateTime,
    ) -> Result<(), MetricError> {
        let identity = self.metric_identity();
        if identity != contract.identity {
            return Err(MetricError::ObservationIdentityMismatch {
                expected: contract.identity.clone(),
                observed: identity,
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
        identity: &'a MetricObservationIdentity,
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
        identity: &'a MetricObservationIdentity,
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
    pub current: Option<MetricObservation>,
    pub instrumented: bool,
    pub graduation_qualified: bool,
}

impl MetricObservationEvidence {
    fn from_history(
        contract: &MetricContract,
        observations: &[MetricObservation],
    ) -> Result<Self, MetricError> {
        let mut evidence = Self::default();
        let mut current = None;
        for observation in observations {
            if observation.metric_identity() != contract.identity {
                continue;
            }
            // Match through canonical Work, but order by the original evidence digest.
            let order = (observation.source_time(), observation.digest()?);
            if current.is_none_or(|(selected, _)| order > selected) {
                current = Some((order, observation));
            }
            evidence.instrumented |= observation.contract_revision() == contract.contract_revision;
            evidence.graduation_qualified =
                evidence.graduation_qualified || observation.qualifies_graduation(contract);
        }
        evidence.current = current.map(|(_, observation)| observation.clone());
        Ok(evidence)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetricPortfolioDto {
    pub metrics: Vec<MetricReadingDto>,
    pub contract_issues: Vec<MetricContractIssueDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetricReadingDto {
    pub target: Option<Threshold>,
    pub identity: MetricIdentity,
    pub contract_revision: String,
    pub name: String,
    pub description: String,
    pub stage: MetricStage,
    /// Whether any accepted observation measured this exact contract revision.
    pub instrumented: bool,
    pub instrument: String,
    pub unit: String,
    pub window: String,
    pub freshness_policy: String,
    pub freshness: MetricFreshnessDto,
    pub evidence: MetricEvidenceDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricFreshnessDto {
    Never,
    Fresh {
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_time: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        expires_at: OffsetDateTime,
    },
    Stale {
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_time: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        expires_at: OffsetDateTime,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricEvidenceDto {
    Untargeted {
        value: f64,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    Met {
        value: f64,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    Missed {
        value: f64,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    Unknown {
        cause: MetricUnknownCauseDto,
    },
    Unavailable {
        reason: String,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_as_of: OffsetDateTime,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricUnknownCauseDto {
    Never,
    TargetUnavailable {
        value: f64,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    RevisionMismatch {
        expected_contract_revision: String,
        observed_contract_revision: String,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_time: OffsetDateTime,
    },
    Incomplete {
        value: f64,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    WindowMismatch {
        value: f64,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    StaleObservation {
        value: f64,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_start: OffsetDateTime,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_window_end: OffsetDateTime,
    },
    StaleUnavailable {
        reason: String,
        #[schemars(with = "String")]
        #[serde(with = "time::serde::rfc3339")]
        source_as_of: OffsetDateTime,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetricContractIssueDto {
    ProjectUnavailable {
        wave_id: String,
        reason: String,
    },
    UnresolvedTarget {
        wave_id: String,
        metric_id: String,
    },
    MalformedContract {
        path: String,
        message: String,
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
    target: Option<&Threshold>,
    evaluation_time: OffsetDateTime,
) -> Result<MetricReadingDto, MetricError> {
    let persisted = MetricObservationEvidence::from_history(contract, observations)?;
    derive_metric_reading_from_evidence(contract, &persisted, target, evaluation_time)
}

fn derive_metric_reading_from_evidence(
    contract: &MetricContract,
    persisted: &MetricObservationEvidence,
    target: Option<&Threshold>,
    evaluation_time: OffsetDateTime,
) -> Result<MetricReadingDto, MetricError> {
    let (freshness, evidence) = match persisted.current.as_ref() {
        None => (
            MetricFreshnessDto::Never,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::Never,
            },
        ),
        Some(observation) => derive_evidence(contract, observation, target, evaluation_time)?,
    };
    Ok(MetricReadingDto {
        identity: contract.identity.clone(),
        contract_revision: contract.contract_revision.clone(),
        name: contract.name.clone(),
        description: contract.description.clone(),
        stage: contract.stage,
        instrumented: persisted.instrumented,
        instrument: contract.instrument.clone(),
        unit: contract.unit.clone(),
        target: target.cloned(),
        window: contract.window.as_str().to_string(),
        freshness_policy: contract.freshness_policy.as_str().to_string(),
        freshness,
        evidence,
    })
}

pub fn derive_metric_portfolio(
    entries: &[(&MetricContract, &[MetricObservation])],
    targets: &[crate::metric::MetricTarget],
    evaluation_time: OffsetDateTime,
) -> Result<MetricPortfolioDto, MetricError> {
    let mut metrics = Vec::new();
    let mut contract_issues = Vec::new();
    for (contract, observations) in entries {
        let persisted = MetricObservationEvidence::from_history(contract, observations)?;
        let target = targets
            .iter()
            .find(|target| target.metric_id == contract.identity.metric_id)
            .map(|target| &target.target);
        let reading =
            derive_metric_reading_from_evidence(contract, &persisted, target, evaluation_time)?;
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
/// `None` targets means unavailable planning; `Some(&[])` proves no targets.
pub fn compose_metric_portfolio(
    discovery: MetricContractDiscovery,
    registered_instruments: &BTreeMap<MetricIdentity, String>,
    observations: &BTreeMap<MetricIdentity, MetricObservationEvidence>,
    targets: Option<&[crate::metric::MetricTarget]>,
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
        let target = targets.and_then(|targets| {
            targets
                .iter()
                .find(|target| target.metric_id == contract.identity.metric_id)
                .map(|target| &target.target)
        });
        let mut reading =
            derive_metric_reading_from_evidence(&contract, &persisted, target, evaluation_time)?;
        if targets.is_none() {
            if let MetricEvidenceDto::Untargeted {
                value,
                source_window_start,
                source_window_end,
            } = reading.evidence
            {
                reading.evidence = MetricEvidenceDto::Unknown {
                    cause: MetricUnknownCauseDto::TargetUnavailable {
                        value,
                        source_window_start,
                        source_window_end,
                    },
                };
            }
        }
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

fn derive_evidence(
    contract: &MetricContract,
    observation: &MetricObservation,
    target: Option<&Threshold>,
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
            ..
        } if target.is_none() => MetricEvidenceDto::Untargeted {
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
        } if target.is_some_and(|target| target.is_met(*value)) => MetricEvidenceDto::Met {
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

pub fn contract_issue_sort_key(issue: &MetricContractIssueDto) -> (u8, String, String) {
    match issue {
        MetricContractIssueDto::ProjectUnavailable { wave_id, reason } => {
            (5, wave_id.clone(), reason.clone())
        }
        MetricContractIssueDto::UnresolvedTarget { wave_id, metric_id } => {
            (4, wave_id.clone(), metric_id.clone())
        }
        MetricContractIssueDto::MalformedContract { path, .. } => (0, path.clone(), String::new()),
        MetricContractIssueDto::InstrumentMismatch {
            wave_id, metric_id, ..
        } => (2, wave_id.clone(), metric_id.clone()),
        MetricContractIssueDto::InvalidGraduation {
            wave_id, metric_id, ..
        } => (3, wave_id.clone(), metric_id.clone()),
    }
}
