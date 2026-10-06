use std::path::Path;
use std::process::{Command, Output};

use loopflow::spend::{Inventory, Invoice};
use loopflow::store::{open_ephemeral_store, StorageConfig};
use serde_json::{json, Value};

fn inventory_fixture<T: serde::de::DeserializeOwned>() -> T {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap()
}

fn invoice_fixture<T: serde::de::DeserializeOwned>() -> T {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/invoice.json"
    ))
    .unwrap()
}

fn cli(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("LF_HOME", home)
        .env("PATH", "/nonexistent")
        .args(args)
        .output()
        .unwrap()
}
fn success(home: &Path, args: &[&str]) -> String {
    let output = cli(home, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn document(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn report(home: &Path) -> Value {
    serde_json::from_str(&success(
        home,
        &["auth", "report", "--period", "2026-09", "--json"],
    ))
    .unwrap()
}

#[test]
fn dependency_cost_round_trip() {
    let home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            home.path().join("loopflow.db"),
        )))
        .unwrap();
    let mut inventory: Value = inventory_fixture();
    let mut invoice: Value = invoice_fixture();
    // Same-named Waves remain distinct through their existing registered IDs.
    for (index, repo) in ["example/one", "example/two"].iter().enumerate() {
        let id = loopflow::id::WaveId::new();
        runtime
            .block_on(store.create_wave(&loopflow::work::wave::Wave::new(
                id.clone(),
                "spend".into(),
                (*repo).into(),
            )))
            .unwrap();
        inventory["rules"][1]["assignments"][index]["consumer"]["wave_id"] = json!(id);
        if index == 0 {
            inventory["rules"][0]["assignments"][0]["consumer"]["wave_id"] = json!(id);
        }
    }
    let inventory_path = home.path().join("inventory.json");
    let invoice_path = home.path().join("invoice.json");
    document(&inventory_path, &inventory);
    success(
        home.path(),
        &[
            "auth",
            "inventory",
            "import",
            inventory_path.to_str().unwrap(),
        ],
    );
    let import = || {
        success(
            home.path(),
            &[
                "auth",
                "source",
                "import",
                "fixture",
                "--period",
                "2026-09",
                "--file",
                invoice_path.to_str().unwrap(),
                "--json",
            ],
        )
    };
    document(&invoice_path, &invoice);
    let initial: Value = serde_json::from_str(&import()).unwrap();
    let first = report(home.path());
    assert_eq!(first["totals"][0]["billed"], "170.00");
    assert_eq!(first["totals"][0]["direct"], "90.00");
    assert_eq!(first["totals"][0]["allocated"], "60.00");
    assert_eq!(first["totals"][0]["unassigned"], "20.00");
    let wave = serde_json::from_value(
        inventory["rules"][0]["assignments"][0]["consumer"]["wave_id"].clone(),
    )
    .unwrap();
    let scoped = runtime
        .block_on(store.spend_report("2026-09".into(), Some("example/one".into()), Some(wave)))
        .unwrap();
    let export = scoped
        .export("example/one".into(), scoped.wave_id.clone())
        .unwrap();
    assert_eq!(export.totals[0].billed.0.to_string(), "120.00");
    assert!(export
        .amounts
        .iter()
        .all(|amount| amount.wave_id == scoped.wave_id));
    let other_repo = scoped
        .export("example/two".into(), scoped.wave_id.clone())
        .unwrap();
    assert!(other_repo.amounts.is_empty());
    assert!(other_repo.totals.is_empty());
    invoice["charges"].as_array_mut().unwrap().reverse();
    document(&invoice_path, &invoice);
    assert_eq!(serde_json::from_str::<Value>(&import()).unwrap(), initial);
    let tools: Value = serde_json::from_str(&success(
        home.path(),
        &[
            "auth",
            "dependency",
            "show",
            "tools",
            "--period",
            "2026-09",
            "--json",
        ],
    ))
    .unwrap();
    assert!(tools["attributed_cost"].is_null());
    assert_eq!(tools["linked_invoices"].as_array().unwrap().len(), 1);
    let unknown = success(
        home.path(),
        &[
            "auth",
            "dependency",
            "show",
            "unknown",
            "--period",
            "2026-09",
        ],
    );
    assert!(unknown.contains("Attributed cost: unknown"));
    assert!(success(home.path(), &["auth", "report", "--period", "2026-09"]).contains("170.00"));
    for charge in invoice["charges"].as_array_mut().unwrap() {
        if charge["amount"] == "100.00" {
            charge["amount"] = json!("90.00");
        }
    }
    invoice["total"] = json!("160.00");
    document(&invoice_path, &invoice);
    let corrected: Value = serde_json::from_str(&import()).unwrap();
    assert_ne!(corrected["revision"], initial["revision"]);
    assert_eq!(report(home.path())["totals"][0]["billed"], "160.00");
    let owner: Value = serde_json::from_str(&success(
        home.path(),
        &[
            "auth",
            "report",
            "--period",
            "2026-09",
            "--repo",
            "example/one",
            "--json",
        ],
    ))
    .unwrap();
    assert_eq!(owner["totals"][0]["billed"], "110.00");
    inventory["rules"][0]["revision"] = json!("2");
    inventory["rules"][0]["assignments"][0]["consumer"] =
        json!({"repo":"example/three","wave_id":null});
    document(&inventory_path, &inventory);
    success(
        home.path(),
        &[
            "auth",
            "inventory",
            "import",
            inventory_path.to_str().unwrap(),
        ],
    );
    assert_eq!(serde_json::from_str::<Value>(&import()).unwrap(), corrected);
    assert_eq!(report(home.path())["invoices"][0], corrected);
    invoice["complete"] = json!(false);
    document(&invoice_path, &invoice);
    assert!(!cli(
        home.path(),
        &[
            "auth",
            "source",
            "import",
            "fixture",
            "--period",
            "2026-09",
            "--file",
            invoice_path.to_str().unwrap()
        ]
    )
    .status
    .success());
    assert_eq!(report(home.path())["invoices"][0], corrected);
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let revisions: i64 = db
        .query_row("SELECT count(*) FROM spend_invoice_revisions", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(revisions, 2);
    let previous: String = db
        .query_row(
            "SELECT payload FROM spend_invoice_revisions WHERE revision=?1",
            [initial["revision"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&previous).unwrap(), initial);
}

#[test]
fn spend_dto_fixtures_round_trip() {
    let inventory: Inventory = inventory_fixture();
    let invoice: Invoice = invoice_fixture();
    for (encoded, fixture) in [
        (
            serde_json::to_value(inventory).unwrap(),
            include_str!("../../../tests/fixtures/dto/spend/inventory.json"),
        ),
        (
            serde_json::to_value(invoice).unwrap(),
            include_str!("../../../tests/fixtures/dto/spend/invoice.json"),
        ),
    ] {
        assert_eq!(encoded, serde_json::from_str::<Value>(fixture).unwrap());
    }
}

#[tokio::test]
async fn billing_probe_records_unavailable_without_claiming_scope_or_remote_access() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = inventory_fixture();
    inventory.environments[0].home_id = Some(store.local_home().await.unwrap().id);
    inventory.requirements[0].billing_probe = Some(loopflow::spend::BillingProbe::Runpod);
    inventory.requirements[0].revision = "runpod-probe".into();
    let id = inventory.environments[0].id.clone();
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    // Invalid period returns before lookup; this test never contacts Doppler or Runpod.
    let local = store
        .verify_spend_access(id.clone(), "invalid".into(), None)
        .await
        .unwrap();
    let observation = &local.observations[0];
    assert_eq!(
        observation.outcome,
        loopflow::spend::AccessOutcome::Unavailable
    );
    assert!(observation.scope_evidence.is_none());
    assert!(observation
        .gap
        .as_ref()
        .unwrap()
        .contains("invalid billing period"));
    assert_eq!(
        observation.credential_reference.as_ref(),
        Some(&inventory.credentials[0].reference)
    );
    inventory.environments[0].home_id = None;
    store.import_spend_inventory(inventory).await.unwrap();
    let remote = store
        .verify_spend_access(id, "2026-09".into(), None)
        .await
        .unwrap();
    assert_eq!(
        remote.observations[0].outcome,
        loopflow::spend::AccessOutcome::Unavailable
    );
    assert!(remote
        .current_observation(&remote.requirements[0])
        .is_none());
    assert!(remote
        .observations
        .iter()
        .all(|o| o.scope_evidence.is_none()));
    assert_eq!(remote.observations.len(), 2);
}

#[tokio::test]
async fn access_observations_cannot_certify_a_different_home() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = inventory_fixture();
    inventory.environments[0].home_id = Some(store.local_home().await.unwrap().id);
    inventory.requirements[0].credential = None;
    inventory.requirements[0].tool = Some("auth.report".into());
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let id = loopflow::spend::EnvironmentId("laptop".into());
    let local = store
        .verify_spend_access(id.clone(), "2026-09".into(), None)
        .await
        .unwrap();
    assert_eq!(
        local.observations[0].outcome,
        loopflow::spend::AccessOutcome::Success
    );
    assert!(local.observations[0].scope_evidence.is_none());
    inventory.environments[0].home_id = None;
    store.import_spend_inventory(inventory).await.unwrap();
    let remote = store
        .verify_spend_access(id, "2026-09".into(), None)
        .await
        .unwrap();
    assert_eq!(
        remote.observations[0].outcome,
        loopflow::spend::AccessOutcome::Unavailable
    );
    assert_eq!(
        remote.observations[1].outcome,
        loopflow::spend::AccessOutcome::Success
    );
    assert!(remote
        .coverage
        .iter()
        .any(|g| g.contains("current access unverified")));
}

