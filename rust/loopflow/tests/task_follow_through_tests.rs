mod support;

use loopflow::store::sqlite::SqliteStore;
use loopflow::work::task::follow_through::{FollowThroughIntent, FollowThroughLink};
use loopflow::work::task::{TaskEventKind, TaskFollowUp};
use loopflow_test_support::TestRepo;
use support::register_task_with_pr;

fn intent(key: &str) -> FollowThroughIntent {
    FollowThroughIntent {
        key: key.into(),
        issue_id: uuid::Uuid::new_v4().to_string(),
        relation_id: uuid::Uuid::new_v4().to_string(),
        project_id: "original-project".into(),
        team_id: "team".into(),
        state_id: Some("todo".into()),
        wave: "product".into(),
        title: "Verify installed release".into(),
        notes: "Run the accepted check on the installed version; retain failures".into(),
        due: Some("2026-10-08".into()),
        existing: false,
    }
}

#[test]
fn retry_retains_original_issue_payload_and_destination() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let fixture = register_task_with_pr(
        home.path(),
        repo.path(),
        "test/follow-through",
        &repo.head_sha(),
    );
    let sqlite = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let mut original = intent("installed");
    original.project_id = fixture.task.project_id.to_string();
    let saved = sqlite
        .reserve_follow_through(&fixture.task.id, &original)
        .unwrap();
    let mut retry = intent("installed");
    retry.project_id = "next-chapter".into();
    retry.title = "Edited title".into();
    assert_eq!(
        sqlite
            .reserve_follow_through(&fixture.task.id, &retry)
            .unwrap(),
        saved
    );
    assert_eq!(
        sqlite
            .task_follow_through(&fixture.task.id)
            .unwrap()
            .intents,
        [original]
    );
}

#[test]
fn filing_cannot_finish_until_every_issue_and_link_is_confirmed() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let fixture = register_task_with_pr(
        home.path(),
        repo.path(),
        "test/follow-through",
        &repo.head_sha(),
    );
    let sqlite = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    for key in ["installed", "usage"] {
        sqlite
            .reserve_follow_through(&fixture.task.id, &intent(key))
            .unwrap();
    }
    let snapshot = sqlite.task_follow_through(&fixture.task.id).unwrap();
    assert!(!snapshot.resolved());
    assert!(sqlite
        .finish_follow_through(&fixture.task.id, "none", true)
        .is_err());
    for (index, intent) in snapshot.intents.iter().enumerate() {
        assert!(sqlite
            .finish_follow_through(&fixture.task.id, "filed", false)
            .is_err());
        let mut link = FollowThroughLink {
            key: intent.key.clone(),
            issue_id: intent.issue_id.clone(),
            identifier: format!("LOO-{}", index + 1),
            url: None,
            due: intent.due.clone(),
        };
        sqlite.link_follow_through(&fixture.task.id, &link).unwrap();
        sqlite.link_follow_through(&fixture.task.id, &link).unwrap();
        // A historical provider issue need not have a local Task. Multiple
        // saved presentations of its link still identify one source.
        link.identifier = format!("RENAMED-{}", index + 1);
        sqlite.link_follow_through(&fixture.task.id, &link).unwrap();
        let sources = sqlite.follow_up_sources().unwrap();
        assert_eq!(sources[&link.issue_id].len(), 1);
        assert_eq!(
            sources[&link.issue_id][0].identifier,
            fixture.task.plan.identifier
        );
    }
    sqlite
        .finish_follow_through(&fixture.task.id, "Accepted evidence checks filed", false)
        .unwrap();
    sqlite
        .finish_follow_through(&fixture.task.id, "retry", false)
        .unwrap();
    let done = sqlite.task_follow_through(&fixture.task.id).unwrap();
    assert!(done.resolved());
    assert_eq!(done.links.len(), 2);
    assert_eq!(
        done.reason.as_deref(),
        Some("Accepted evidence checks filed")
    );
    assert!(sqlite
        .reserve_follow_through(&fixture.task.id, &intent("new-scope"))
        .is_err());
}

#[test]
fn historical_remaining_work_requires_a_new_disposition() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let fixture = register_task_with_pr(
        home.path(),
        repo.path(),
        "test/follow-through",
        &repo.head_sha(),
    );
    let sqlite = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(fixture.store.append_task_event(
            &fixture.task.id,
            &TaskEventKind::FollowUp {
                remaining: Some(TaskFollowUp {
                    outcome: "installed check".into(),
                    evidence: "successful invocation".into(),
                    check_at: 1,
                }),
                reason: "Accepted before upgrade".into(),
            },
        ))
        .unwrap();
    let pending = sqlite.task_follow_through(&fixture.task.id).unwrap();
    assert!(pending.needs_conversion);
    assert!(pending.scope_notes[0].contains("installed check"));
    assert!(pending.scope_notes[0].contains("successful invocation"));
    assert!(pending.scope_notes[0].contains("1970-01-01"));
    assert!(!pending.resolved());
    sqlite
        .finish_follow_through(
            &fixture.task.id,
            "Prior obligation independently satisfied",
            true,
        )
        .unwrap();
    assert!(sqlite
        .task_follow_through(&fixture.task.id)
        .unwrap()
        .resolved());
}
