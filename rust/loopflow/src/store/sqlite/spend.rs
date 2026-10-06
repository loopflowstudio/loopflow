mod rotation;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use super::SqliteStore;
use crate::spend::{
    self, AccessEnvironment, AccessInspection, AccessObservation, AccessOutcome, AccessRequirement,
    AssignmentKind, BillingSource, ChargeKind, Consumer, Credential, Dependency, DependencyId,
    DependencyInspection, EnvironmentId, Inventory, Invoice, InvoiceRevision, Money, Report,
    Resource, ServiceAccount,
};
use crate::store::StoreResult;

fn encode<T: Serialize>(value: &T) -> StoreResult<String> {
    Ok(serde_json::to_string(value)?)
}
fn records<T: DeserializeOwned>(conn: &Connection, table: &str) -> StoreResult<Vec<T>> {
    let mut statement = conn.prepare(&format!("SELECT payload FROM {table} ORDER BY id"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
}
fn record<T: DeserializeOwned>(conn: &Connection, table: &str, id: &str) -> StoreResult<Option<T>> {
    let value: Option<String> = conn
        .query_row(
            &format!("SELECT payload FROM {table} WHERE id=?1"),
            [id],
            |r| r.get(0),
        )
        .optional()?;
    value.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
}
fn upsert_record<T: Serialize>(
    conn: &Connection,
    table: &str,
    id: &str,
    value: &T,
) -> StoreResult<()> {
    if id.trim().is_empty() {
        return Err(spend::invalid("record IDs must be nonempty"));
    }
    conn.execute(&format!("INSERT INTO {table}(id,payload) VALUES (?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload"), params![id, encode(value)?])?;
    Ok(())
}
fn check_consumer(conn: &Connection, consumer: &Consumer) -> StoreResult<()> {
    if consumer.repo.trim().is_empty() {
        return Err(spend::invalid(
            "consumer requires canonical repository identity",
        ));
    }
    if let Some(wave) = &consumer.wave_id {
        let repo: Option<String> = conn
            .query_row("SELECT repo FROM waves WHERE id=?1", [wave.as_str()], |r| {
                r.get(0)
            })
            .optional()?;
        if repo.as_deref() != Some(&consumer.repo) {
            return Err(spend::invalid(
                "consumer repository does not match its registered Wave",
            ));
        }
    }
    Ok(())
}

impl SqliteStore {
    pub(crate) fn import_spend_inventory(&self, inventory: &Inventory) -> StoreResult<()> {
        spend::date(&inventory.effective_from)?;
        for ids in [
            inventory
                .dependencies
                .iter()
                .map(|v| &v.id.0)
                .collect::<Vec<_>>(),
            inventory.accounts.iter().map(|v| &v.id.0).collect(),
            inventory.resources.iter().map(|v| &v.id.0).collect(),
            inventory.sources.iter().map(|v| &v.id.0).collect(),
            inventory.credentials.iter().map(|v| &v.id.0).collect(),
            inventory.environments.iter().map(|v| &v.id.0).collect(),
            inventory.requirements.iter().map(|v| &v.id.0).collect(),
            inventory.rules.iter().map(|v| &v.id.0).collect(),
        ] {
            let mut seen = std::collections::HashSet::new();
            if ids
                .iter()
                .any(|id| id.trim().is_empty() || !seen.insert(*id))
            {
                return Err(spend::invalid("inventory contains empty or duplicate IDs"));
            }
        }

        if inventory.provenance.trim().is_empty() {
            return Err(spend::invalid("inventory provenance is required"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for credential in &inventory.credentials {
            upsert_record(&tx, "spend_credentials", &credential.id.0, credential)?;
        }
        for environment in &inventory.environments {
            upsert_record(&tx, "spend_environments", &environment.id.0, environment)?;
        }
        for account in &inventory.accounts {
            let existing: Option<ServiceAccount> = record(&tx, "spend_accounts", &account.id.0)?;
            if existing.as_ref().is_some_and(|a| {
                a.provider != account.provider
                    || (a.native_id.is_some() && a.native_id != account.native_id)
            }) {
                return Err(spend::invalid("account identity is immutable"));
            }
            if let Some(managed) = &account.managed_account {
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM provider_accounts WHERE provider=?1 AND account_id=?2)",
                    params![managed.provider.as_str(), managed.account_id.as_str()],
                    |r| r.get(0),
                )?;
                if !exists {
                    return Err(spend::invalid("managed account does not exist"));
                }
            }
            tx.execute("INSERT INTO spend_accounts(id,provider,native_id,payload) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET native_id=excluded.native_id,payload=excluded.payload", params![account.id.0, account.provider, account.native_id, encode(account)?])?;
        }
        for resource in &inventory.resources {
            let existing: Option<Resource> = record(&tx, "spend_resources", &resource.id.0)?;
            if existing
                .as_ref()
                .is_some_and(|r| r.account != resource.account || r.native_id != resource.native_id)
            {
                return Err(spend::invalid("resource identity is immutable"));
            }
            tx.execute("INSERT INTO spend_resources(id,account,native_id,payload) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![resource.id.0, resource.account.0, resource.native_id, encode(resource)?])?;
        }
        for source in &inventory.sources {
            let existing: Option<BillingSource> = record(&tx, "spend_sources", &source.id.0)?;
            if existing
                .as_ref()
                .is_some_and(|s| s.authority != source.authority)
            {
                return Err(spend::invalid("billing authority is immutable"));
            }
            tx.execute("INSERT INTO spend_sources(id,authority,credential,payload) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET credential=excluded.credential,payload=excluded.payload", params![source.id.0, source.authority, source.credential.as_ref().map(|c| &c.0), encode(source)?])?;
            // Current scope can shrink without deleting historical invoices.
            tx.execute(
                "DELETE FROM spend_source_accounts WHERE source=?1",
                [&source.id.0],
            )?;
            for account in &source.accounts {
                tx.execute(
                    "INSERT OR IGNORE INTO spend_source_accounts(source,account) VALUES (?1,?2)",
                    params![source.id.0, account.0],
                )?;
            }
        }
        for requirement in &inventory.requirements {
            let existing: Option<AccessRequirement> =
                record(&tx, "spend_requirements", &requirement.id.0)?;
            if existing
                .as_ref()
                .is_some_and(|r| r.revision == requirement.revision && r != requirement)
            {
                return Err(spend::invalid(
                    "changed access requirements need a new revision",
                ));
            }
            if requirement.credential.is_some() == requirement.tool.is_some() {
                return Err(spend::invalid(
                    "requirement must name exactly one credential or tool",
                ));
            }
            tx.execute("INSERT INTO spend_requirements(id,environment,credential,payload) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET environment=excluded.environment,credential=excluded.credential,payload=excluded.payload", params![requirement.id.0, requirement.environment.0, requirement.credential.as_ref().map(|c| &c.0), encode(requirement)?])?;
        }
        for dependency in &inventory.dependencies {
            for consumer in &dependency.consumers {
                check_consumer(&tx, consumer)?;
            }
            upsert_record(&tx, "spend_dependencies", &dependency.id.0, dependency)?;
            for (table, column, ids) in [
                (
                    "spend_dependency_accounts",
                    "account",
                    dependency.accounts.iter().map(|x| &x.0).collect::<Vec<_>>(),
                ),
                (
                    "spend_dependency_resources",
                    "resource",
                    dependency.resources.iter().map(|x| &x.0).collect(),
                ),
                (
                    "spend_dependency_sources",
                    "source",
                    dependency.sources.iter().map(|x| &x.0).collect(),
                ),
                (
                    "spend_dependency_requirements",
                    "requirement",
                    dependency.requirements.iter().map(|x| &x.0).collect(),
                ),
            ] {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE dependency=?1"),
                    [&dependency.id.0],
                )?;
                for id in ids {
                    tx.execute(
                        &format!("INSERT INTO {table}(dependency,{column}) VALUES (?1,?2)"),
                        params![dependency.id.0, id],
                    )?;
                }
            }
        }
        for rule in &inventory.rules {
            if spend::date(&rule.effective_from)? >= spend::date(&rule.effective_to)?
                || rule.reason.trim().is_empty()
                || rule.assignments.is_empty()
            {
                return Err(spend::invalid(
                    "rule requires an interval, reason and assignments",
                ));
            }
            let mut weight = 0u32;
            for assignment in &rule.assignments {
                weight = weight
                    .checked_add(assignment.weight)
                    .ok_or_else(|| spend::invalid("allocation weight overflow"))?;
                if assignment.weight == 0 || weight > 10000 {
                    return Err(spend::invalid(
                        "allocation weights must total at most 10000 basis points",
                    ));
                }
                match assignment.kind {
                    AssignmentKind::Direct
                        if rule.assignments.len() != 1 || assignment.weight != 10000 =>
                    {
                        return Err(spend::invalid(
                            "direct attribution requires one complete assignment",
                        ))
                    }
                    AssignmentKind::Shared | AssignmentKind::Unassigned
                        if assignment.consumer.is_some() || assignment.dependency.is_some() =>
                    {
                        return Err(spend::invalid(
                            "shared/unassigned amounts cannot have an owner",
                        ))
                    }
                    _ => {}
                }
                if matches!(
                    assignment.kind,
                    AssignmentKind::Direct | AssignmentKind::Allocated
                ) && assignment.consumer.is_none()
                    && assignment.dependency.is_none()
                {
                    return Err(spend::invalid("attributed amounts require an owner"));
                }
                if let Some(c) = &assignment.consumer {
                    check_consumer(&tx, c)?;
                }
                if let Some(d) = &assignment.dependency {
                    if record::<Dependency>(&tx, "spend_dependencies", &d.0)?.is_none() {
                        return Err(spend::invalid("unknown dependency in rule"));
                    }
                }
            }
            if let Some(resource) = &rule.resource {
                let resource: Resource = record(&tx, "spend_resources", &resource.0)?
                    .ok_or_else(|| spend::invalid("unknown rule resource"))?;
                if resource.account != rule.account {
                    return Err(spend::invalid(
                        "rule resource belongs to a different account",
                    ));
                }
            }
            let payload = encode(rule)?;
            let old: Option<String> = tx
                .query_row(
                    "SELECT payload FROM spend_rule_history WHERE id=?1 AND revision=?2",
                    params![rule.id.0, rule.revision],
                    |r| r.get(0),
                )
                .optional()?;
            if old.as_ref().is_some_and(|s| s != &payload) {
                return Err(spend::invalid(
                    "rule revisions are immutable; supply a new revision",
                ));
            }
            tx.execute(
                "INSERT OR IGNORE INTO spend_rule_history(id,revision,payload) VALUES (?1,?2,?3)",
                params![rule.id.0, rule.revision, payload],
            )?;
            tx.execute("INSERT INTO spend_rules(id,revision,account,resource,payload) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,account=excluded.account,resource=excluded.resource,payload=excluded.payload", params![rule.id.0, rule.revision, rule.account.0, rule.resource.as_ref().map(|r| &r.0), payload])?;
        }
        tx.execute("INSERT INTO spend_inventory_history(effective_from,provenance,payload) VALUES (?1,?2,?3)", params![inventory.effective_from, inventory.provenance, encode(inventory)?])?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn import_spend_invoice(&self, invoice: &Invoice) -> StoreResult<InvoiceRevision> {
        if !invoice.complete {
            return Err(spend::invalid(
                "incomplete billing evidence cannot replace an invoice",
            ));
        }
        validate_invoice(invoice)?;
        let mut normalized = invoice.clone();
        let mut items = normalized
            .charges
            .into_iter()
            .map(|charge| Ok((encode(&charge)?, charge)))
            .collect::<StoreResult<Vec<_>>>()?;
        items.sort_by(|a, b| a.0.cmp(&b.0));
        normalized.charges = items.into_iter().map(|(_, charge)| charge).collect();
        let revision = hex::encode(Sha256::digest(
            encode(&(
                &normalized.account,
                &normalized.document_id,
                &normalized.period,
                &normalized.currency,
                &normalized.total,
                &normalized.charges,
            ))?
            .as_bytes(),
        ));
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx.query_row("SELECT payload FROM spend_invoice_revisions WHERE source=?1 AND account=?2 AND document_id=?3 AND revision=?4", params![invoice.source.0, invoice.account.0, invoice.document_id, revision], |r| r.get(0)).optional()?;
        if let Some(existing) = existing {
            return Ok(serde_json::from_str(&existing)?);
        }
        for charge in &normalized.charges {
            if let Some(resource) = &charge.resource {
                let resource: Resource = record(&tx, "spend_resources", &resource.0)?
                    .ok_or_else(|| spend::invalid("unknown invoice resource"))?;
                if resource.account != invoice.account {
                    return Err(spend::invalid(
                        "invoice resource belongs to another account",
                    ));
                }
            }
        }
        let source: BillingSource = record(&tx, "spend_sources", &invoice.source.0)?
            .ok_or_else(|| spend::invalid("billing source not found"))?;
        if !source.accounts.contains(&invoice.account) {
            return Err(spend::invalid(
                "invoice account is outside the billing source scope",
            ));
        }
        let rules = records(&tx, "spend_rules")?;
        let mut statement = tx.prepare(
            "SELECT payload FROM spend_inventory_history ORDER BY effective_from, revision",
        )?;
        let mut observations = std::collections::BTreeMap::new();
        for row in statement.query_map([], |row| row.get::<_, String>(0))? {
            let inventory: Inventory = serde_json::from_str(&row?)?;
            let effective = spend::date(&inventory.effective_from)?
                .and_hms_opt(0, 0, 0)
                .expect("midnight")
                .and_utc()
                .timestamp();
            for resource in inventory.resources {
                let at = effective.max(resource.observed_at);
                observations.insert((resource.id.clone(), at), resource);
            }
        }
        drop(statement);
        let observations: Vec<_> = observations
            .into_iter()
            .map(|((_, at), resource)| (at, resource))
            .collect();
        let charges = normalized
            .charges
            .iter()
            .map(|c| spend::evaluate(c, &invoice.account, &rules, &observations))
            .collect::<StoreResult<Vec<_>>>()?;
        let billed = charges
            .iter()
            .try_fold(Decimal::ZERO, |sum, c| spend::add(sum, c.charge.amount.0))?;
        let difference = billed
            .checked_sub(invoice.total.0)
            .ok_or_else(|| spend::invalid("decimal overflow"))?;
        let result = InvoiceRevision {
            revision: revision.clone(),
            invoice: normalized,
            charges,
            billed: Money(billed),
            difference: Money(difference),
        };
        tx.execute("INSERT INTO spend_invoices(source,account,document_id,current_revision) VALUES (?1,?2,?3,?4) ON CONFLICT(source,account,document_id) DO UPDATE SET current_revision=excluded.current_revision", params![invoice.source.0, invoice.account.0, invoice.document_id, revision])?;
        tx.execute("INSERT INTO spend_invoice_revisions(source,account,document_id,revision,period,payload) VALUES (?1,?2,?3,?4,?5,?6)", params![invoice.source.0, invoice.account.0, invoice.document_id, revision, invoice.period, encode(&result)?])?;
        tx.commit()?;
        Ok(result)
    }

    pub(crate) fn spend_report(
        &self,
        period: &str,
        repo: Option<&str>,
        wave: Option<&crate::id::WaveId>,
    ) -> StoreResult<Report> {
        let start = spend::date(&format!("{period}-01"))?;
        let end = start
            .checked_add_months(chrono::Months::new(1))
            .ok_or_else(|| spend::invalid("period overflow"))?;
        let start = start
            .and_hms_opt(0, 0, 0)
            .expect("midnight")
            .and_utc()
            .timestamp();
        let end = end
            .and_hms_opt(0, 0, 0)
            .expect("midnight")
            .and_utc()
            .timestamp();
        let session_usage = self
            .conversation_history(wave.map(|w| w.as_str()), None, None, None, start, true)?
            .into_iter()
            .filter(|history| {
                history.captured.is_some()
                    && history.observed_at >= start
                    && history.observed_at < end
                    && repo.is_none_or(|repo| history.repo.as_deref() == Some(repo))
            })
            .map(|history| spend::SessionUsageLink {
                session_id: history.session_id,
                artifact_key: history.artifact_key,
                captured: history.captured.expect("filtered captured inputs"),
                observed_at: history.observed_at,
                repo: history.repo,
                wave_id: history.wave_id,
                task_id: history.task_id,
                usage: history.usage,
                evidence_gaps: history.evidence_gaps,
            })
            .collect();
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare("SELECT r.payload FROM spend_invoices i JOIN spend_invoice_revisions r ON r.source=i.source AND r.account=i.account AND r.document_id=i.document_id AND r.revision=i.current_revision WHERE r.period=?1 ORDER BY r.source,r.account,r.document_id")?;
        let invoices: Vec<InvoiceRevision> = stmt
            .query_map([period], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect::<StoreResult<_>>()?;
        let mut coverage = Vec::new();
        for source in records::<BillingSource>(&conn, "spend_sources")? {
            coverage.extend(source.coverage);
            for account in &source.accounts {
                if !invoices
                    .iter()
                    .any(|i| i.invoice.source == source.id && &i.invoice.account == account)
                {
                    coverage.push(format!(
                        "missing period {period}: source {} account {}",
                        source.id.0, account.0
                    ));
                }
            }
        }
        let gaps: Option<String> = conn
            .query_row(
                "SELECT payload FROM spend_inventory_history ORDER BY revision DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(gaps) = gaps {
            coverage.extend(serde_json::from_str::<Inventory>(&gaps)?.discovery_gaps);
        }
        if invoices.is_empty() {
            coverage.push("no billed evidence for this period; cost is unknown".into());
        }
        for invoice in &invoices {
            if invoice.difference.0 != Decimal::ZERO {
                coverage.push(format!(
                    "invoice {} does not reconcile",
                    invoice.invoice.document_id
                ));
            }
            coverage.extend(invoice.charges.iter().filter_map(|c| c.gap.clone()));
        }
        let totals = spend::totals(&invoices, repo, wave, None)?;
        if totals.is_empty() && (repo.is_some() || wave.is_some()) {
            coverage.push("attributed cost unknown for the requested repository/Wave".into());
        }
        coverage.push("Session usage covers inputs captured in this period, not apportioned monthly usage; estimates are separate from bills and provider-key linkage is unknown".into());
        Ok(Report {
            session_usage,
            period: period.into(),
            repo: repo.map(str::to_owned),
            wave_id: wave.cloned(),
            invoices,
            totals,
            coverage,
        })
    }

    pub(crate) fn spend_dependency(
        &self,
        id: &DependencyId,
        period: &str,
    ) -> StoreResult<Option<DependencyInspection>> {
        let report = self.spend_report(period, None, None)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(dependency) = record::<Dependency>(&conn, "spend_dependencies", &id.0)? else {
            return Ok(None);
        };
        let accounts = records::<ServiceAccount>(&conn, "spend_accounts")?
            .into_iter()
            .filter(|a| dependency.accounts.contains(&a.id))
            .collect();
        let resources = records::<Resource>(&conn, "spend_resources")?
            .into_iter()
            .filter(|r| dependency.resources.contains(&r.id))
            .collect();
        let sources = records::<BillingSource>(&conn, "spend_sources")?
            .into_iter()
            .filter(|s| dependency.sources.contains(&s.id))
            .collect();
        let environments = records::<AccessEnvironment>(&conn, "spend_environments")?;
        let mut access = Vec::new();
        for environment in environments {
            let inspection = read_access(&conn, environment, Some(&dependency.requirements))?;
            if !inspection.requirements.is_empty() {
                access.push(inspection);
            }
        }
        let totals = spend::totals(&report.invoices, None, None, Some(id))?;
        let attributed_cost = (!totals.is_empty()).then_some(totals);
        let linked_invoices = report
            .invoices
            .into_iter()
            .filter(|i| {
                dependency.accounts.contains(&i.invoice.account)
                    || dependency.sources.contains(&i.invoice.source)
            })
            .collect();
        let mut coverage = dependency.coverage.clone();
        coverage.extend(report.coverage);
        if attributed_cost.is_none() {
            coverage.push("attributed cost unknown; account links confer no allocation".into());
        }
        Ok(Some(DependencyInspection {
            dependency,
            accounts,
            resources,
            sources,
            access,
            attributed_cost,
            linked_invoices,
            coverage,
        }))
    }
}
fn validate_invoice(invoice: &Invoice) -> StoreResult<()> {
    if invoice.document_id.trim().is_empty()
        || invoice.currency.len() != 3
        || !invoice.currency.bytes().all(|c| c.is_ascii_uppercase())
    {
        return Err(spend::invalid(
            "invoice requires document identity and ISO currency",
        ));
    }
    spend::date(&format!("{}-01", invoice.period))?;
    for charge in &invoice.charges {
        if spend::date(&charge.start)? >= spend::date(&charge.end)?
            || charge.locator.trim().is_empty()
        {
            return Err(spend::invalid(
                "charge requires source locator and a nonempty interval",
            ));
        }
        if matches!(charge.kind, ChargeKind::Credit) && charge.amount.0 > Decimal::ZERO {
            return Err(spend::invalid("normalized credits must be nonpositive"));
        }
    }
    Ok(())
}

impl SqliteStore {
    pub(crate) fn spend_access(
        &self,
        environment: &EnvironmentId,
    ) -> StoreResult<Option<AccessInspection>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(environment) =
            record::<AccessEnvironment>(&conn, "spend_environments", &environment.0)?
        else {
            return Ok(None);
        };
        Ok(Some(read_access(&conn, environment, None)?))
    }
    pub(crate) fn record_spend_access(
        &self,
        observations: &[AccessObservation],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for observation in observations {
            let requirement: AccessRequirement =
                record(&tx, "spend_requirements", &observation.requirement.0)?
                    .ok_or_else(|| spend::invalid("access requirement not found"))?;
            if requirement.environment != observation.environment
                || requirement.revision != observation.requirement_revision
                || requirement.credential != observation.credential
            {
                return Err(spend::invalid(
                    "access requirement changed during verification",
                ));
            }
            if let Some(credential) = &observation.credential {
                let credential: Credential = record(&tx, "spend_credentials", &credential.0)?
                    .ok_or_else(|| spend::invalid("credential not found"))?;
                if credential.version != observation.credential_version
                    || Some(&credential.reference) != observation.credential_reference.as_ref()
                {
                    return Err(spend::invalid("credential changed during verification"));
                }
            }
            tx.execute("INSERT INTO spend_access_observations(environment,requirement,observed_at,payload) VALUES (?1,?2,?3,?4)", params![observation.environment.0, observation.requirement.0, observation.observed_at, encode(observation)?])?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn read_access(
    conn: &Connection,
    environment: AccessEnvironment,
    selected: Option<&[crate::spend::RequirementId]>,
) -> StoreResult<AccessInspection> {
    let requirements: Vec<AccessRequirement> =
        records::<AccessRequirement>(conn, "spend_requirements")?
            .into_iter()
            .filter(|r| {
                r.environment == environment.id && selected.is_none_or(|ids| ids.contains(&r.id))
            })
            .collect();
    let credentials: Vec<Credential> = records::<Credential>(conn, "spend_credentials")?
        .into_iter()
        .filter(|c| {
            requirements
                .iter()
                .any(|r| r.credential.as_ref() == Some(&c.id))
        })
        .collect();
    let mut statement = conn.prepare(
            "SELECT payload FROM spend_access_observations WHERE environment=?1 ORDER BY observed_at DESC, id DESC",
        )?;
    let observations: Vec<AccessObservation> = statement
        .query_map([&environment.id.0], |r| r.get::<_, String>(0))?
        .map(|r| Ok(serde_json::from_str(&r?)?))
        .collect::<StoreResult<_>>()?;
    let observations = observations
        .into_iter()
        .filter(|o| requirements.iter().any(|r| r.id == o.requirement))
        .collect();
    let mut inspection = AccessInspection {
        environment,
        requirements,
        credentials,
        observations,
        coverage: Vec::new(),
    };
    let mut coverage = Vec::new();
    for requirement in &inspection.requirements {
        let credential = inspection
            .credentials
            .iter()
            .find(|c| requirement.credential.as_ref() == Some(&c.id));
        let current = inspection.current_observation(requirement);
        if !current.is_some_and(|o| o.outcome == AccessOutcome::Success) {
            coverage.push(format!("{}: current access unverified", requirement.id.0));
        }
        if credential.is_some_and(|c| c.version.is_none()) {
            coverage.push(format!("{}: credential version unknown", requirement.id.0));
        }
    }
    coverage.push("declared permissions are not provider-enforced scope evidence".into());
    inspection.coverage = coverage;
    Ok(inspection)
}

impl SqliteStore {
    pub(crate) fn spend_dependency_history(
        &self,
        id: &DependencyId,
    ) -> StoreResult<Vec<crate::spend::DependencyHistory>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT payload FROM spend_inventory_history ORDER BY effective_from, revision",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut versions = std::collections::BTreeMap::new();
        let mut dependency = None;
        let mut accounts = std::collections::BTreeMap::new();
        let mut resources = std::collections::BTreeMap::new();
        let mut sources = std::collections::BTreeMap::new();
        let mut requirements = std::collections::BTreeMap::new();
        let mut credentials = std::collections::BTreeMap::new();
        let mut environments = std::collections::BTreeMap::new();
        for row in rows {
            let inventory: Inventory = serde_json::from_str(&row?)?;
            if let Some(updated) = inventory.dependencies.into_iter().find(|d| &d.id == id) {
                dependency = Some(updated);
            }
            accounts.extend(inventory.accounts.into_iter().map(|v| (v.id.clone(), v)));
            resources.extend(inventory.resources.into_iter().map(|v| (v.id.clone(), v)));
            sources.extend(inventory.sources.into_iter().map(|v| (v.id.clone(), v)));
            requirements.extend(
                inventory
                    .requirements
                    .into_iter()
                    .map(|v| (v.id.clone(), v)),
            );
            credentials.extend(inventory.credentials.into_iter().map(|v| (v.id.clone(), v)));
            environments.extend(
                inventory
                    .environments
                    .into_iter()
                    .map(|v| (v.id.clone(), v)),
            );
            let Some(dependency) = dependency.as_ref() else {
                continue;
            };
            let selected: Vec<_> = requirements
                .values()
                .filter(|r| dependency.requirements.contains(&r.id))
                .cloned()
                .collect();
            let version = crate::spend::DependencyHistory {
                dependency: dependency.clone(),
                accounts: accounts
                    .values()
                    .filter(|a| dependency.accounts.contains(&a.id))
                    .cloned()
                    .collect(),
                resources: resources
                    .values()
                    .filter(|r| dependency.resources.contains(&r.id))
                    .cloned()
                    .collect(),
                sources: sources
                    .values()
                    .filter(|s| dependency.sources.contains(&s.id))
                    .cloned()
                    .collect(),
                credentials: credentials
                    .values()
                    .filter(|c| {
                        selected
                            .iter()
                            .any(|r| r.credential.as_ref() == Some(&c.id))
                    })
                    .cloned()
                    .collect(),
                environments: environments
                    .values()
                    .filter(|e| selected.iter().any(|r| r.environment == e.id))
                    .cloned()
                    .collect(),
                requirements: selected,
                effective_from: inventory.effective_from.clone(),
                effective_to: None,
                provenance: inventory.provenance,
            };
            versions.insert(inventory.effective_from, version);
        }
        let mut history: Vec<_> = versions.into_values().collect();
        let mut previous = None;
        history.retain(|version| {
            let projection = serde_json::to_value((
                &version.dependency,
                &version.accounts,
                &version.resources,
                &version.sources,
                &version.requirements,
                &version.credentials,
                &version.environments,
            ))
            .expect("inventory records serialize");
            let changed = previous.as_ref() != Some(&projection);
            previous = Some(projection);
            changed
        });
        for index in 1..history.len() {
            history[index - 1].effective_to = Some(history[index].effective_from.clone());
        }
        Ok(history)
    }
}

impl SqliteStore {
    pub(crate) fn import_spend_aws_cur(
        &self,
        export: &crate::spend::aws_cur::AwsCurExport,
        source: crate::spend::SourceId,
        period: &str,
    ) -> StoreResult<InvoiceRevision> {
        let (account, resources) = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let account: ServiceAccount = record(&conn, "spend_accounts", &export.account.0)?
                .ok_or_else(|| spend::invalid("AWS billing account not found"))?;
            (account, records::<Resource>(&conn, "spend_resources")?)
        };
        let invoice = spend::aws_cur::read_export(export, source, period, &account, &resources)?;
        self.import_spend_invoice(&invoice)
    }
}

#[cfg(test)]
mod tests {
    use crate::store::migrations::{apply_before_current_draft, current_draft_sql};

    #[test]
    fn spend_links_retained_session_usage_without_turning_it_into_bills() {
        let home = tempfile::tempdir().unwrap();
        let store =
            crate::store::sqlite::SqliteStore::open_ephemeral(&home.path().join("store.db"))
                .unwrap();
        let mut session = store.test_session("seed", "run_00000000000000000000000000000001");
        session.id = "september".into();
        session.artifact_key = "run_00000000000000000000000000000002".into();
        session.captured = None;
        session.created_at = 1788220800;
        session.repo = Some("example/one".into());
        let session = store.create_session(session, None, None).unwrap();
        let source = "events.jsonl:1";
        let evidence = serde_json::json!({
            "schema_version":1,"seq":1,"observed_at":"2026-09-29T00:00:00Z",
            "type":"usage","provider":"codex","model":null,"attempt_key":"attempt","turn_key":"turn",
            "usage_stream_id":"recorder","observation_seq":1,"counter_kind":"cumulative","start_known":true,
            "final_receipt":true,"usage":{"input_tokens":18,"output_tokens":5,"cost_usd":0.2}
        });
        store.retain_session_observation(&session, &crate::session::SessionObservation {
            artifact_key: session.artifact_key.clone(), source: source.into(), observed_at: 1790640000,
            task_id: None, wave_id: None,
            payload: serde_json::json!({"input_id":session.artifact_key,"source":source,"evidence":evidence}),
        }).unwrap();
        let history = store.input_history(&session.artifact_key).unwrap();
        let report = store
            .spend_report("2026-09", Some("example/one"), None)
            .unwrap();
        assert_eq!(report.session_usage.len(), 1);
        assert_eq!(report.session_usage[0].usage, history.usage);
        assert_eq!(report.session_usage[0].usage.cost_usd, Some(0.2));
        assert_eq!(
            report.session_usage[0].artifact_key.as_ref(),
            Some(&session.artifact_key)
        );
        assert!(report.totals.is_empty());
        assert!(report.invoices.is_empty());
        assert!(store
            .spend_report("2026-10", None, None)
            .unwrap()
            .session_usage
            .is_empty());
        assert!(store
            .spend_report("2026-09", Some("example/two"), None)
            .unwrap()
            .session_usage
            .is_empty());
    }

    #[test]
    fn dependency_schema_upgrades_released_frontier_without_changing_wave_identity() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        apply_before_current_draft(&conn, "computing_dependencies");
        conn.execute("INSERT INTO waves(id,name,repo,created_at) VALUES('preserved','spend','example/one',1)", []).unwrap();
        conn.execute_batch(&current_draft_sql("computing_dependencies"))
            .unwrap();
        let repo: String = conn
            .query_row("SELECT repo FROM waves WHERE id='preserved'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(repo, "example/one");
        conn.execute(
            "INSERT INTO spend_credentials VALUES('old','{}'),('candidate','{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO spend_rotations VALUES('first','old','candidate','candidate','{}')",
            [],
        )
        .unwrap();
        assert!(conn.execute("INSERT INTO spend_rotations VALUES('competing','old','candidate','candidate','{}')", []).is_err());
        conn.execute(
            "UPDATE spend_rotations SET state='cancelled' WHERE id='first'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO spend_rotations VALUES('retry','old','candidate','candidate','{}')",
            [],
        )
        .unwrap();
        assert!(conn
            .execute(
                "INSERT INTO spend_dependency_accounts VALUES('missing','missing')",
                []
            )
            .is_err());
    }
}