#[tokio::test]
async fn invoice_completeness_multiplicity_and_unknown_periods() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let inventory: Inventory = inventory_fixture();
    store.import_spend_inventory(inventory).await.unwrap();
    let mut invoice: Invoice = invoice_fixture();
    invoice.charges.push(invoice.charges[2].clone());
    let duplicated = store.import_spend_invoice(invoice.clone()).await.unwrap();
    assert_eq!(duplicated.billed.0.to_string(), "190.00");
    assert_eq!(duplicated.difference.0.to_string(), "20.00");
    let report = store
        .spend_report("2026-10".into(), None, None)
        .await
        .unwrap();
    assert!(report.totals.is_empty());
    assert!(report.coverage.iter().any(|c| c.contains("unknown")));
    let unrelated = store
        .spend_report("2026-09".into(), Some("example/unknown".into()), None)
        .await
        .unwrap();
    assert!(unrelated.totals.is_empty());
    assert!(unrelated
        .coverage
        .iter()
        .any(|g| g.contains("attributed cost unknown")));
    invoice.charges.clear();
    invoice.total = loopflow::spend::Money::parse("0.00").unwrap();
    let zero = store.import_spend_invoice(invoice).await.unwrap();
    assert_eq!(zero.billed.0.to_string(), "0");
    assert_eq!(
        store
            .spend_report("2026-09".into(), None, None)
            .await
            .unwrap()
            .totals[0]
            .billed
            .0
            .to_string(),
        "0"
    );
}

