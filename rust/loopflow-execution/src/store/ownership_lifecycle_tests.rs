//! Ownership acquisition and exchange with the hub through application commands.

use crate::durable::{OwningMachineSource, TaskExecutionSource};
use crate::store::ownership::local_label;
use loopflow_model::comment::{TaskComment, TaskCommentAuthor};
use loopflow_model::durable::{MachineId, OwnershipKind, RepositoryId, TaskId};
use loopflow_storage::sqlite::task_moves::MoveStage;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::NewTask;
use loopflow_test_support::TestRepo;
fn published_bodies(store: &SqliteStore, task: &TaskId) -> Vec<String> {
    let conn = store.conn.lock().unwrap();
    let mut statement = conn
        .prepare("SELECT body FROM task_comments WHERE task_id=?1")
        .unwrap();
    statement
        .query_map([task.as_str()], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn a_claim_names_its_person_once_and_the_latest_name_shows() {
    let (_dir, store, task) = loopflow_storage::test_fixture::unowned_task();
    store
        .record_local_person("linear:lee", Some("Lee"))
        .unwrap();
    let claim = crate::store::ownership::claim_task(&store, &task)
        .unwrap()
        .unwrap();
    assert_eq!(claim.person_id.as_deref(), Some("linear:lee"));
    store
        .record_local_person("linear:lee", Some("Lee Park"))
        .unwrap();
    let ownership = crate::store::ownership::task_ownership(&store, &task).unwrap();
    assert_eq!(ownership.log[0].person.as_deref(), Some("Lee Park"));
    assert_eq!(
        ownership.owner.unwrap().person_id.as_deref(),
        Some("linear:lee")
    );
}

#[test]
fn connecting_linear_keeps_journal_era_claims_yours() {
    let (dir, store, task) = loopflow_storage::test_fixture::unowned_task();
    for args in [&["init", "-q"][..], &["config", "user.email", "a@x"]] {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .status()
            .unwrap();
        assert!(status.success());
    }
    store.record_local_person("journal:k", None).unwrap();
    let claim = crate::store::ownership::claim_task(&store, &task)
        .unwrap()
        .unwrap();
    assert_eq!(claim.person_id.as_deref(), Some("journal:k"));
    assert_eq!(claim.person_key.as_deref(), Some("a@x"));

    store.record_local_person("linear:v", Some("Lee")).unwrap();
    let ownership = crate::store::ownership::task_ownership(&store, &task).unwrap();
    assert_eq!(ownership.superseded_by, None);
    assert!(crate::store::ownership::unassign_task(&store, &task)
        .unwrap()
        .is_some());
    assert!(crate::store::ownership::claim_task(&store, &task)
        .unwrap()
        .is_some());
}

#[test]
fn unassigning_an_unowned_task_writes_nothing() {
    let (_dir, store, task) = loopflow_storage::test_fixture::unowned_task();
    assert_eq!(
        crate::store::ownership::unassign_task(&store, &task).unwrap(),
        None
    );
    assert!(crate::store::ownership::task_ownership(&store, &task)
        .unwrap()
        .log
        .is_empty());
}

#[test]
fn a_private_machine_publishes_its_id_but_not_its_hostname() {
    let (_dir, store, task) = loopflow_storage::test_fixture::unowned_task();
    crate::store::ownership::claim_task(&store, &task)
        .unwrap()
        .unwrap();
    let local = crate::store::machines::local_machine(&store).unwrap().id;
    let bodies = published_bodies(&store, &task);
    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].contains(local.as_str()));
    assert!(!bodies[0].contains(&local_label()));
}

#[test]
fn a_move_does_not_publish_the_targets_local_label() {
    let (_dir, store, task) = loopflow_storage::test_fixture::unowned_task();
    crate::store::ownership::claim_task(&store, &task)
        .unwrap()
        .unwrap();
    let mini = MachineId::new();
    crate::store::machines::add_machine(&store, &mini, "ssh://mini.example", "kims-basement-mini")
        .unwrap();
    let entry = crate::store::ownership::move_claim(&store, &task, &mini).unwrap();
    assert_eq!(entry.machine_label, None);
    let bodies = published_bodies(&store, &task);
    assert!(bodies.iter().any(|body| body.contains(mini.as_str())));
    assert!(bodies
        .iter()
        .all(|body| !body.contains("kims-basement-mini")));
}

