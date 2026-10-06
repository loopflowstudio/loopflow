//! Dependency metadata and exact billed evidence. Provider credentials never enter these records.
use std::collections::BTreeMap;
use std::str::FromStr;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::provider_auth::doppler::DopplerReference;
use crate::store::{StoreError, StoreResult};

macro_rules! record_id {
    ($($name:ident),+) => {$ (
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
    )+};
}
record_id!(
    DependencyId,
    ServiceAccountId,
    ResourceId,
    SourceId,
    CredentialId,
    EnvironmentId,
    RequirementId,
    RuleId
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(#[serde(with = "rust_decimal::serde::str")] pub Decimal);
impl Money {
    pub fn parse(value: &str) -> StoreResult<Self> {
        Decimal::from_str_exact(value)
            .map(Self)
            .map_err(|_| invalid("invalid exact decimal amount"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consumer {
    /// Canonical repository identity, never a checkout basename.
    pub repo: String,
    pub wave_id: Option<crate::id::WaveId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub id: DependencyId,
    pub name: String,
    pub consumers: Vec<Consumer>,
    pub accounts: Vec<ServiceAccountId>,
    pub resources: Vec<ResourceId>,
    pub sources: Vec<SourceId>,
    pub requirements: Vec<RequirementId>,
    pub coverage: Vec<String>,
    pub recurring_estimate: Option<RecurringEstimate>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecurringEstimate {
    pub amount: Money,
    pub currency: String,
    pub cadence: String,
    pub provenance: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceAccount {
    pub id: ServiceAccountId,
    pub provider: String,
    pub native_id: Option<String>,
    pub managed_account: Option<ManagedAccount>,
    pub evidence: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedAccount {
    pub provider: crate::provider_auth::Provider,
    pub account_id: crate::store::ProviderAccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub id: ResourceId,
    pub account: ServiceAccountId,
    pub native_id: String,
    pub project: Option<String>,
    pub tags: BTreeMap<String, String>,
    pub observed_at: i64,
    pub deleted_at: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BillingSource {
    pub id: SourceId,
    pub accounts: Vec<ServiceAccountId>,
    pub credential: Option<CredentialId>,
    pub coverage: Vec<String>,
    /// Stable provider billing authority, shared by API and export transports.
    pub authority: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    pub id: CredentialId,
    pub reference: DopplerReference,
    pub provider_key_id: Option<String>,
    pub version: Option<String>,
    pub purpose: String,
    pub declared_permissions: Vec<String>,
    pub expires_at: Option<i64>,
    pub rotation_evidence: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessEnvironment {
    pub id: EnvironmentId,
    pub name: String,
    pub home_id: Option<crate::durable::HomeId>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessRequirement {
    pub id: RequirementId,
    pub environment: EnvironmentId,
    pub credential: Option<CredentialId>,
    pub tool: Option<String>,
    pub purpose: String,
    pub permissions: Vec<String>,
    pub revision: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub provenance: String,
    pub effective_from: String,
    pub dependencies: Vec<Dependency>,
    pub accounts: Vec<ServiceAccount>,
    pub resources: Vec<Resource>,
    pub sources: Vec<BillingSource>,
    pub credentials: Vec<Credential>,
    pub environments: Vec<AccessEnvironment>,
    pub requirements: Vec<AccessRequirement>,
    pub rules: Vec<AttributionRule>,
    pub discovery_gaps: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AssignmentKind {
    Direct,
    Allocated,
    Shared,
    Unassigned,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub kind: AssignmentKind,
    pub consumer: Option<Consumer>,
    pub dependency: Option<DependencyId>,
    /// Basis points, from 1 through 10000.
    pub weight: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributionRule {
    pub id: RuleId,
    pub revision: String,
    pub account: ServiceAccountId,
    pub resource: Option<ResourceId>,
    pub effective_from: String,
    pub effective_to: String,
    pub reason: String,
    pub assignments: Vec<Assignment>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChargeKind {
    Usage,
    Subscription,
    Tax,
    Credit,
    Adjustment,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Charge {
    pub resource: Option<ResourceId>,
    pub description: String,
    pub kind: ChargeKind,
    pub amount: Money,
    pub start: String,
    pub end: String,
    pub quantity: Option<Money>,
    pub unit: Option<String>,
    pub model: Option<String>,
    pub provider_key_id: Option<String>,
    pub locator: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invoice {
    pub source: SourceId,
    pub account: ServiceAccountId,
    pub document_id: String,
    pub period: String,
    pub currency: String,
    pub total: Money,
    pub fetched_at: i64,
    pub generated_at: Option<i64>,
    pub complete: bool,
    pub charges: Vec<Charge>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributedAmount {
    pub kind: AssignmentKind,
    pub consumer: Option<Consumer>,
    pub dependency: Option<DependencyId>,
    pub amount: Money,
    pub rule: Option<RuleId>,
    pub rule_revision: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatedCharge {
    pub charge: Charge,
    pub attribution: Vec<AttributedAmount>,
    pub gap: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceRevision {
    pub revision: String,
    pub invoice: Invoice,
    pub charges: Vec<EvaluatedCharge>,
    pub billed: Money,
    pub difference: Money,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyTotal {
    pub currency: String,
    pub billed: Money,
    pub direct: Money,
    pub allocated: Money,
    pub shared: Money,
    pub unassigned: Money,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub period: String,
    pub repo: Option<String>,
    pub wave_id: Option<crate::id::WaveId>,
    pub invoices: Vec<InvoiceRevision>,
    pub totals: Vec<CurrencyTotal>,
    pub coverage: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInspection {
    pub dependency: Dependency,
    pub accounts: Vec<ServiceAccount>,
    pub resources: Vec<Resource>,
    pub sources: Vec<BillingSource>,
    pub requirements: Vec<AccessRequirement>,
    pub credentials: Vec<Credential>,
    pub attributed_cost: Option<Vec<CurrencyTotal>>,
    pub linked_invoices: Vec<InvoiceRevision>,
    pub coverage: Vec<String>,
}

pub(crate) fn invalid(message: &str) -> StoreError {
    StoreError::InvalidData(message.to_owned())
}
pub(crate) fn date(value: &str) -> StoreResult<NaiveDate> {
    let parsed = NaiveDate::from_str(value).map_err(|_| invalid("expected an ISO date"))?;
    if parsed.to_string() != value {
        return Err(invalid("expected a canonical YYYY-MM-DD date"));
    }
    Ok(parsed)
}
pub(crate) fn add(a: Decimal, b: Decimal) -> StoreResult<Decimal> {
    a.checked_add(b)
        .ok_or_else(|| invalid("decimal sum exceeds supported precision"))
}
pub(crate) fn evaluate(
    charge: &Charge,
    account: &ServiceAccountId,
    rules: &[AttributionRule],
) -> StoreResult<EvaluatedCharge> {
    let matching: Vec<_> = rules
        .iter()
        .filter(|r| {
            &r.account == account
                && (r.resource.is_none() || r.resource == charge.resource)
                && r.effective_from < charge.end
                && charge.start < r.effective_to
        })
        .collect();
    let mut attribution = Vec::new();
    let mut assigned = Decimal::ZERO;
    let mut gap = None;
    if matching.len() == 1
        && matching[0].effective_from <= charge.start
        && matching[0].effective_to >= charge.end
    {
        let rule = matching[0];
        let total_weight: u32 = rule.assignments.iter().map(|a| a.weight).sum();
        for (index, assignment) in rule.assignments.iter().enumerate() {
            let amount = if total_weight == 10000 && index + 1 == rule.assignments.len() {
                charge
                    .amount
                    .0
                    .checked_sub(assigned)
                    .ok_or_else(|| invalid("decimal overflow"))?
            } else {
                charge
                    .amount
                    .0
                    .checked_mul(Decimal::from(assignment.weight))
                    .and_then(|v| v.checked_div(Decimal::from(10000)))
                    .ok_or_else(|| invalid("decimal allocation overflow"))?
                    .round_dp_with_strategy(
                        charge.amount.0.scale(),
                        rust_decimal::RoundingStrategy::ToZero,
                    )
            };
            assigned = add(assigned, amount)?;
            attribution.push(AttributedAmount {
                kind: assignment.kind.clone(),
                consumer: assignment.consumer.clone(),
                dependency: assignment.dependency.clone(),
                amount: Money(amount),
                rule: Some(rule.id.clone()),
                rule_revision: Some(rule.revision.clone()),
            });
        }
    } else if !matching.is_empty() {
        gap = Some("conflicting mapping or charge crosses an effective boundary".into());
    }
    if assigned != charge.amount.0 || attribution.is_empty() {
        attribution.push(AttributedAmount {
            kind: AssignmentKind::Unassigned,
            consumer: None,
            dependency: None,
            amount: Money(
                charge
                    .amount
                    .0
                    .checked_sub(assigned)
                    .ok_or_else(|| invalid("decimal overflow"))?,
            ),
            rule: None,
            rule_revision: None,
        });
    }
    Ok(EvaluatedCharge {
        charge: charge.clone(),
        attribution,
        gap,
    })
}

pub(crate) fn totals(
    invoices: &[InvoiceRevision],
    repo: Option<&str>,
    wave: Option<&crate::id::WaveId>,
    dependency: Option<&DependencyId>,
) -> StoreResult<Vec<CurrencyTotal>> {
    let mut totals = BTreeMap::new();
    for revision in invoices {
        let amounts: Vec<_> = revision
            .charges
            .iter()
            .flat_map(|c| &c.attribution)
            .filter(|a| {
                !repo.is_some_and(|r| a.consumer.as_ref().is_none_or(|c| c.repo != r))
                    && !wave.is_some_and(|w| {
                        a.consumer
                            .as_ref()
                            .is_none_or(|c| c.wave_id.as_ref() != Some(w))
                    })
                    && !dependency.is_some_and(|d| a.dependency.as_ref() != Some(d))
            })
            .collect();
        if amounts.is_empty() && (repo.is_some() || wave.is_some() || dependency.is_some()) {
            continue;
        }
        let t = totals
            .entry(revision.invoice.currency.clone())
            .or_insert_with(|| CurrencyTotal {
                currency: revision.invoice.currency.clone(),
                billed: Money(Decimal::ZERO),
                direct: Money(Decimal::ZERO),
                allocated: Money(Decimal::ZERO),
                shared: Money(Decimal::ZERO),
                unassigned: Money(Decimal::ZERO),
            });
        for a in amounts {
            t.billed.0 = add(t.billed.0, a.amount.0)?;
            let target = match a.kind {
                AssignmentKind::Direct => &mut t.direct,
                AssignmentKind::Allocated => &mut t.allocated,
                AssignmentKind::Shared => &mut t.shared,
                AssignmentKind::Unassigned => &mut t.unassigned,
            };
            target.0 = add(target.0, a.amount.0)?;
        }
    }
    Ok(totals.into_values().collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccessOutcome {
    Success,
    Denied,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessObservation {
    pub environment: EnvironmentId,
    pub executed_home: crate::durable::HomeId,
    pub requirement: RequirementId,
    pub requirement_revision: String,
    pub credential: Option<CredentialId>,
    pub credential_version: Option<String>,
    pub operation: String,
    pub observed_at: i64,
    pub outcome: AccessOutcome,
    /// A read response never proves read-only enforcement.
    pub scope_evidence: Option<String>,
    pub gap: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessInspection {
    pub environment: AccessEnvironment,
    pub requirements: Vec<AccessRequirement>,
    pub credentials: Vec<Credential>,
    pub observations: Vec<AccessObservation>,
    pub coverage: Vec<String>,
}