#[tokio::test]
async fn designated_export_excludes_other_recipients_and_inventory() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let inventory: Inventory = inventory_fixture();
    store.import_spend_inventory(inventory).await.unwrap();
    let mut invoice: Invoice = invoice_fixture();
    invoice.charges[0].description = "unrelated confidential annotation".into();
    store.import_spend_invoice(invoice).await.unwrap();
    let output = home.path().join("report.json");
    success(
        home.path(),
        &[
            "auth",
            "export",
            "--period",
            "2026-09",
            "--repo",
            "example/two",
            "--output",
            output.to_str().unwrap(),
        ],
    );
    let contents = std::fs::read_to_string(output).unwrap();
    let export: loopflow::spend::ReportExport = serde_json::from_str(&contents).unwrap();
    assert_eq!(export.totals[0].billed.0.to_string(), "30.00");
    assert_eq!(export.amounts.len(), 1);
    assert_eq!(export.amounts[0].document_id, "invoice-1");
    for excluded in [
        "example/one",
        "unrelated confidential annotation",
        "BILLING_KEY",
        "170.00",
        "60.00",
        "account-1",
    ] {
        assert!(!contents.contains(excluded), "export contains {excluded}");
    }
    let report = store
        .spend_report("2026-09".into(), None, None)
        .await
        .unwrap();
    let unknown = report.export("example/missing".into(), None).unwrap();
    assert!(unknown.amounts.is_empty());
    assert!(unknown.totals.is_empty());
    assert!(unknown.coverage.iter().any(|g| g.contains("unknown")));
}

