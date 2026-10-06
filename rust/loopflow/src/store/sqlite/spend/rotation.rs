use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use super::{encode, record, records, upsert_record};
use crate::spend::{self, AccessEnvironment, AccessRequirement, Credential, Inventory};
use crate::store::{sqlite::SqliteStore, StoreResult};

impl SqliteStore {
    pub(crate) fn begin_spend_rotation(
        &self,
        old: &crate::spend::CredentialId,
        candidate: &crate::spend::CredentialId,
        consumer_inventory_evidence: Option<String>,
    ) -> StoreResult<crate::spend::rotation::Rotation> {
        use crate::spend::rotation::{Rotation, RotationConsumer, RotationId, RotationState};
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pending: Option<String> = tx
            .query_row(
                "SELECT payload FROM spend_rotations WHERE old_credential=?1 AND state NOT IN ('retired', 'cancelled')",
                [&old.0],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(payload) = pending {
            let mut rotation: Rotation = serde_json::from_str(&payload)?;
            if rotation.candidate.id != *candidate {
                return Err(spend::invalid("another candidate is already pending"));
            }
            if let Some(evidence) = consumer_inventory_evidence.filter(|e| !e.trim().is_empty()) {
                validate_rotation_inventory(&tx, &rotation)?;
                rotation.consumer_inventory_evidence = Some(evidence);
                write_rotation(&tx, &rotation)?;
                tx.commit()?;
            }
            return Ok(rotation);
        }
        let old: Credential = record(&tx, "spend_credentials", &old.0)?
            .ok_or_else(|| spend::invalid("credential not found"))?;
        let candidate: Credential = record(&tx, "spend_credentials", &candidate.0)?
            .ok_or_else(|| spend::invalid("candidate not found"))?;
        if old.id == candidate.id
            || old.reference == candidate.reference
            || candidate
                .version
                .as_ref()
                .is_none_or(|v| v.trim().is_empty())
        {
            return Err(spend::invalid("rotation requires a distinct replacement reference and trustworthy candidate version"));
        }
        let requirements: Vec<AccessRequirement> =
            records::<AccessRequirement>(&tx, "spend_requirements")?
                .into_iter()
                .filter(|r| {
                    r.credential.as_ref() == Some(&old.id)
                        || r.credential.as_ref() == Some(&candidate.id)
                })
                .collect();
        let consumers = records::<AccessEnvironment>(&tx, "spend_environments")?
            .into_iter()
            .filter(|e| requirements.iter().any(|r| r.environment == e.id))
            .map(|e| RotationConsumer {
                environment: e.id,
                home_id: e.home_id,
            })
            .collect();
        let rotation = Rotation {
            inventory_revision: 1,
            id: RotationId(uuid::Uuid::new_v4().to_string()),
            old,
            candidate,
            requirements,
            consumers,
            consumer_inventory_evidence: consumer_inventory_evidence
                .filter(|e| !e.trim().is_empty()),
            state: RotationState::Candidate,
            receipts: Vec::new(),
            activated_at: None,
        };
        tx.execute("INSERT INTO spend_rotations(id,old_credential,candidate_credential,state,payload) VALUES (?1,?2,?3,'candidate',?4)",
            params![rotation.id.0, rotation.old.id.0, rotation.candidate.id.0, encode(&rotation)?])?;
        tx.commit()?;
        Ok(rotation)
    }

    pub(crate) fn spend_rotation(
        &self,
        id: &crate::spend::rotation::RotationId,
    ) -> StoreResult<Option<crate::spend::rotation::Rotation>> {
        record(
            &self.conn.lock().expect("store mutex poisoned"),
            "spend_rotations",
            &id.0,
        )
    }

    pub(crate) fn record_spend_rotation(
        &self,
        id: &crate::spend::rotation::RotationId,
        receipt: crate::spend::rotation::RotationReceipt,
    ) -> StoreResult<crate::spend::rotation::Rotation> {
        use crate::spend::rotation::{Rotation, RotationOperation, RotationState};
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut rotation: Rotation = record(&tx, "spend_rotations", &id.0)?
            .ok_or_else(|| spend::invalid("rotation not found"))?;
        if matches!(
            rotation.state,
            RotationState::Retired | RotationState::Cancelled
        ) {
            return Err(spend::invalid("rotation already closed"));
        }
        validate_rotation_inventory(&tx, &rotation)?;
        if receipt.inventory_revision != rotation.inventory_revision {
            return Err(spend::invalid(
                "receipt belongs to an obsolete rotation inventory revision",
            ));
        }
        if Some(&receipt.candidate_version) != rotation.candidate.version.as_ref()
            || receipt.evidence.trim().is_empty()
        {
            return Err(spend::invalid(
                "receipt requires candidate version and non-secret evidence",
            ));
        }
        match receipt.operation {
            RotationOperation::CandidateRead | RotationOperation::ConsumerCutover => {
                if (receipt.operation == RotationOperation::CandidateRead)
                    != (rotation.state == RotationState::Candidate)
                {
                    return Err(spend::invalid(
                        "receipt operation does not match rotation phase",
                    ));
                }
                if receipt.operation == RotationOperation::ConsumerCutover
                    && rotation
                        .activated_at
                        .is_none_or(|at| receipt.observed_at < at)
                {
                    return Err(spend::invalid("cutover receipt predates activation"));
                }
                if !rotation.consumers.iter().any(|c| {
                    Some(&c.environment) == receipt.environment.as_ref()
                        && c.home_id.is_some()
                        && c.home_id == receipt.executed_home
                }) {
                    return Err(spend::invalid(
                        "receipt must identify a required environment and its executing Home",
                    ));
                }
            }
            RotationOperation::Retirement => {
                if rotation.state != RotationState::RetirementPending
                    || rotation.consumer_inventory_evidence.is_none()
                    || !rotation.consumers_ready(RotationOperation::ConsumerCutover)
                    || receipt.environment.is_some()
                    || receipt.executed_home.is_some()
                {
                    return Err(spend::invalid("retirement requires all consumer cutovers and complete consumer inventory evidence"));
                }
                if receipt.success {
                    rotation.state = RotationState::Retired;
                }
            }
        }
        rotation.receipts.push(receipt);
        write_rotation(&tx, &rotation)?;
        tx.commit()?;
        Ok(rotation)
    }

    pub(crate) fn activate_spend_rotation(
        &self,
        id: &crate::spend::rotation::RotationId,
    ) -> StoreResult<crate::spend::rotation::Rotation> {
        use crate::spend::rotation::{Rotation, RotationOperation, RotationState};
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut rotation: Rotation = record(&tx, "spend_rotations", &id.0)?
            .ok_or_else(|| spend::invalid("rotation not found"))?;
        if rotation.state != RotationState::Candidate {
            return Ok(rotation);
        }
        validate_rotation_inventory(&tx, &rotation)?;
        if !rotation.consumers_ready(RotationOperation::CandidateRead) {
            return Err(spend::invalid(
                "candidate needs a successful read receipt from every required environment",
            ));
        }
        let mut active = rotation.candidate.clone();
        active.id = rotation.old.id.clone();
        upsert_record(&tx, "spend_credentials", &active.id.0, &active)?;
        // Record the same mutation in the inventory's historical authority.
        let prior: String = tx.query_row(
            "SELECT payload FROM spend_inventory_history ORDER BY revision DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?;
        let discovery_gaps = serde_json::from_str::<Inventory>(&prior)?.discovery_gaps;
        let inventory = Inventory {
            provenance: format!(
                "rotation {} activation; consumer cutover and retirement pending",
                id.0
            ),
            effective_from: time::OffsetDateTime::now_utc().date().to_string(),
            credentials: vec![active],
            dependencies: Vec::new(),
            accounts: Vec::new(),
            resources: Vec::new(),
            sources: Vec::new(),
            environments: Vec::new(),
            requirements: Vec::new(),
            rules: Vec::new(),
            discovery_gaps,
        };
        tx.execute("INSERT INTO spend_inventory_history(effective_from,provenance,payload) VALUES (?1,?2,?3)",
            params![inventory.effective_from, inventory.provenance, encode(&inventory)?])?;
        rotation.state = RotationState::RetirementPending;
        rotation.activated_at = Some(time::OffsetDateTime::now_utc().unix_timestamp());
        write_rotation(&tx, &rotation)?;
        tx.commit()?;
        Ok(rotation)
    }
}

fn write_rotation(
    conn: &Connection,
    rotation: &crate::spend::rotation::Rotation,
) -> StoreResult<()> {
    use crate::spend::rotation::RotationState;
    let state = match rotation.state {
        RotationState::Candidate => "candidate",
        RotationState::RetirementPending => "retirement_pending",
        RotationState::Retired => "retired",
        RotationState::Cancelled => "cancelled",
    };
    conn.execute(
        "UPDATE spend_rotations SET state=?1,payload=?2 WHERE id=?3",
        params![state, encode(rotation)?, rotation.id.0],
    )?;
    Ok(())
}

fn validate_rotation_inventory(
    conn: &Connection,
    rotation: &crate::spend::rotation::Rotation,
) -> StoreResult<()> {
    use crate::spend::rotation::RotationState;
    let mut expected_active = if rotation.state == RotationState::Candidate {
        rotation.old.clone()
    } else {
        rotation.candidate.clone()
    };
    expected_active.id = rotation.old.id.clone();
    for expected in [&expected_active, &rotation.candidate] {
        let actual: Credential = record(conn, "spend_credentials", &expected.id.0)?
            .ok_or_else(|| spend::invalid("rotation credential missing"))?;
        if actual.reference != expected.reference
            || actual.version != expected.version
            || ((rotation.state == RotationState::Candidate || expected.id == rotation.old.id)
                && &actual != expected)
        {
            return Err(spend::invalid(
                "rotation credential changed; reconcile before continuing",
            ));
        }
    }
    let current: Vec<AccessRequirement> = records::<AccessRequirement>(conn, "spend_requirements")?
        .into_iter()
        .filter(|r| {
            r.credential.as_ref() == Some(&rotation.old.id)
                || r.credential.as_ref() == Some(&rotation.candidate.id)
        })
        .collect();
    if current != rotation.requirements {
        return Err(spend::invalid(
            "rotation consumer requirements changed; reconcile before continuing",
        ));
    }
    for consumer in &rotation.consumers {
        let environment: AccessEnvironment =
            record(conn, "spend_environments", &consumer.environment.0)?
                .ok_or_else(|| spend::invalid("rotation environment missing"))?;
        if environment.home_id != consumer.home_id {
            return Err(spend::invalid(
                "rotation environment changed; fresh verification required",
            ));
        }
    }
    Ok(())
}

impl SqliteStore {
    pub(crate) fn cancel_spend_rotation(
        &self,
        id: &crate::spend::rotation::RotationId,
    ) -> StoreResult<crate::spend::rotation::Rotation> {
        use crate::spend::rotation::{Rotation, RotationState};
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut rotation: Rotation = record(&tx, "spend_rotations", &id.0)?
            .ok_or_else(|| spend::invalid("rotation not found"))?;
        if !matches!(
            rotation.state,
            RotationState::Candidate | RotationState::Cancelled
        ) {
            return Err(spend::invalid(
                "activation already occurred; finish consumer cutover and retirement",
            ));
        }
        rotation.state = RotationState::Cancelled;
        write_rotation(&tx, &rotation)?;
        tx.commit()?;
        Ok(rotation)
    }
}

impl SqliteStore {
    pub(crate) fn reconcile_spend_rotation(
        &self,
        id: &crate::spend::rotation::RotationId,
        consumer_inventory_evidence: Option<String>,
    ) -> StoreResult<crate::spend::rotation::Rotation> {
        use crate::spend::rotation::{Rotation, RotationConsumer, RotationState};
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut rotation: Rotation = record(&tx, "spend_rotations", &id.0)?
            .ok_or_else(|| spend::invalid("rotation not found"))?;
        if matches!(
            rotation.state,
            RotationState::Retired | RotationState::Cancelled
        ) {
            return Err(spend::invalid("rotation already closed"));
        }
        let active: Credential = record(&tx, "spend_credentials", &rotation.old.id.0)?
            .ok_or_else(|| spend::invalid("active credential missing"))?;
        let candidate: Credential = record(&tx, "spend_credentials", &rotation.candidate.id.0)?
            .ok_or_else(|| spend::invalid("candidate credential missing"))?;
        let expected_active = if rotation.state == RotationState::Candidate {
            &rotation.old
        } else {
            &rotation.candidate
        };
        // Reconciliation never silently switches a consumer's active key or the key to retire.
        if active.reference != expected_active.reference
            || active.version != expected_active.version
        {
            return Err(spend::invalid("active reference changed outside rotation; restore its recorded reference before reconciliation"));
        }
        if candidate
            .version
            .as_ref()
            .is_none_or(|v| v.trim().is_empty())
            || (rotation.state == RotationState::Candidate
                && candidate.reference == active.reference)
            || (rotation.state != RotationState::Candidate
                && (candidate.reference != active.reference || candidate.version != active.version))
        {
            return Err(spend::invalid("candidate must retain the activated reference or be a distinct versioned replacement before activation"));
        }
        let requirements: Vec<AccessRequirement> =
            records::<AccessRequirement>(&tx, "spend_requirements")?
                .into_iter()
                .filter(|r| {
                    r.credential.as_ref() == Some(&rotation.old.id)
                        || r.credential.as_ref() == Some(&rotation.candidate.id)
                })
                .collect();
        let consumers = records::<AccessEnvironment>(&tx, "spend_environments")?
            .into_iter()
            .filter(|e| requirements.iter().any(|r| r.environment == e.id))
            .map(|e| RotationConsumer {
                environment: e.id,
                home_id: e.home_id,
            })
            .collect::<Vec<_>>();
        let mut expected = rotation.candidate.clone();
        expected.id = rotation.old.id.clone();
        let changed = rotation.requirements != requirements
            || rotation.consumers != consumers
            || (rotation.state == RotationState::Candidate
                && (rotation.candidate != candidate || rotation.old != active))
            || (rotation.state != RotationState::Candidate && expected != active);
        if changed {
            rotation.inventory_revision = rotation
                .inventory_revision
                .checked_add(1)
                .ok_or_else(|| spend::invalid("rotation inventory revision overflow"))?;
            rotation.consumer_inventory_evidence = None;
            rotation.requirements = requirements;
            rotation.consumers = consumers;
            // After activation, preserve the original key's retirement identity.
            if rotation.state == RotationState::Candidate {
                rotation.old = active;
                rotation.candidate = candidate;
            } else {
                rotation.candidate = active;
                rotation.candidate.id = candidate.id;
            }
        }
        if let Some(evidence) = consumer_inventory_evidence.filter(|e| !e.trim().is_empty()) {
            rotation.consumer_inventory_evidence = Some(evidence);
        }
        write_rotation(&tx, &rotation)?;
        tx.commit()?;
        Ok(rotation)
    }
}
