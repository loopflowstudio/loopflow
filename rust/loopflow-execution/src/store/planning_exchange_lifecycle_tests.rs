use loopflow_model::durable::TaskId;
use loopflow_storage::planning_destination::PlanningDestination;
use loopflow_storage::work::task::NewTask;

#[test]
fn standalone_task_exchange_preserves_membership_comments_and_ownership() {
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let a = crate::store::opening::open_ephemeral(&left.path().join("db")).unwrap();
    let b = crate::store::opening::open_ephemeral(&right.path().join("db")).unwrap();
    let repo = tempfile::tempdir().unwrap();
    let canonical = repo.path().canonicalize().unwrap();
    let path = canonical.to_str().unwrap();
    let route =
        PlanningDestination::new("/synthetic.git", "refs/loopflow/planning/shared/test").unwrap();
    a.bind_git_destination(path, &route).unwrap();
    b.bind_git_destination(path, &route).unwrap();
    let task = a
        .create_task(&NewTask {
            parent_task_id: None,
            id: TaskId::new(),
            repository_id: crate::repository::location::ensure_repository(&a, path).unwrap(),
            project_id: None,
            title: "Before filing".into(),
            description: "Keep the same Work".into(),
            due_date: None,
        })
        .unwrap();
    a.append_task_comment(
        &task.id,
        &loopflow_model::comment::TaskComment {
            id: "standalone-comment".into(),
            body: "Retain direction".into(),
            author: loopflow_model::comment::TaskCommentAuthor::Integration,
            created_at: Some("2026-10-17T00:00:00Z".into()),
        },
    )
    .unwrap();
    let claim = crate::store::ownership::claim_task(&a, &task.id)
        .unwrap()
        .unwrap();
    let journal = a.export_git_plan(path, &route.id()).unwrap();
    crate::store::git_tracker::import_git_plan(&b, path, &route.id(), "first", &journal).unwrap();
    let imported = b.task(&task.id).unwrap().unwrap();
    assert!(imported.project_id.is_none());
    assert_ne!(imported.repository_id, task.repository_id);
    assert_eq!(
        b.repository_path(&imported.repository_id)
            .unwrap()
            .as_deref(),
        Some(path)
    );
    // The claim travels as a marked comment; the other Machine follows it.
    let ownership = crate::store::ownership::task_ownership(&b, &task.id).unwrap();
    assert_eq!(ownership.log, vec![claim.clone()]);
    assert_eq!(
        ownership.owner.unwrap().machine.id,
        Some(crate::store::machines::local_machine(&a).unwrap().id)
    );
    let project = crate::work::wave::bootstrap::ensure_wave_project(&b, path, "product").unwrap();
    b.move_task(&task.id, None, Some(&project.id)).unwrap();
    let journal = b.export_git_plan(path, &route.id()).unwrap();
    crate::store::git_tracker::import_git_plan(&a, path, &route.id(), "attached", &journal)
        .unwrap();
    assert_eq!(
        a.task(&task.id).unwrap().unwrap().project_id,
        Some(project.id.clone())
    );
    a.move_task(&task.id, Some(&project.id), None).unwrap();
    a.delete_task(&task.id).unwrap();
    let journal = a.export_git_plan(path, &route.id()).unwrap();
    crate::store::git_tracker::import_git_plan(&b, path, &route.id(), "detached", &journal)
        .unwrap();
    assert!(b.task(&task.id).unwrap().unwrap().project_id.is_none());
    assert!(b.list_tasks(None).unwrap().is_empty());
    let conn = b.conn.lock().unwrap();
    let body: String = conn
        .query_row(
            "SELECT body FROM task_comments WHERE id='standalone-comment'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(body, "Retain direction");
}