#[tokio::test]
async fn dependency_inspection_shows_only_its_access_and_dated_relationships() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = inventory_fixture();
    inventory.environments[0].home_id = Some(store.local_home().await.unwrap().id);
    inventory.requirements[0].credential = None;
    inventory.requirements[0].tool = Some("auth.report".into());
    let mut unrelated = inventory.requirements[0].clone();
    unrelated.id = loopflow::spend::RequirementId("unrelated".into());
    inventory.requirements.push(unrelated);
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    store
        .verify_spend_access(inventory.environments[0].id.clone(), "2026-09".into(), None)
        .await
        .unwrap();
    let id = loopflow::spend::DependencyId("workers".into());
    let inspection = store
        .spend_dependency(id.clone(), "2026-09".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(inspection.access.len(), 1);
    let access = &inspection.access[0];
    assert_eq!(access.requirements.len(), 1);
    assert_eq!(access.observations.len(), 1);
    assert_eq!(
        access
            .current_observation(&access.requirements[0])
            .unwrap()
            .outcome,
        loopflow::spend::AccessOutcome::Success
    );
    let text = success(
        home.path(),
        &[
            "auth",
            "dependency",
            "show",
            "workers",
            "--period",
            "2026-09",
        ],
    );
    assert!(text.contains("Current read: Success"));
    assert!(!text.contains("unrelated"));
    inventory.effective_from = "2026-10-01".into();
    inventory.dependencies[0].consumers[0].repo = "example/changed".into();
    inventory.dependencies[0].accounts.clear();
    inventory.dependencies[0].requirements.clear();
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    // An omitted dependency does not terminate its last effective relationships.
    inventory.effective_from = "2026-11-01".into();
    inventory.dependencies.clear();
    store.import_spend_inventory(inventory).await.unwrap();
    let history = store.spend_dependency_history(id.clone()).await.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].effective_to.as_deref(), Some("2026-10-01"));
    assert_eq!(history[0].dependency.consumers[0].repo, "example/one");
    assert_eq!(history[0].dependency.accounts.len(), 1);
    assert_eq!(history[1].dependency.consumers[0].repo, "example/changed");
    assert!(history[1].dependency.accounts.is_empty());
    assert!(history[1].effective_to.is_none());
    assert!(store
        .spend_dependency(id, "2026-09".into())
        .await
        .unwrap()
        .unwrap()
        .access
        .is_empty());
}

