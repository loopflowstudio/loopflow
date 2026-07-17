mod support;

use loopflow::child_session::ChildRef;
use loopflow::webhook::{ingest_event, WebhookEvent, WebhookOutcome};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard};
use time::OffsetDateTime;

const VIEWER: &str = "user-loopflow";

/// One verified webhook maps onto Task control through the durable substrate: an
/// issue edit becomes a Steer (once), a user comment becomes another Steer
/// (once), Loopflow's own change is skipped, and an issue with no Session is a
/// no-op.
#[test]
fn verified_webhooks_drive_task_control_exactly_once() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/linear-webhook";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "linear webhook\n");
    repo.stage_all();
    repo.commit("seed");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let issue_id = task.session.launch.issue.id.as_str().to_string();
    let target = ChildRef::Task(task.session.id.clone());
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let now = OffsetDateTime::now_utc();

    let edit = |revision: &str| WebhookEvent::IssueEdit {
        issue_id: issue_id.clone(),
        title: "New title".to_string(),
        description: "New body".to_string(),
        revision: revision.to_string(),
        actor_id: Some("user-human".to_string()),
    };
    let comment = |id: &str, author: &str| WebhookEvent::Comment {
        issue_id: issue_id.clone(),
        comment_id: id.to_string(),
        body: "please prioritize".to_string(),
        author_id: Some(author.to_string()),
    };

    // Issue edit → Steer; a redelivery applies nothing.
    let outcome = rt
        .block_on(ingest_event(
            &task.store,
            edit("2026-07-15T01:00:00.000Z"),
            VIEWER,
            now,
        ))
        .expect("edit");
    assert_eq!(
        outcome,
        WebhookOutcome::Edit {
            steer_applied: true
        }
    );
    let outcome = rt
        .block_on(ingest_event(
            &task.store,
            edit("2026-07-15T01:00:00.000Z"),
            VIEWER,
            now,
        ))
        .expect("edit redelivery");
    assert_eq!(
        outcome,
        WebhookOutcome::Edit {
            steer_applied: false
        }
    );

    // User comment → one Steer; a redelivery delivers nothing.
    let outcome = rt
        .block_on(ingest_event(
            &task.store,
            comment("c-1", "user-human"),
            VIEWER,
            now,
        ))
        .expect("comment");
    assert_eq!(outcome, WebhookOutcome::Comment { delivered: true });
    let outcome = rt
        .block_on(ingest_event(
            &task.store,
            comment("c-1", "user-human"),
            VIEWER,
            now,
        ))
        .expect("comment redelivery");
    assert_eq!(outcome, WebhookOutcome::Comment { delivered: false });

    // Loopflow's own comment never reaches the worker.
    let outcome = rt
        .block_on(ingest_event(
            &task.store,
            comment("c-2", VIEWER),
            VIEWER,
            now,
        ))
        .expect("self comment");
    assert_eq!(outcome, WebhookOutcome::SelfAuthored);

    // An issue with no Task Session is a no-op.
    let outcome = rt
        .block_on(ingest_event(
            &task.store,
            WebhookEvent::Comment {
                issue_id: "issue-unknown".to_string(),
                comment_id: "c-x".to_string(),
                body: "hi".to_string(),
                author_id: Some("user-human".to_string()),
            },
            VIEWER,
            now,
        ))
        .expect("no target");
    assert_eq!(outcome, WebhookOutcome::NoTarget);

    // Exactly two ordered Steers landed; authored direction never enters the
    // lifecycle command ledger.
    let work = rt
        .block_on(task.store.work_for_child(&target))
        .expect("work");
    let seed = rt
        .block_on(task.store.boundary_seed(&work))
        .expect("boundary seed");
    assert_eq!(seed.steers.len(), 2);
    assert!(seed.steers[0].text.contains("New title"));
    assert!(seed.steers[1].text.contains("please prioritize"));
    assert!(rt
        .block_on(task.store.list_child_commands(&target))
        .expect("commands")
        .is_empty());
}
