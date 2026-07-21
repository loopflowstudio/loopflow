use loopflow::lf::commands::waves::WaveDetailSnapshot;
use loopflow::ops::pm::PmShowResult;

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
fn wave_detail_requires_authored_turn_intent() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert!(!snapshot.wave.paused);

    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: WaveDetailSnapshot = serde_json::from_str(&encoded).unwrap();
    assert!(!decoded.wave.paused);

    let mut legacy: serde_json::Value = serde_json::from_str(WAVE_DETAIL).unwrap();
    legacy["wave"].as_object_mut().unwrap().remove("paused");
    let error = serde_json::from_value::<WaveDetailSnapshot>(legacy).unwrap_err();
    assert!(error.to_string().contains("paused"));
}
