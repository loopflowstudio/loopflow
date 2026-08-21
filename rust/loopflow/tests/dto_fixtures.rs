use loopflow::lf::commands::waves::WaveDetailSnapshot;
use loopflow::ops::pm::PmShowResult;
use loopflow::{child::CurrentWorkState, durable::WorkStatus};

const PM_SHOW: &str = include_str!("../../../tests/fixtures/dto/pm_show.json");
const WAVE_DETAIL: &str = include_str!("../../../tests/fixtures/dto/wave_detail.json");

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
    assert_eq!(runtime.current.state, CurrentWorkState::Ready);
    assert_eq!(runtime.reason, runtime.current.reason);
    assert_eq!(runtime.current.reason, "ready");
    assert!(!runtime.current.reason.contains("credential"));
    assert_eq!(
        failure.message,
        "project runner failed: credential is missing"
    );
    assert!(failure.run_id.is_some());
    assert!(failure.occurred_at < time::OffsetDateTime::now_utc());
    assert_eq!(
        snapshot.unavailable_projects[0].current.state,
        CurrentWorkState::Abandoned
    );
    assert_eq!(
        snapshot.unavailable_projects[0].tasks[0].current.state,
        CurrentWorkState::Ready
    );
    let task_runtime = snapshot.projects[0].tasks[0].runtime.as_ref().unwrap();
    assert_eq!(task_runtime.reason, task_runtime.current.reason);
    assert_eq!(task_runtime.current.reason, "ready");
}
