use std::path::Path;
use std::process::{Command, Output};

use loopflow::spend::{Inventory, Invoice};
use loopflow::store::{open_ephemeral_store, StorageConfig};
use serde_json::{json, Value};

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
    let mut inventory: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap();
    let mut invoice: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/invoice.json"
    ))
    .unwrap();
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
    let inventory: Inventory = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap();
    let invoice: Invoice = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/invoice.json"
    ))
    .unwrap();
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
async fn access_observations_cannot_certify_a_different_home() {
    let home = tempfile::tempdir().unwrap();
    let store = open_ephemeral_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
        .await
        .unwrap();
    let mut inventory: Inventory = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap();
    inventory.environments[0].home_id = Some(store.local_home().await.unwrap().id);
    inventory.requirements[0].credential = None;
    inventory.requirements[0].tool = Some("auth.report".into());
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let id = loopflow::spend::EnvironmentId("laptop".into());
    let local = store
        .verify_spend_access(id.clone(), "2026-09".into())
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
        .verify_spend_access(id, "2026-09".into())
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
    let inventory: Inventory = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap();
    store.import_spend_inventory(inventory).await.unwrap();
    let mut invoice: Invoice = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/invoice.json"
    ))
    .unwrap();
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
    let inventory: Inventory = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap();
    store.import_spend_inventory(inventory).await.unwrap();
    let mut invoice: Invoice = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/invoice.json"
    ))
    .unwrap();
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
    let mut inventory: Inventory = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap();
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
        .verify_spend_access(inventory.environments[0].id.clone(), "2026-09".into())
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
    let mut inventory: Inventory = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap();
    let mut invoice: Invoice = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/invoice.json"
    ))
    .unwrap();
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
    round_trip::<DependencyInspection>(&fixture["dependency"]);
    round_trip::<Report>(&fixture["report"]);
    round_trip::<ReportExport>(&fixture["export"]);
    round_trip::<Vec<DependencyHistory>>(&fixture["history"]);
}
