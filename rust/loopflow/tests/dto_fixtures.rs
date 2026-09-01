use loopflow::controller::wave::metrics::MetricPortfolioDto;
use loopflow::durable::WorkStatus;
use loopflow::lf::commands::waves::{RoadmapSnapshot, WaveDetailSnapshot};
use loopflow::ops::pm::PmShowResult;

const PM_SHOW: &str = include_str!("../../../tests/fixtures/dto/pm_show.json");
const WAVE_DETAIL: &str = include_str!("../../../tests/fixtures/dto/wave_detail.json");
const ROADMAP: &str = include_str!("../../../tests/fixtures/dto/roadmap_snapshot.json");
const METRIC_PORTFOLIO: &str = include_str!("../../../tests/fixtures/dto/metric_portfolio.json");

#[test]
fn pm_show_preserves_repository_team_and_project_ownership() {
    let snapshot: PmShowResult = serde_json::from_str(PM_SHOW).unwrap();

    assert_eq!(snapshot.wave, "survival/infrastructure");
    assert_eq!(
        snapshot.projects[0].initiative_ids,
        ["initiative-infrastructure"]
    );
    assert_eq!(snapshot.projects[0].name, "Gmail");
    assert_eq!(snapshot.projects[0].team_ids, ["team-loo"]);
    assert_eq!(snapshot.items[0].identifier, "LOO-2");
    assert_eq!(snapshot.items[0].project_id, "project-gmail");
    assert_eq!(snapshot.items[0].team_id, "team-loo");

    let round_trip = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_str::<PmShowResult>(&round_trip).unwrap(),
        snapshot
    );
}

#[test]
fn pm_show_rejects_a_legacy_item_without_stable_ownership() {
    let mut fixture: serde_json::Value = serde_json::from_str(PM_SHOW).unwrap();
    fixture["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("project_id");

    let error = serde_json::from_value::<PmShowResult>(fixture).unwrap_err();
    assert!(error.to_string().contains("project_id"));
}

#[test]
fn wave_detail_requires_machine_and_turn_controls() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert!(!snapshot.wave.paused);
    assert!(snapshot.wave.enabled);

    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: WaveDetailSnapshot = serde_json::from_str(&encoded).unwrap();
    assert!(!decoded.wave.paused);
    assert!(decoded.wave.enabled);

    let mut legacy: serde_json::Value = serde_json::from_str(WAVE_DETAIL).unwrap();
    legacy["wave"].as_object_mut().unwrap().remove("paused");
    let error = serde_json::from_value::<WaveDetailSnapshot>(legacy).unwrap_err();
    assert!(error.to_string().contains("paused"));

    let mut missing_enabled: serde_json::Value = serde_json::from_str(WAVE_DETAIL).unwrap();
    missing_enabled["wave"]
        .as_object_mut()
        .unwrap()
        .remove("enabled");
    let error = serde_json::from_value::<WaveDetailSnapshot>(missing_enabled).unwrap_err();
    assert!(error.to_string().contains("enabled"));
}

#[test]
fn status_surfaces_keep_last_failure_out_of_current_truth() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    let project = &snapshot.projects[0];
    let runtime = project.runtime.as_ref().unwrap();
    let failure = runtime.last_failure.as_ref().unwrap();

    assert_eq!(runtime.status, WorkStatus::Ready);
    assert_eq!(runtime.reason, "ready");
    assert!(!runtime.reason.contains("credential"));
    assert_eq!(
        failure.message,
        "project runner failed: credential is missing"
    );
    assert!(failure.occurred_at < time::OffsetDateTime::now_utc());
    assert_eq!(
        snapshot.unavailable_projects[0].status,
        WorkStatus::Abandoned
    );
    assert_eq!(
        snapshot.unavailable_projects[0].tasks[0].status,
        WorkStatus::Ready
    );
    let task_runtime = snapshot.projects[0].tasks[0].runtime.as_ref().unwrap();
    assert_eq!(task_runtime.reason, "ready");
}

#[test]
fn status_and_roadmap_require_the_shared_metric_portfolio() {
    let detail: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert!(matches!(
        detail.metric_portfolio.metrics.as_slice(),
        [metric] if metric.identity.metric_id == "task-loop-trust"
            && matches!(metric.evidence, loopflow::controller::wave::metrics::MetricEvidenceDto::Met { .. })
    ));

    let roadmap: RoadmapSnapshot = serde_json::from_str(ROADMAP).unwrap();
    assert_eq!(
        roadmap.waves[0].metric_portfolio.metrics[0]
            .identity
            .metric_id,
        "task-loop-trust"
    );
    assert!(roadmap.waves[1].metric_portfolio.metrics.is_empty());

    let mut detail_without_metrics: serde_json::Value = serde_json::from_str(WAVE_DETAIL).unwrap();
    detail_without_metrics
        .as_object_mut()
        .unwrap()
        .remove("metric_portfolio");
    let error = serde_json::from_value::<WaveDetailSnapshot>(detail_without_metrics).unwrap_err();
    assert!(error.to_string().contains("metric_portfolio"));

    let mut roadmap_without_metrics: serde_json::Value = serde_json::from_str(ROADMAP).unwrap();
    roadmap_without_metrics["waves"][0]
        .as_object_mut()
        .unwrap()
        .remove("metric_portfolio");
    let error = serde_json::from_value::<RoadmapSnapshot>(roadmap_without_metrics).unwrap_err();
    assert!(error.to_string().contains("metric_portfolio"));
}

#[test]
fn metric_portfolio_fixture_locks_every_tagged_payload() {
    let portfolio: MetricPortfolioDto = serde_json::from_str(METRIC_PORTFOLIO).unwrap();
    assert_eq!(portfolio.metrics.len(), 9);
    assert_eq!(portfolio.contract_issues.len(), 4);
    assert_eq!(
        portfolio.metrics[0].description,
        "Fraction of qualifying events that settled successfully."
    );

    let value: serde_json::Value = serde_json::from_str(METRIC_PORTFOLIO).unwrap();
    let evidence = value["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|metric| metric["evidence"]["kind"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        evidence,
        [
            "met",
            "missed",
            "unknown",
            "unknown",
            "unknown",
            "unknown",
            "unknown",
            "unknown",
            "unavailable",
        ]
    );

    let mut missing_required = value;
    missing_required["metrics"][0]
        .as_object_mut()
        .unwrap()
        .remove("description");
    let error = serde_json::from_value::<MetricPortfolioDto>(missing_required).unwrap_err();
    assert!(error.to_string().contains("description"));

    let mut with_unknown_field: serde_json::Value = serde_json::from_str(METRIC_PORTFOLIO).unwrap();
    with_unknown_field["metrics"][0]["future_field"] = serde_json::json!(true);
    serde_json::from_value::<MetricPortfolioDto>(with_unknown_field).unwrap();
}