#[tokio::test]
async fn corrected_invoice_uses_new_rules_without_rewriting_older_revision() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = inventory_fixture();
    let mut invoice: Invoice = invoice_fixture();
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let original = store.import_spend_invoice(invoice.clone()).await.unwrap();
    inventory.rules[0].revision = "2".into();
    inventory.rules[0].assignments[0]
        .consumer
        .as_mut()
        .unwrap()
        .repo = "example/changed".into();
    store.import_spend_inventory(inventory).await.unwrap();
    invoice.charges[0].amount = loopflow::spend::Money::parse("90.00").unwrap();
    invoice.total = loopflow::spend::Money::parse("160.00").unwrap();
    let correction = store.import_spend_invoice(invoice).await.unwrap();
    let changed = store
        .spend_report("2026-09".into(), Some("example/changed".into()), None)
        .await
        .unwrap();
    assert_eq!(changed.totals[0].billed.0.to_string(), "80.00");
    assert!(correction
        .charges
        .iter()
        .flat_map(|c| &c.attribution)
        .filter(|a| a.rule.as_ref().is_some_and(|r| r.0 == "direct-rule"))
        .all(|a| a.rule_revision.as_deref() == Some("2")));
    let previous = store
        .import_spend_invoice(original.invoice.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(previous).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    assert_eq!(
        store
            .spend_report("2026-09".into(), None, None)
            .await
            .unwrap()
            .invoices[0]
            .revision,
        correction.revision
    );
}

#[test]
fn inspection_output_fixtures_round_trip() {
    use loopflow::spend::{
        AccessInspection, DependencyHistory, DependencyInspection, Report, ReportExport,
    };
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/outputs.json"
    ))
    .unwrap();
    fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(value: &Value) {
        let parsed: T = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), *value);
    }
    round_trip::<AccessInspection>(&fixture["access"]);
    round_trip::<loopflow::spend::consumer::ConsumptionReceipt>(&fixture["consumption_receipt"]);
    round_trip::<DependencyInspection>(&fixture["dependency"]);
    round_trip::<Report>(&fixture["report"]);
    round_trip::<ReportExport>(&fixture["export"]);
    round_trip::<Vec<DependencyHistory>>(&fixture["history"]);
    round_trip::<loopflow::spend::rotation::Rotation>(&fixture["rotation"]);
}

#[tokio::test]
async fn source_scope_can_shrink_without_losing_billed_evidence() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = inventory_fixture();
    let mut invoice: Invoice = invoice_fixture();
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let original = store.import_spend_invoice(invoice.clone()).await.unwrap();
    inventory.sources[0].accounts.clear();
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let report = store
        .spend_report("2026-09".into(), None, None)
        .await
        .unwrap();
    assert_eq!(report.invoices[0].revision, original.revision);
    assert_eq!(report.totals[0].billed.0.to_string(), "170.00");
    // Existing evidence remains idempotent; new documents use the current scope.
    assert_eq!(
        store
            .import_spend_invoice(invoice.clone())
            .await
            .unwrap()
            .revision,
        original.revision
    );
    invoice.document_id = "outside-current-scope".into();
    assert!(store.import_spend_invoice(invoice).await.is_err());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let links: i64 = db
        .query_row("SELECT count(*) FROM spend_source_accounts", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(links, 0);
    inventory.sources[0]
        .accounts
        .push(loopflow::spend::ServiceAccountId("missing".into()));
    assert!(store.import_spend_inventory(inventory).await.is_err());
    assert_eq!(
        store
            .spend_report("2026-09".into(), None, None)
            .await
            .unwrap()
            .invoices[0]
            .revision,
        original.revision
    );
}

