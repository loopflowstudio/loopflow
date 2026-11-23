use loopflow_model::durable::{MachineId, TaskId};

#[test]
fn machine_defaults_route_unowned_tasks_until_one_is_claimed() {
    use crate::durable::{OwningMachineSource, TaskExecutionSource};
    use loopflow_model::durable::{DefaultScope, MachineSetting};
    let _env = crate::journal::test_env_lock();
    let home = tempfile::tempdir().unwrap();
    let _home =
        crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
    let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
    let repo = home.path().canonicalize().unwrap();
    let repo = repo.to_str().unwrap();
    let (mini, worker) = (MachineId::new(), MachineId::new());
    let project = crate::test_support::seed_wave_project(&store, repo, "work").unwrap();
    let wave = project.wave_id.clone();
    let repository = crate::repository::location::ensure_repository(&store, repo).unwrap();
    let new_task = |title: &str| {
        store
            .create_task(&loopflow_storage::work::task::NewTask {
                parent_task_id: None,
                repository_id: repository.clone(),
                id: TaskId::new(),
                project_id: Some(project.id.clone()),
                title: title.into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap()
    };
    // Unset: this Machine takes the Task, and the claim is captured.
    let local = crate::store::machines::local_machine(&store).unwrap().id;
    let claimed = new_task("Claimed here");
    crate::store::ownership::claim_task(&store, &claimed.id)
        .unwrap()
        .unwrap();

    crate::store::placement::set_machine_default(
        &store,
        &DefaultScope::Repository(repository.clone()),
        MachineSetting::Tasks,
        &worker,
    )
    .unwrap();
    let task = new_task("Routed");
    let route = crate::store::ownership::task_execution_route(&store, &task.id).unwrap();
    assert_eq!(route.machine_id, worker);
    assert!(
        matches!(route.source, TaskExecutionSource::OwningMachine(OwningMachineSource::Preference(selected)) if selected.scope == crate::config::settings::SettingScope::Repository)
    );
    // The Wave's Session Machine places no Task.
    crate::store::placement::set_machine_default(
        &store,
        &DefaultScope::Wave(wave.clone()),
        MachineSetting::Session,
        &local,
    )
    .unwrap();
    assert_eq!(
        crate::store::ownership::task_execution_route(&store, &task.id)
            .unwrap()
            .machine_id,
        worker
    );
    assert_eq!(
        crate::store::placement::wave_owning_machine(&store, &wave, MachineSetting::Session)
            .unwrap()
            .0,
        local.clone()
    );
    // The Wave's default outranks the repository's.
    crate::store::placement::set_machine_default(
        &store,
        &DefaultScope::Wave(wave.clone()),
        MachineSetting::Tasks,
        &mini,
    )
    .unwrap();
    let route = crate::store::ownership::task_execution_route(&store, &task.id).unwrap();
    assert_eq!(route.machine_id, mini);
    assert!(
        matches!(route.source, TaskExecutionSource::OwningMachine(OwningMachineSource::Preference(selected)) if selected.scope == crate::config::settings::SettingScope::Wave)
    );
    // Only your Machine for the Wave claims; this one refuses without writing.
    assert!(crate::store::ownership::claim_task(&store, &task.id).is_err());
    assert!(crate::store::ownership::task_ownership(&store, &task.id)
        .unwrap()
        .log
        .is_empty());
    // Captured, not inherited: a later default moves no claimed Task.
    let route = crate::store::ownership::task_execution_route(&store, &claimed.id).unwrap();
    assert_eq!(
        (route.machine_id, route.source),
        (local, TaskExecutionSource::Owner)
    );
    assert_eq!(
        crate::store::placement::machine_defaults(&store)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn a_waves_capacity_takes_its_new_tasks_until_a_nearer_machine_does() {
    use loopflow_model::durable::{DefaultScope, MachineSetting};
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
    let repo = home.path().canonicalize().unwrap();
    let repo = repo.to_str().unwrap();
    let project = crate::test_support::seed_wave_project(&store, repo, "infrastructure").unwrap();
    let wave = DefaultScope::Wave(project.wave_id.clone());
    let repository = crate::repository::location::ensure_repository(&store, repo).unwrap();
    let task = store
        .create_task(&loopflow_storage::work::task::NewTask {
            parent_task_id: None,
            repository_id: repository.clone(),
            id: TaskId::new(),
            project_id: Some(project.id.clone()),
            title: "Rented".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    let worker = MachineId::new();
    crate::store::placement::set_machine_default(
        &store,
        &DefaultScope::Repository(repository.clone()),
        MachineSetting::Tasks,
        &worker,
    )
    .unwrap();
    assert_eq!(
        crate::store::placement::task_capacity(&store, &task.id).unwrap(),
        None
    );

    // The Wave is nearer than the repository: its Tasks rent, and until
    // `lf task run` places one it stays on this Machine.
    crate::store::placement::set_tasks_capacity(&store, &wave, "code").unwrap();
    assert_eq!(
        crate::store::placement::task_capacity(&store, &task.id)
            .unwrap()
            .as_deref(),
        Some("code")
    );
    assert_eq!(
        crate::store::placement::wave_owning_machine(
            &store,
            &project.wave_id,
            MachineSetting::Tasks
        )
        .unwrap()
        .0,
        crate::store::machines::local_machine(&store).unwrap().id
    );
    assert_eq!(
        crate::store::placement::capacity_defaults(&store)
            .unwrap()
            .len(),
        1
    );

    // A Machine at the same scope replaces it.
    crate::store::placement::set_machine_default(&store, &wave, MachineSetting::Tasks, &worker)
        .unwrap();
    assert_eq!(
        crate::store::placement::task_capacity(&store, &task.id).unwrap(),
        None
    );
    assert!(crate::store::placement::capacity_defaults(&store)
        .unwrap()
        .is_empty());
}

#[test]
fn repository_plans_are_explicit_and_do_not_depend_on_clone_names() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let first = crate::store::opening::open_ephemeral(&a.path().join("db")).unwrap();
    let second = crate::store::opening::open_ephemeral(&b.path().join("db")).unwrap();
    let id = crate::repository::location::ensure_repository(&first, "/src/same").unwrap();
    assert_eq!(
        crate::repository::location::ensure_repository(&first, "/src/same").unwrap(),
        id
    );
    let other = crate::repository::location::ensure_repository(&second, "/src/same").unwrap();
    assert_ne!(id, other);
    crate::repository::location::bind_repository(&second, "/src/same", &id).unwrap();
    assert_eq!(second.repository_id("/src/same").unwrap(), Some(id.clone()));
    assert_eq!(
        second.repository_path(&other).unwrap().as_deref(),
        Some("/src/same")
    );
    assert_eq!(
        crate::repository::location::ensure_repository(&second, "/src/same").unwrap(),
        id
    );
    crate::repository::location::bind_repository(&second, "/src/same", &id).unwrap();
    assert_eq!(
        second.repository_path(&id).unwrap().as_deref(),
        Some("/src/same")
    );
    assert_eq!(
        first.repository_path(&id).unwrap().as_deref(),
        Some("/src/same")
    );
    let unrelated =
        crate::repository::location::ensure_repository(&second, "/another/clone").unwrap();
    assert!(crate::repository::location::bind_repository(&second, "/another/clone", &id).is_err());
    assert_eq!(
        second.repository_id("/another/clone").unwrap(),
        Some(unrelated)
    );
    crate::repository::location::bind_repository(&second, "/src/same", &other).unwrap();
    assert_eq!(second.repository_id("/src/same").unwrap(), Some(other));
    assert_eq!(
        second.repository_path(&id).unwrap().as_deref(),
        Some("/src/same")
    );
    assert!(first.repository_id("/unknown").unwrap().is_none());
}
