//! Finalized legacy AWS CUR CSV exports. Never polls AWS or resolves credentials.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    add, date, invalid, Charge, ChargeKind, Invoice, Money, Resource, ServiceAccount,
    ServiceAccountId, SourceId,
};
use crate::store::StoreResult;

const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwsCurExport {
    pub account: ServiceAccountId,
    pub invoice_id: String,
    pub currency: String,
    pub invoice_total: Money,
    pub fetched_at: i64,
    pub generated_at: Option<i64>,
    pub manifest: PathBuf,
    /// Root containing reportKeys with their original S3 relative paths.
    pub directory: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    report_keys: Vec<String>,
    billing_period: BillingPeriod,
}
#[derive(Debug, Deserialize)]
struct BillingPeriod {
    start: String,
    end: String,
}

fn read_bounded(reader: impl Read, budget: &mut u64) -> StoreResult<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(*budget + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("AWS CUR export could not be read"))?;
    if bytes.len() as u64 > *budget {
        return Err(invalid("AWS CUR export exceeds 64 MiB decompressed budget"));
    }
    *budget -= bytes.len() as u64;
    Ok(bytes)
}

fn midnight(value: &str) -> StoreResult<String> {
    let at = chrono::DateTime::parse_from_rfc3339(value)
        .map_err(|_| invalid("AWS CUR timestamp is invalid"))?
        .with_timezone(&chrono::Utc);
    if at.time() != chrono::NaiveTime::from_hms_opt(0, 0, 0).expect("midnight") {
        return Err(invalid("AWS CUR reader requires daily or monthly UTC intervals; hourly imports are not implemented"));
    }
    Ok(at.date_naive().to_string())
}