#[tokio::test]
async fn historical_metadata_controls_project_and_tag_attribution() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = inventory_fixture();
    let mut invoice: Invoice = invoice_fixture();
    inventory.dependencies[0]
        .resources
        .push(inventory.resources[0].id.clone());
    inventory.resources[0].observed_at = 1788220800; // 2026-09-01 UTC
    inventory.resources[0].project = Some("build".into());
    inventory.resources[0]
        .tags
        .insert("owner".into(), "one".into());
    inventory.rules[0].project = Some("build".into());
    inventory.rules[0].tags.insert("owner".into(), "one".into());
    // Avoid applying the project rule to resources whose historical metadata is unknown.
    inventory.rules[0].resource = Some(inventory.resources[0].id.clone());
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let original = store.import_spend_invoice(invoice.clone()).await.unwrap();
    assert!(original
        .charges
        .iter()
        .filter(|c| c.charge.resource.as_ref() == Some(&inventory.resources[0].id))
        .all(|c| c.attribution[0].kind == loopflow::spend::AssignmentKind::Direct));
    inventory.effective_from = "2026-10-01".into();
    inventory.resources[0].observed_at = 1790812800;
    inventory.resources[0]
        .tags
        .insert("owner".into(), "two".into());
    inventory.accounts[0].evidence = "updated account observation".into();
    inventory.credentials[0].reference.name = "UPDATED_KEY".into();
    inventory.requirements[0].purpose = "updated purpose".into();
    inventory.requirements[0].revision = "2".into();
    inventory.environments[0].name = "renamed laptop".into();
    inventory.dependencies.clear(); // linked metadata alone must create a historical interval
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let history = store
        .spend_dependency_history(loopflow::spend::DependencyId("workers".into()))
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].resources[0].tags["owner"], "one");
    assert_eq!(history[1].resources[0].tags["owner"], "two");
    assert_eq!(history[0].credentials[0].reference.name, "BILLING_KEY");
    assert_eq!(history[1].credentials[0].reference.name, "UPDATED_KEY");
    assert_eq!(history[1].requirements[0].purpose, "updated purpose");
    assert_eq!(history[1].environments[0].name, "renamed laptop");
    assert_eq!(
        history[1].accounts[0].evidence,
        "updated account observation"
    );
    // A correction still uses September metadata, never October's tags.
    invoice.charges[0].amount = loopflow::spend::Money::parse("90.00").unwrap();
    invoice.total = loopflow::spend::Money::parse("160.00").unwrap();
    let corrected = store.import_spend_invoice(invoice.clone()).await.unwrap();
    assert!(corrected
        .charges
        .iter()
        .filter(|c| c.charge.resource.as_ref() == Some(&inventory.resources[0].id))
        .all(|c| c.attribution[0].kind == loopflow::spend::AssignmentKind::Direct));
    // A newly supplied dated change within September makes a whole-month charge unresolved.
    inventory.effective_from = "2026-09-15".into();
    inventory.resources[0].observed_at = 1789430400;
    store.import_spend_inventory(inventory).await.unwrap();
    invoice.document_id = "crosses-boundary".into();
    let crossed = store.import_spend_invoice(invoice).await.unwrap();
    assert!(crossed
        .charges
        .iter()
        .filter(|c| c.charge.resource.as_ref().is_some_and(|r| r.0 == "direct"))
        .all(
            |c| c.attribution[0].kind == loopflow::spend::AssignmentKind::Unassigned
                && c.gap.is_some()
        ));
    assert_eq!(
        serde_json::to_value(
            store
                .import_spend_invoice(original.invoice.clone())
                .await
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(original).unwrap()
    );
}