#[test]
fn a_move_leaves_from_its_claim_until_it_is_sent_or_handed_back() {
    let (_dir, store, task) = loopflow_storage::test_fixture::unowned_task();
    let (laptop, mini) = (
        crate::store::machines::local_machine(&store).unwrap().id,
        MachineId::new(),
    );
    crate::store::machines::add_machine(&store, &mini, "ssh://mini.example", "mini").unwrap();

    // An unowned Task is claimed here, then its claim moves to mini.
    let claim = crate::store::ownership::begin_task_move(&store, &task, &mini).unwrap();
    let ownership = crate::store::ownership::task_ownership(&store, &task).unwrap();
    assert_eq!(ownership.log.len(), 2);
    assert_eq!(ownership.owner.unwrap().machine.id, Some(mini.clone()));
    store
        .record_move_stopped(&task, "{\"step\":\"implement\"}")
        .unwrap();
    let leaving = store.task_move_in_flight(&task).unwrap().unwrap();
    assert_eq!(leaving.to_machine_id, mini);
    assert_eq!(
        leaving.stage,
        MoveStage::Leaving {
            claim_id: claim.id.clone(),
            stopped: Some("{\"step\":\"implement\"}".into())
        }
    );

    // Handed back, nothing is leaving and the laptop owns it again.
    crate::store::ownership::hand_back(&store, &task, &claim).unwrap();
    assert_eq!(store.task_move_in_flight(&task).unwrap(), None);
    let owner = crate::store::ownership::task_ownership(&store, &task)
        .unwrap()
        .owner
        .unwrap();
    assert_eq!(owner.machine.id, Some(laptop.clone()));

    // Sent, the move is no longer leaving.
    crate::store::ownership::begin_task_move(&store, &task, &mini).unwrap();
    store.record_move_sent(&task, "{}").unwrap();
    let sent = store.task_move_in_flight(&task).unwrap().unwrap();
    assert!(matches!(sent.stage, MoveStage::Sent { .. }));

    // Claimed back here before it arrives, the move is over; its row stays.
    crate::store::ownership::claim_task(&store, &task)
        .unwrap()
        .unwrap();
    assert_eq!(store.task_move_in_flight(&task).unwrap(), None);
    let moves: i64 = store
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM task_moves WHERE task_id=?1 AND state='sent'",
            [task.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(moves, 1);
}

/// One Machine's store and checkout of the shared repository.
struct Side {
    store: SqliteStore,
    repo: TestRepo,
    path: String,
}

/// A laptop whose plan is on mini, its hub; `to_laptop`/`to_mini` are one
/// exchange each, as `lf task sync` on the laptop makes it.
struct Pair {
    laptop: Side,
    mini: Side,
    task: TaskId,
    _dir: tempfile::TempDir,
}

impl Pair {
    /// mini creates the Task; the laptop has not seen it yet.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let repository = RepositoryId::new();
        let side = |name: &str| {
            let store = crate::store::opening::open_ephemeral(&dir.path().join(name)).unwrap();
            let repo = TestRepo::new();
            let path = repo.path().canonicalize().unwrap();
            let path = path.to_str().unwrap().to_owned();
            crate::repository::location::bind_repository(&store, &path, &repository).unwrap();
            Side { store, repo, path }
        };
        let (laptop, mini) = (side("laptop.db"), side("mini.db"));
        let project =
            crate::work::wave::bootstrap::ensure_wave_project(&mini.store, &mini.path, "owners")
                .unwrap();
        let task = mini
            .store
            .create_task(&NewTask {
                parent_task_id: None,
                repository_id: crate::repository::location::ensure_repository(
                    &mini.store,
                    &mini.path,
                )
                .unwrap(),
                id: TaskId::new(),
                project_id: Some(project.id),
                title: "Ownership demo".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap()
            .id;
        Self {
            laptop,
            mini,
            task,
            _dir: dir,
        }
    }

    /// One exchange between the laptop and mini, its hub.
    fn exchange(&self) {
        loopflow_storage::test_fixture::exchange(
            &self.laptop.store,
            &self.laptop.path,
            &self.mini.store,
            &self.mini.path,
        );
    }

    fn to_laptop(&self) {
        self.exchange();
    }

    fn to_mini(&self) {
        self.exchange();
    }
}

#[test]
fn of_two_claims_made_apart_the_first_holds_and_a_claim_that_saw_it_takes_over() {
    let pair = Pair::new();
    let task = &pair.task;
    pair.to_laptop();
    let mini = crate::store::machines::local_machine(&pair.mini.store)
        .unwrap()
        .id;
    let laptop = crate::store::machines::local_machine(&pair.laptop.store)
        .unwrap()
        .id;
    // No owning Machine is configured, so each Machine may claim for itself.
    crate::store::ownership::claim_task(&pair.mini.store, task)
        .unwrap()
        .unwrap();
    // Entries order by the millisecond; the laptop's claim is the later one.
    std::thread::sleep(std::time::Duration::from_millis(5));
    crate::store::ownership::claim_task(&pair.laptop.store, task)
        .unwrap()
        .unwrap();
    pair.to_mini();
    pair.to_laptop();
    for side in [&pair.mini, &pair.laptop] {
        let ownership = crate::store::ownership::task_ownership(&side.store, task).unwrap();
        assert_eq!(ownership.owner.unwrap().machine.id, Some(mini.clone()));
        assert!(ownership.superseded_by.is_none());
        assert_eq!(ownership.log.len(), 2);
    }
    assert_eq!(
        crate::store::ownership::task_execution_route(&pair.mini.store, task)
            .unwrap()
            .machine_id,
        mini
    );
    // The laptop's Sessions are told who was first; mini's hear nothing.
    let told = |side: &Side, text: &str| {
        side.store
            .task_steers(task)
            .unwrap()
            .iter()
            .any(|steer| steer.text.contains(text))
    };
    assert!(told(&pair.laptop, "claimed this Task first"));
    assert!(!told(&pair.mini, "claimed"));

    // A claim made after seeing mini's takes the Task over; mini keeps its
    // own claim open and names the later one.
    crate::store::ownership::claim_task(&pair.laptop.store, task)
        .unwrap()
        .unwrap();
    pair.to_mini();
    pair.to_laptop();
    let first = crate::store::ownership::task_ownership(&pair.mini.store, task).unwrap();
    assert_eq!(first.owner.unwrap().machine.id, Some(mini));
    assert_eq!(
        first.superseded_by.unwrap().machine.id,
        Some(laptop.clone())
    );
    assert!(told(&pair.mini, "Also claimed later by"));
    let second = crate::store::ownership::task_ownership(&pair.laptop.store, task).unwrap();
    assert_eq!(second.owner.unwrap().machine.id, Some(laptop));
    assert!(second.superseded_by.is_none());
    // Exchange settles: neither Machine re-claims a log that shows a later claim.
    pair.to_mini();
    pair.to_laptop();
    assert_eq!(
        crate::store::ownership::task_ownership(&pair.mini.store, task)
            .unwrap()
            .log
            .len(),
        3
    );
}

#[test]
fn the_owner_reclaims_after_a_stale_unclaim_elsewhere() {
    let pair = Pair::new();
    let task = &pair.task;
    pair.to_laptop();
    let mini = crate::store::machines::local_machine(&pair.mini.store)
        .unwrap()
        .id;
    crate::store::ownership::claim_task(&pair.laptop.store, task)
        .unwrap()
        .unwrap();
    pair.to_mini();
    // A stale reading on mini treats the Task as its own and unclaims it there,
    // delivering the unclaim as mini would: a marked comment on the Task.
    let later = time::OffsetDateTime::now_utc() + time::Duration::seconds(5);
    let entry = serde_json::json!({
        "id": format!("own_{}", uuid::Uuid::new_v4().simple()),
        "task_id": task.as_str(), "kind": "unclaim", "person": "Kim",
        "machine_id": mini.as_str(),
        "at": later.unix_timestamp(),
    });
    pair.mini
        .store
        .append_task_comment(
            task,
            &TaskComment {
                id: uuid::Uuid::new_v4().to_string(),
                body: format!("Kim unclaimed this\n\n<!-- loopflow-ownership:{entry} -->"),
                author: TaskCommentAuthor::Integration,
                created_at: Some(
                    later
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap(),
                ),
            },
        )
        .unwrap();
    pair.to_laptop();
    // The laptop's claim stays open locally and it claims again on the hub.
    let ownership = crate::store::ownership::task_ownership(&pair.laptop.store, task).unwrap();
    assert_eq!(
        ownership.owner.unwrap().machine.id,
        Some(
            crate::store::machines::local_machine(&pair.laptop.store)
                .unwrap()
                .id
        )
    );
    assert_eq!(ownership.log.last().unwrap().kind, OwnershipKind::Claim);
}

#[test]
fn another_persons_claim_never_routes_work_to_their_machine() {
    let pair = Pair::new();
    let task = &pair.task;
    let mini = crate::store::machines::local_machine(&pair.mini.store)
        .unwrap()
        .id;
    let laptop = crate::store::machines::local_machine(&pair.laptop.store)
        .unwrap()
        .id;
    // Kim claims on mini, a Machine this laptop also has registered. Her
    // person key, not the Machine, says whose claim it is.
    assert!(std::process::Command::new("git")
        .args(["config", "user.email", "kim@example.com"])
        .current_dir(pair.mini.repo.path())
        .status()
        .unwrap()
        .success());
    crate::store::ownership::claim_task(&pair.mini.store, task)
        .unwrap()
        .unwrap();
    pair.to_laptop();
    assert_eq!(
        crate::store::ownership::task_ownership(&pair.laptop.store, task)
            .unwrap()
            .owner
            .unwrap()
            .person_key
            .as_deref(),
        Some("kim@example.com")
    );
    // Someone else's claim stays theirs: no route to mini, no handoff or unassign.
    let route = crate::store::ownership::task_execution_route(&pair.laptop.store, task).unwrap();
    assert_eq!(
        (route.machine_id, route.source),
        (
            laptop.clone(),
            TaskExecutionSource::OwningMachine(OwningMachineSource::ThisMachine)
        )
    );
    assert!(crate::store::ownership::unassign_task(&pair.laptop.store, task).is_err());
    assert!(crate::store::ownership::move_claim(&pair.laptop.store, task, &mini).is_err());
    // Claiming writes here; the later claim wins.
    crate::store::ownership::claim_task(&pair.laptop.store, task)
        .unwrap()
        .unwrap();
    pair.to_mini();
    // Kim's claim stays Kim's on mini, which now sees the later claim.
    let later = crate::store::ownership::task_ownership(&pair.mini.store, task)
        .unwrap()
        .superseded_by
        .unwrap();
    assert_eq!(later.machine.id, Some(laptop));
}
