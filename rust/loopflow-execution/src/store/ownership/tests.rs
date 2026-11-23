use super::{acquire, claim_task, held_planning, task_execution_route, task_ownership};
use crate::store::placement::{set_machine_default, wave_owning_machine};
use loopflow_model::durable::{DefaultScope, MachineId, MachineSetting, TaskId};
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::NewTask;
use loopflow_test_support::TestRepo;
use rusqlite::params;

fn store() -> (tempfile::TempDir, SqliteStore) {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
    (home, store)
}

fn git(repo: &std::path::Path, args: &[&str]) {
    let result = std::process::Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn input(store: &SqliteStore, repo: &std::path::Path) -> NewTask {
    NewTask {
        id: TaskId::new(),
        repository_id: crate::repository::location::ensure_repository(
            store,
            crate::repository::discover(repo)
                .unwrap()
                .as_path()
                .to_str()
                .unwrap(),
        )
        .unwrap(),
        project_id: None,
        title: "Scope race".into(),
        description: String::new(),
        due_date: None,
    }
}

#[test]
fn changed_repository_rejects_stale_claim_and_uses_its_git_identity() {
    let (_home, store) = store();
    let before = TestRepo::new();
    let after = TestRepo::new();
    git(
        before.path(),
        &["config", "user.email", "before@example.test"],
    );
    git(
        after.path(),
        &["config", "user.email", "after@example.test"],
    );
    let task = store.create_task(&input(&store, before.path())).unwrap();
    let stale = acquire(&store, store.task_scope(&task.id).unwrap()).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE repository_plans SET repo=?2 WHERE id=?1",
            params![task.repository_id.as_str(), after.path().to_str().unwrap()],
        )
        .unwrap();
    assert_eq!(store.claim_task_prepared(&task.id, &stale).unwrap(), None);
    assert!(task_ownership(&store, &task.id).unwrap().log.is_empty());
    let claim = claim_task(&store, &task.id).unwrap().unwrap();
    assert_eq!(claim.person_key.as_deref(), Some("after@example.test"));
}

#[test]
fn new_task_reacquires_placement_after_wave_ancestry_changes() {
    let (_home, store) = store();
    let repo = TestRepo::new();
    let project = crate::work::wave::bootstrap::ensure_wave_project(
        &store,
        repo.path().to_str().unwrap(),
        "child",
    )
    .unwrap();
    let old_parent =
        crate::work::wave::bootstrap::ensure_wave(&store, repo.path().to_str().unwrap(), "old")
            .unwrap();
    let new_parent =
        crate::work::wave::bootstrap::ensure_wave(&store, repo.path().to_str().unwrap(), "new")
            .unwrap();
    let local = store.saved_local_machine().unwrap().id;
    set_machine_default(
        &store,
        &DefaultScope::Wave(old_parent.clone()),
        MachineSetting::Tasks,
        &MachineId::new(),
    )
    .unwrap();
    set_machine_default(
        &store,
        &DefaultScope::Wave(new_parent.clone()),
        MachineSetting::Tasks,
        &local,
    )
    .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE waves SET parent_wave_id=?2 WHERE id=?1",
            params![project.wave_id.as_str(), old_parent.as_str()],
        )
        .unwrap();
    let mut creation = input(&store, repo.path());
    creation.project_id = Some(project.id);
    let stale = acquire(
        &store,
        store
            .new_task_scope(&creation.repository_id, creation.project_id.as_ref())
            .unwrap(),
    )
    .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE waves SET parent_wave_id=?2 WHERE id=?1",
            params![project.wave_id.as_str(), new_parent.as_str()],
        )
        .unwrap();
    assert!(store
        .promote_task_prepared(
            &creation.id,
            Some(&creation),
            None,
            None,
            None,
            None,
            &stale,
            &loopflow_storage::sqlite::SessionCaller {
                agent: None,
                capture_key: None
            },
        )
        .unwrap()
        .is_none());
    assert!(store.task(&creation.id).unwrap().is_none());
    super::promote_task(
        &store,
        &creation.id,
        Some(&creation),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let claim = claim_task(&store, &creation.id).unwrap().unwrap();
    assert_eq!(claim.machine_id, Some(local));
}