#[tokio::test]
async fn aws_cur_manifest_import_reconciles_and_preserves_last_good_invoice() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = inventory_fixture();
    inventory.accounts[0].provider = "aws".into();
    inventory.accounts[0].native_id = Some("000000000001".into());
    store.import_spend_inventory(inventory).await.unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/spend/aws-cur");
    for name in ["manifest.json", "usage.csv", "credits.csv"] {
        std::fs::copy(root.join(name), home.path().join(name)).unwrap();
    }
    let descriptor = json!({
        "account":"shared", "invoice_id":"fixture-invoice", "currency":"USD",
        "invoice_total":"90.00", "fetched_at":1791244800_i64, "generated_at":null,
        "manifest":"manifest.json", "directory":"."
    });
    let file = home.path().join("export.json");
    document(&file, &descriptor);
    let args = [
        "auth",
        "source",
        "import-aws-cur",
        "fixture",
        "--period",
        "2026-09",
        "--file",
        file.to_str().unwrap(),
        "--json",
    ];
    let original: Value = serde_json::from_str(&success(home.path(), &args)).unwrap();
    assert_eq!(original["billed"], "90.00");
    assert_eq!(original["difference"], "0.00");
    assert_eq!(original["invoice"]["charges"].as_array().unwrap().len(), 2);
    assert_eq!(
        serde_json::from_str::<Value>(&success(home.path(), &args)).unwrap(),
        original
    );
    // Reordering manifest parts cannot create another revision.
    let manifest_path = home.path().join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["reportKeys"].as_array_mut().unwrap().reverse();
    document(&manifest_path, &manifest);
    assert_eq!(
        serde_json::from_str::<Value>(&success(home.path(), &args)).unwrap(),
        original
    );
    // Missing parts and reconciliation failure cannot replace good evidence.
    std::fs::remove_file(home.path().join("credits.csv")).unwrap();
    assert!(!cli(home.path(), &args).status.success());
    std::fs::copy(root.join("credits.csv"), home.path().join("credits.csv")).unwrap();
    let usage = std::fs::read_to_string(home.path().join("usage.csv")).unwrap();
    std::fs::write(
        home.path().join("usage.csv"),
        usage.replace("100.00", "80.00"),
    )
    .unwrap();
    assert!(!cli(home.path(), &args).status.success());
    assert_eq!(
        report(home.path())["invoices"][0]["revision"],
        original["revision"]
    );
    let failed = report(home.path());
    assert_eq!(failed["totals"][0]["billed"], "90.00");
    let observations = failed["imports"].as_array().unwrap();
    assert_eq!(observations.len(), 5);
    assert_eq!(observations.last().unwrap()["succeeded"], false);
    assert!(
        observations.last().unwrap()["observed_at"]
            .as_i64()
            .unwrap()
            > 0
    );
    assert!(failed["coverage"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap.as_str().unwrap().contains("import failed")));
    let mut correction = descriptor.clone();
    correction["invoice_total"] = json!("70.00");
    document(&file, &correction);
    let corrected: Value = serde_json::from_str(&success(home.path(), &args)).unwrap();
    assert_ne!(corrected["revision"], original["revision"]);
    assert_eq!(report(home.path())["totals"][0]["billed"], "70.00");
    assert!(!report(home.path())["coverage"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap.as_str().unwrap().contains("import failed")));
    // A not-yet-finalized row is never substituted for a billed document.
    std::fs::write(
        home.path().join("usage.csv"),
        usage.replace("fixture-invoice", ""),
    )
    .unwrap();
    assert!(!cli(home.path(), &args).status.success());
    assert_eq!(
        report(home.path())["invoices"][0]["revision"],
        corrected["revision"]
    );
    std::fs::write(&file, "malformed-import-sentinel").unwrap();
    let rejected = cli(home.path(), &args);
    assert!(!rejected.status.success());
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("malformed-import-sentinel"));
    let final_report = report(home.path());
    assert_eq!(final_report["totals"][0]["billed"], "70.00");
    assert_eq!(final_report["imports"].as_array().unwrap().len(), 8);
    assert_eq!(final_report["imports"][7]["succeeded"], false);
    assert!(!serde_json::to_string(&final_report)
        .unwrap()
        .contains("malformed-import-sentinel"));
}

