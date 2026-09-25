use loopflow::controller::wave::metrics::MetricPortfolioDto;
use loopflow::durable::WorkStatus;
use loopflow::lf::commands::waves::{Evidence, RoadmapSnapshot, WaveDetailSnapshot};
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
    assert_eq!(
        snapshot.projects[0].flows.as_ref().unwrap().recommended,
        None
    );
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
    assert_eq!(
        snapshot
            .chapter
            .as_ref()
            .unwrap()
            .flows
            .recommended
            .as_deref(),
        Some("task-design")
    );
    assert!(!snapshot.wave.paused);
    assert!(snapshot.wave.enabled);

    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: WaveDetailSnapshot = serde_json::from_str(&encoded).unwrap();
    assert!(!decoded.wave.paused);
    assert!(decoded.wave.enabled);
    assert_eq!(
        decoded.chapter.as_ref().unwrap().flows,
        snapshot.chapter.as_ref().unwrap().flows
    );

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
fn status_preserves_stranded_tasks_without_a_project_layer() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert_eq!(snapshot.unavailable_tasks[0].status, WorkStatus::Ready);
    let Evidence::Ok { items, .. } = snapshot.tasks else {
        panic!("available tasks")
    };
    assert_eq!(items[0].runtime.as_ref().unwrap().reason, "ready");
    let encoded = serde_json::from_str::<serde_json::Value>(WAVE_DETAIL).unwrap();
    assert!(encoded.get("projects").is_none());
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
    let Evidence::Ok { items, .. } = &roadmap.waves[0].tasks else {
        panic!("fixture has current Project evidence")
    };
    assert!(!items.is_empty());
    assert_eq!(
        roadmap.waves[0].chapter.as_ref().unwrap().flows.recommended,
        None
    );
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
    assert_eq!(portfolio.metrics.len(), 10);
    assert_eq!(portfolio.contract_issues.len(), 5);
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
            "untargeted",
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

#[test]
fn chapter_history_keeps_dated_task_evidence() {
    let snapshot: loopflow::work::chapter::ChapterSnapshot = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/chapter_snapshot.json"
    ))
    .unwrap();
    assert_eq!(snapshot.closed_at, Some(snapshot.observed_at));
    assert_eq!(snapshot.metrics_evaluated_at, 100);
    assert_eq!(snapshot.content.metric_targets.len(), 1);
    assert_eq!(
        snapshot.content.metric_targets[0].target,
        snapshot.metrics.metrics[0].target.clone().unwrap()
    );
    assert!(!snapshot.tasks[0].task.completed);
    assert_eq!(
        snapshot.tasks[0].disposition,
        loopflow::work::chapter::TaskDisposition::Move
    );
    let history: Vec<loopflow::work::chapter::ChapterHistoryEntry> = serde_json::from_str(
        include_str!("../../../tests/fixtures/dto/chapter_history.json"),
    )
    .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].source_project_id, snapshot.source_project_id);
}