pub(crate) fn read_export(
    export: &AwsCurExport,
    source: SourceId,
    period: &str,
    account: &ServiceAccount,
    resources: &[Resource],
) -> StoreResult<Invoice> {
    if account.provider != "aws" || account.id != export.account {
        return Err(invalid(
            "AWS CUR import requires a declared native AWS account identity",
        ));
    }
    let native_account = account
        .native_id
        .as_deref()
        .ok_or_else(|| invalid("AWS CUR import requires a declared native AWS account identity"))?;
    if export.invoice_id.trim().is_empty() {
        return Err(invalid("AWS CUR invoice identity is required"));
    }
    let mut budget = MAX_BYTES;
    let manifest: Manifest = serde_json::from_slice(&read_bounded(
        File::open(&export.manifest).map_err(|_| invalid("AWS CUR manifest unavailable"))?,
        &mut budget,
    )?)
    .map_err(|_| invalid("invalid AWS CUR manifest"))?;
    let start = midnight(&manifest.billing_period.start)?;
    let end = midnight(&manifest.billing_period.end)?;
    let expected_start = date(&format!("{period}-01"))?;
    if date(&start)? != expected_start
        || Some(date(&end)?) != expected_start.checked_add_months(chrono::Months::new(1))
    {
        return Err(invalid(
            "AWS CUR manifest period differs from requested month",
        ));
    }
    if manifest.report_keys.is_empty() {
        return Err(invalid("AWS CUR manifest contains no report parts"));
    }
    let mut keys = BTreeSet::new();
    let mut charges = Vec::new();
    let mut total = rust_decimal::Decimal::ZERO;
    for key in manifest.report_keys {
        if !keys.insert(key.clone())
            || Path::new(&key)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(invalid(
                "AWS CUR report keys must be distinct relative paths",
            ));
        }
        let file = File::open(export.directory.join(&key))
            .map_err(|_| invalid("AWS CUR manifest part unavailable; prior invoice preserved"))?;
        let bytes = if key.ends_with(".csv.gz") {
            read_bounded(flate2::read::MultiGzDecoder::new(file), &mut budget)?
        } else if key.ends_with(".csv") {
            read_bounded(file, &mut budget)?
        } else {
            return Err(invalid(
                "AWS CUR reader supports CSV and gzip CSV; ZIP and Parquet are not implemented",
            ));
        };
        let mut reader = csv::Reader::from_reader(bytes.as_slice());
        let headers = reader
            .headers()
            .map_err(|_| invalid("invalid AWS CUR CSV header"))?
            .clone();
        let columns: BTreeMap<_, _> = headers
            .iter()
            .enumerate()
            .map(|(i, name)| (name, i))
            .collect();
        if columns.len() != headers.len() {
            return Err(invalid("duplicate AWS CUR columns"));
        }
        for row in reader.records() {
            let row = row.map_err(|_| invalid("invalid AWS CUR CSV row"))?;
            let field = |name: &str| -> StoreResult<&str> {
                columns
                    .get(name)
                    .and_then(|i| row.get(*i))
                    .ok_or_else(|| invalid("required AWS CUR column missing"))
            };
            // Other finalized documents in the same monthly assembly are separate imports.
            if field("bill/InvoiceId")? != export.invoice_id {
                continue;
            }
            if field("bill/PayerAccountId")? != native_account
                || field("lineItem/UsageAccountId")? != native_account
            {
                return Err(invalid("AWS CUR account mismatch; consolidated member-account billing is not implemented"));
            }
            if midnight(field("bill/BillingPeriodStartDate")?)? != start
                || midnight(field("bill/BillingPeriodEndDate")?)? != end
                || field("lineItem/CurrencyCode")? != export.currency
            {
                return Err(invalid("AWS CUR row period or currency mismatch"));
            }
            let amount = Money::parse(field("lineItem/UnblendedCost")?)?;
            // Do not combine net cost with explicit discount rows. Reconcile the selected
            // invoice's complete unblended ledger against the independently supplied bill.
            let kind = match field("lineItem/LineItemType")? {
                "Usage" | "DiscountedUsage" | "SavingsPlanCoveredUsage" => ChargeKind::Usage,
                "Fee"
                | "RIFee"
                | "SavingsPlanUpfrontFee"
                | "SavingsPlanRecurringFee"
                | "FlatRateSubscription" => ChargeKind::Subscription,
                "Tax" => ChargeKind::Tax,
                "Credit" | "Refund" | "Discount" | "BundledDiscount" | "SavingsPlanNegation" => {
                    ChargeKind::Credit
                }
                _ => {
                    return Err(invalid(
                        "AWS CUR charge type not implemented; no invoice published",
                    ))
                }
            };
            if kind == ChargeKind::Credit && amount.0 > rust_decimal::Decimal::ZERO {
                return Err(invalid("AWS CUR credit must be nonpositive"));
            }
            let native_resource = columns
                .get("lineItem/ResourceId")
                .and_then(|i| row.get(*i))
                .filter(|v| !v.is_empty());
            let resource = match native_resource {
                Some(native) => Some(resources.iter().find(|r| r.account == account.id && r.native_id == native)
                    .ok_or_else(|| invalid("AWS CUR resource is absent from inventory; import its identity first"))?.id.clone()),
                None => None,
            };
            let item_id = field("identity/LineItemId")?;
            if item_id.is_empty() {
                return Err(invalid("AWS CUR line identity missing"));
            }
            let charge_start = midnight(field("lineItem/UsageStartDate")?)?;
            let charge_end = midnight(field("lineItem/UsageEndDate")?)?;
            if charge_start >= charge_end {
                return Err(invalid("AWS CUR charge interval is empty"));
            }
            total = add(total, amount.0)?;
            charges.push(Charge {
                resource,
                description: field("lineItem/ProductCode")?.into(),
                kind,
                amount,
                start: charge_start,
                end: charge_end,
                quantity: Some(Money::parse(field("lineItem/UsageAmount")?)?),
                unit: columns
                    .get("pricing/unit")
                    .and_then(|i| row.get(*i))
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
                model: None,
                provider_key_id: None,
                locator: format!("aws-cur/{}/{}", export.invoice_id, item_id),
            });
        }
    }
    if charges.is_empty() || total != export.invoice_total.0 {
        return Err(invalid("AWS CUR invoice missing or does not reconcile to supplied invoice total; prior invoice preserved"));
    }
    Ok(Invoice {
        source,
        account: export.account.clone(),
        document_id: export.invoice_id.clone(),
        period: period.into(),
        currency: export.currency.clone(),
        total: export.invoice_total.clone(),
        fetched_at: export.fetched_at,
        generated_at: export.generated_at,
        complete: true,
        charges,
    })
}