#[tokio::test]
async fn currencies_remain_separate_and_failed_inventory_is_atomic() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let inventory: Inventory = inventory_fixture();
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let invoice: Invoice = invoice_fixture();
    store.import_spend_invoice(invoice.clone()).await.unwrap();
    let mut euro = invoice;
    euro.currency = "EUR".into();
    euro.document_id = "euro-document".into();
    store.import_spend_invoice(euro).await.unwrap();
    let before = store
        .spend_dependency(inventory.dependencies[0].id.clone(), "2026-09".into())
        .await
        .unwrap()
        .unwrap();
    let history = store
        .spend_dependency_history(inventory.dependencies[0].id.clone())
        .await
        .unwrap();
    let mut invalid = inventory.clone();
    invalid.credentials[0].purpose = "must roll back".into();
    invalid.accounts[0].evidence = "must roll back".into();
    invalid.dependencies[0].name = "must roll back".into();
    invalid.dependencies[0]
        .accounts
        .push(loopflow::spend::ServiceAccountId("missing".into()));
    assert!(store.import_spend_inventory(invalid).await.is_err());
    let after = store
        .spend_dependency(inventory.dependencies[0].id.clone(), "2026-09".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_eq!(
        serde_json::to_value(history).unwrap(),
        serde_json::to_value(
            store
                .spend_dependency_history(inventory.dependencies[0].id.clone())
                .await
                .unwrap()
        )
        .unwrap()
    );
    let report = store
        .spend_report("2026-09".into(), None, None)
        .await
        .unwrap();
    assert_eq!(report.totals.len(), 2);
    for total in report.totals {
        assert!(matches!(total.currency.as_str(), "USD" | "EUR"));
        assert_eq!(total.billed.0.to_string(), "170.00");
    }
}

#[tokio::test]
async fn designated_access_requires_its_scope_and_keeps_unavailable_evidence() {
    use loopflow::spend::{AccessOutcome, Consumer, EnvironmentId};

    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = inventory_fixture();
    inventory.environments[0].home_id = Some(store.local_home().await.unwrap().id);
    inventory.requirements[0].credential = None;
    inventory.requirements[0].tool = Some("auth.export".into());
    assert!(store
        .import_spend_inventory(inventory.clone())
        .await
        .is_err());
    inventory.requirements[0].report_consumer = Some(Consumer {
        repo: "example/two".into(),
        wave_id: None,
    });
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let id = EnvironmentId("laptop".into());
    let missing = store
        .verify_spend_access(id.clone(), "2026-09".into(), None)
        .await
        .unwrap();
    assert_eq!(missing.observations[0].outcome, AccessOutcome::Unavailable);
    assert!(missing.observations[0].report_receipt.is_none());

    store.import_spend_invoice(invoice_fixture()).await.unwrap();
    let report = store
        .spend_report("2026-09".into(), None, None)
        .await
        .unwrap();
    let wrong = report.export("example/one".into(), None).unwrap();
    let path = home.path().join("wrong.json");
    std::fs::write(&path, serde_json::to_vec(&wrong).unwrap()).unwrap();
    let denied = store
        .verify_spend_access(id.clone(), "2026-09".into(), Some(path.clone()))
        .await
        .unwrap();
    assert_eq!(denied.observations[0].outcome, AccessOutcome::Denied);
    assert!(denied.observations[0].report_receipt.is_none());
    assert_eq!(denied.observations.len(), 2);

    // Scope changes cannot reuse a prior requirement revision.
    inventory.requirements[0]
        .report_consumer
        .as_mut()
        .unwrap()
        .repo = "example/one".into();
    assert!(store
        .import_spend_inventory(inventory.clone())
        .await
        .is_err());
    inventory.requirements[0].revision = "2".into();
    inventory.environments[0].home_id = None;
    store.import_spend_inventory(inventory).await.unwrap();
    let remote = store
        .verify_spend_access(id, "2026-09".into(), Some(path))
        .await
        .unwrap();
    assert_eq!(remote.observations[0].outcome, AccessOutcome::Unavailable);
    assert!(remote.observations[0].report_receipt.is_none());
    assert!(remote
        .current_observation(&remote.requirements[0])
        .is_none());
}