#[test]
fn existing_claim_rejects_a_changed_project_scope() {
    let (_home, store) = store();
    let repo = TestRepo::new();
    let old = crate::work::wave::bootstrap::ensure_wave_project(
        &store,
        repo.path().to_str().unwrap(),
        "old",
    )
    .unwrap();
    let new = crate::work::wave::bootstrap::ensure_wave_project(
        &store,
        repo.path().to_str().unwrap(),
        "new",
    )
    .unwrap();
    let mut creation = input(&store, repo.path());
    creation.project_id = Some(old.id);
    let task = store.create_task(&creation).unwrap();
    let stale = acquire(&store, store.task_scope(&task.id).unwrap()).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET project_id=?2 WHERE id=?1",
            params![task.id.as_str(), new.id.as_str()],
        )
        .unwrap();
    assert_eq!(store.claim_task_prepared(&task.id, &stale).unwrap(), None);
    assert!(claim_task(&store, &task.id).unwrap().is_some());
}

#[test]
fn hand_back_rejects_a_changed_repository_with_the_same_held_set() {
    let (_home, store) = store();
    let repo = TestRepo::new();
    let other = TestRepo::new();
    let task = store.create_task(&input(&store, repo.path())).unwrap();
    let remote = MachineId::new();
    crate::store::machines::add_machine(&store, &remote, "mini", "mini").unwrap();
    claim_task(&store, &task.id).unwrap();
    super::move_claim(&store, &task.id, &remote).unwrap();
    use loopflow_storage::rental::{Hardware, OnDemand, Provider, RentalState, Usd};
    crate::store::rentals::add_ondemand(
        &store,
        &OnDemand {
            name: "code".into(),
            provider: Provider::Daytona,
            hardware: Hardware {
                vcpus: 2,
                memory_gb: 4,
                disk_gb: 20,
                gpu: None,
                spot: false,
            },
            max_machines: 1,
            max_usd_per_hour: Usd::parse("1.00").unwrap(),
            hours: 1.0,
        },
    )
    .unwrap();
    let rental = crate::store::rentals::admit_rental(&store, "code", None, None, false).unwrap();
    store
        .confirm_rental(&rental.id, "remote", Usd::ZERO)
        .unwrap();
    store.attach_rental(&rental.id, &remote).unwrap();
    let prepared = held_planning(&store, &remote).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE repository_plans SET repo=?2 WHERE id=?1",
            params![task.repository_id.as_str(), other.path().to_str().unwrap()],
        )
        .unwrap();
    assert!(store
        .set_rental_state_prepared(&rental.id, RentalState::Ended, Some(&prepared))
        .unwrap()
        .is_none());
    assert_eq!(
        store.rental(&rental.id).unwrap().state,
        RentalState::Running
    );
    assert_eq!(
        task_ownership(&store, &task.id)
            .unwrap()
            .owner
            .unwrap()
            .machine
            .id,
        Some(remote)
    );
    let (_, returned) = super::set_rental_state(&store, &rental.id, RentalState::Ended).unwrap();
    assert_eq!(returned, vec![task.id.clone()]);
    assert_eq!(
        task_execution_route(&store, &task.id).unwrap().machine_id,
        store.saved_local_machine().unwrap().id
    );
}

#[test]
fn linked_checkout_resolves_the_same_private_placement_file() {
    let (home, store) = store();
    let repo = TestRepo::new();
    let checkout = home.path().join("linked");
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            checkout.to_str().unwrap(),
        ],
    );
    let project = crate::work::wave::bootstrap::ensure_wave_project(
        &store,
        repo.path().to_str().unwrap(),
        "work",
    )
    .unwrap();
    let target = MachineId::new();
    set_machine_default(
        &store,
        &DefaultScope::Wave(project.wave_id.clone()),
        MachineSetting::Tasks,
        &target,
    )
    .unwrap();
    let mut scope = store.placement_scope(Some(&project.wave_id), None).unwrap();
    for address in &mut scope.chain {
        address.repo = checkout.to_str().unwrap().to_string();
    }
    assert_eq!(
        crate::store::placement::owning_machine(
            &store.home_dir().unwrap(),
            &scope,
            MachineSetting::Tasks
        )
        .unwrap()
        .0,
        target
    );
    assert_eq!(
        wave_owning_machine(&store, &project.wave_id, MachineSetting::Tasks)
            .unwrap()
            .0,
        target
    );
    assert!(!checkout.join(crate::config::files::PRIVATE_FILE).exists());
}

#[test]
fn held_scope_batches_ten_thousand_tasks_and_keys_repository_joins() {
    let (_home, store) = store();
    let repo = TestRepo::new();
    let task = store.create_task(&input(&store, repo.path())).unwrap();
    claim_task(&store, &task.id).unwrap();
    let local = store.saved_local_machine().unwrap().id;
    let before = loopflow_storage::query_plans::statements(&store, || {
        assert_eq!(
            held_planning(&store, &local).unwrap().0.repositories.len(),
            1
        );
    });
    {
        let mut conn = store.conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute(
            "WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<10000)
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
             SELECT printf('task_%032x',i),?1,printf('BATCH-%d',i),1 FROM n",
            [task.repository_id.as_str()],
        )
        .unwrap();
        tx.execute(
            "WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<10000)
             INSERT INTO task_ownership(id,task_id,kind,machine_id,at,ordered_at,writer_machine_id,written_here)
             SELECT printf('own_%032x',i),printf('task_%032x',i),'claim',?1,1,1,?1,1 FROM n",
            [local.as_str()],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let after = loopflow_storage::query_plans::statements(&store, || {
        assert_eq!(
            held_planning(&store, &local).unwrap().0.repositories.len(),
            10001
        );
    });
    assert_eq!(before.len(), after.len());
    let repository_joins: Vec<_> = after
        .into_iter()
        .filter(|sql| sql.contains("SELECT t.id,t.repository_id,r.repo"))
        .collect();
    assert_eq!(repository_joins.len(), 1);
    assert!(loopflow_storage::query_plans::growing_scans(&store, &repository_joins).is_empty());
}

#[test]
fn promotion_refuses_a_replaced_caller_for_either_title_source() {
    use crate::process::AgentCaller;
    use crate::session::TitleSource;
    use crate::store::StoreError;
    use loopflow_storage::sqlite::SessionCaller;

    let (_home, store) = store();
    let repo = TestRepo::new();
    let absent = SessionCaller {
        agent: None,
        capture_key: None,
    };
    for source in [TitleSource::Generated, TitleSource::Human] {
        let id = format!("promotion-{source:?}");
        let session = store.test_session(&id, &crate::session_record::new_artifact_key());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET title_source=?2 WHERE id=?1",
                params![
                    id,
                    if source == TitleSource::Generated {
                        "generated"
                    } else {
                        "human"
                    }
                ],
            )
            .unwrap();
        store
            .rename_session_prepared(&id, "Original", source, &absent)
            .unwrap();
        let process = store.test_lf_process();
        let first = store
            .attach_session(&id, None, &process, None, &absent)
            .unwrap();
        let caller = SessionCaller {
            agent: Some(AgentCaller {
                session_id: id.clone().into(),
                agent_process_id: Some(first.agent_process_id.clone()),
                origin_lf_process_id: process,
                trace_id: None,
            }),
            capture_key: session.artifact_key.clone(),
        };
        let creation = input(&store, repo.path());
        let facts = acquire(
            &store,
            store.new_task_scope(&creation.repository_id, None).unwrap(),
        )
        .unwrap();
        let replacement = store
            .attach_session(&id, Some(&first), &store.test_lf_process(), None, &absent)
            .unwrap();
        let before = store.session(&id).unwrap().unwrap();
        assert_eq!(before.title_source, source);
        let history = store.session_history(&id, 0, 0).unwrap();
        let result = store.promote_task_prepared(
            &creation.id,
            Some(&creation),
            None,
            Some(&before),
            None,
            None,
            &facts,
            &caller,
        );
        assert!(
            matches!(result, Err(StoreError::InvalidAuthority(ref message)) if message == "Session provider was replaced"),
            "{result:?}"
        );
        assert!(store.task(&creation.id).unwrap().is_none());
        assert_eq!(store.session(&id).unwrap(), Some(before));
        assert_eq!(store.session_history(&id, 0, 0).unwrap(), history);
        assert_eq!(store.session_attachment(&id).unwrap(), Some(replacement));
    }
}
