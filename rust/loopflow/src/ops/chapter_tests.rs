use std::collections::BTreeMap;
use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::{classify_task, TaskDisposition, TaskStartEvidence};
use crate::ops::pm::{pm_sync, resolve_context, PmSyncOptions, PmTestContext, PM_TEST_CONTEXT};
use crate::ops::NullProgress;
use crate::planning::{LinearIssueId, TaskPlan};
use crate::pm::{PmItem, PmProject, ProjectStatus};
use crate::store::{open_ephemeral_store, CredentialType, ProviderToken, StorageConfig};
use crate::work::task::{Observation, PmWritebackState, Task, TaskId, TaskPr, TaskPrId};
use crate::work::wave::{ensure_wave_row, WaveLocator};
use time::OffsetDateTime;

fn project(id: &str, name: &str, status: ProjectStatus) -> PmProject {
    PmProject {
        revision: None,
        id: id.into(),
        name: name.into(),
        slug: name.into(),
        summary: String::new(),
        workflow: "feature".into(),
        status,
        metric_targets: Vec::new(),
        krs: Vec::new(),
        initiative_ids: vec!["initiative".into()],
        team_ids: vec!["team-1".into()],
    }
}

fn task(state: &str) -> PmItem {
    PmItem {
        branch_name: None,
        revision: None,
        id: "issue".into(),
        identifier: "FIX-1".into(),
        url: None,
        name: "Work".into(),
        description: String::new(),
        rank: 0,
        completed: state == "completed",
        completed_at: None,
        state: Some(state.into()),
        project_id: Some("old".into()),
        project: Some("old".into()),
        team_id: Some("team-1".into()),
        assignee: None,
    }
}

#[test]
fn unreviewed_backlog_remains_while_started_work_moves() {
    let mut evidence = TaskStartEvidence {
        begun: false,
        authored: Some(false),
        published: false,
        abandoned: false,
        completed: false,
    };
    assert_eq!(
        classify_task(&task("unstarted"), &evidence).0,
        TaskDisposition::Historical
    );
    assert_eq!(
        classify_task(&task("started"), &evidence).0,
        TaskDisposition::Move
    );
    assert_eq!(
        classify_task(&task("completed"), &evidence).0,
        TaskDisposition::Historical
    );
    evidence.authored = None;
    assert_eq!(
        classify_task(&task("backlog"), &evidence).0,
        TaskDisposition::Unresolved
    );
    evidence.begun = true;
    assert_eq!(
        classify_task(&task("backlog"), &evidence).0,
        TaskDisposition::Move
    );
}

#[test]
fn unresolved_abandonment_requires_explicit_settlement() {
    let mut evidence = TaskStartEvidence {
        begun: false,
        authored: Some(false),
        published: false,
        abandoned: true,
        completed: false,
    };
    assert_eq!(
        classify_task(&task("unstarted"), &evidence).0,
        TaskDisposition::Unresolved
    );
    assert_eq!(
        classify_task(&task("canceled"), &evidence).0,
        TaskDisposition::Historical
    );
    evidence.begun = true;
    assert_eq!(
        classify_task(&task("unstarted"), &evidence).0,
        TaskDisposition::Unresolved
    );
    evidence.begun = false;
    evidence.authored = Some(true);
    assert_eq!(
        classify_task(&task("unstarted"), &evidence).0,
        TaskDisposition::Unresolved
    );
    evidence.abandoned = false;
    evidence.completed = true;
    assert_eq!(
        classify_task(&task("unstarted"), &evidence).0,
        TaskDisposition::Unresolved
    );
}

#[derive(Default)]
struct Provider {
    projects: BTreeMap<String, Value>,
    issues: BTreeMap<String, Value>,
    mutations: usize,
    revision: i64,
    interrupt_after: Option<usize>,
    unavailable: bool,
    interrupt_after_transfer_readback: bool,
    status_after_transfer: Option<(String, String)>,
    binding_collision: Option<std::path::PathBuf>,
    binding_replacement: Option<(std::path::PathBuf, String)>,
    inventory_pause: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
}

fn page(nodes: Vec<Value>) -> Value {
    json!({"nodes": nodes, "pageInfo": {"hasNextPage": false, "endCursor": null}})
}

async fn graphql(
    State(state): State<Arc<Mutex<Provider>>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let query = request["query"].as_str().unwrap();
    let vars = &request["variables"];
    let id = vars["id"].as_str().unwrap_or("");
    if query.contains("query FindProject") {
        let pause = state.lock().await.inventory_pause.take();
        if let Some((entered, release)) = pause {
            entered.notify_one();
            release.notified().await;
        }
    }
    let mut provider = state.lock().await;
    if provider.unavailable {
        return Json(json!({"errors":[{"message":"fixture interrupted connection"}]}));
    }
    for project in provider.projects.values_mut() {
        project
            .as_object_mut()
            .unwrap()
            .entry("updatedAt")
            .or_insert(json!("1970-01-01T00:00:00Z"));
    }
    for issue in provider.issues.values_mut() {
        issue
            .as_object_mut()
            .unwrap()
            .entry("updatedAt")
            .or_insert(json!("1970-01-01T00:00:00Z"));
        issue["branchName"] = Value::Null;
    }
    let prior_projects = provider.projects.clone();
    let prior_issues = provider.issues.clone();
    let data = if query.contains("query ListTeams") {
        json!({"teams":page(vec![json!({"id":"team-1", "name":"Fixture", "key":"FIX", "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"})])})
    } else if query.contains("query ProjectInitiative") {
        json!({"initiative":{"id":id}})
    } else if query.contains("query ListInitiatives") {
        json!({"initiatives":page(vec![
            json!({"id":"initiative-a","name":"A","description":""}),
            json!({"id":"initiative-b","name":"B","description":""}),
        ])})
    } else if query.contains("query ListInitiativeProjects") {
        let initiative = &vars["initiativeId"];
        json!({"initiative":{"projects":page(provider.projects.values().filter(|project|
            project["archivedAt"].is_null() && project["initiatives"]["nodes"].as_array().unwrap().iter().any(|node| &node["id"] == initiative)).cloned().collect())}})
    } else if query.contains("query ListProjectIssues") {
        json!({"project":{"issues":page(provider.issues.values().filter(|issue| issue["project"]["id"] == vars["projectId"]).cloned().collect())}})
    } else if query.contains("query FindProject") {
        json!({"projects":page(provider.projects.get(id).cloned().into_iter().collect())})
    } else if query.contains("query ProjectOwnership") {
        json!({"project":provider.projects.get(id)})
    } else if query.contains("query IssueOwnership") {
        let mut issue = provider.issues[id].clone();
        issue["project"] = provider.projects[issue["project"]["id"].as_str().unwrap()].clone();
        if provider.interrupt_after_transfer_readback
            && id == "a-started"
            && issue["project"]["id"] != "00000000-0000-4000-8000-000000000001"
        {
            provider.interrupt_after_transfer_readback = false;
            provider.unavailable = true;
        }
        if id == "a-started" && issue["project"]["id"] != old_id("a") {
            if let Some((project, status)) = provider.status_after_transfer.take() {
                provider.revision += 1;
                let revision = fixture_revision(provider.revision);
                let project = provider.projects.get_mut(&project).unwrap();
                project["status"]["type"] = json!(status);
                project["updatedAt"] = revision;
            }
        }
        json!({"issue":issue})
    } else if query.contains("query IssueTeam") {
        json!({"issue":{"team":{"id":"team-1"}}})
    } else if query.contains("query ProjectStatuses") {
        json!({"projectStatuses":page(["planned", "started", "completed"].into_iter().map(|kind|
            json!({"id":kind,"type":kind,"teamId":null,"position":0.0})).collect())})
    } else if query.contains("query CanceledWorkflowStates") {
        json!({"workflowStates":{"nodes":[{"id":"cancel-state","position":0.0}]}})
    } else if query.contains("mutation CreateProject") {
        let supplied = uuid::Uuid::parse_str(id).unwrap();
        if supplied.get_version_num() != 4 || supplied.get_variant() != uuid::Variant::RFC4122 {
            return Json(json!({"errors":[{"message":"supplied Project id must be UUID v4"}]}));
        }
        provider.projects.insert(id.into(), json!({"id":id,"name":vars["name"],"description":"", "archivedAt":null,
            "content":vars["content"],"status":{"type":vars["statusId"]},"teams":{"nodes":[{"id":"team-1"}]},"initiatives":{"nodes":[]}}));
        json!({"projectCreate":{"project":{"id":id}}})
    } else if query.contains("mutation AttachProject") {
        provider
            .projects
            .get_mut(vars["projectId"].as_str().unwrap())
            .unwrap()["initiatives"]["nodes"] = json!([{"id":vars["initiativeId"]}]);
        json!({"initiativeToProjectCreate":{"initiativeToProject":{"id":"link"}}})
    } else if query.contains("mutation AdoptProject") {
        let project = provider.projects.get_mut(id).unwrap();
        project["content"] = vars["input"]["content"].clone();
        if let Some(status) = vars["input"].get("statusId") {
            project["status"]["type"] = status.clone();
        }
        json!({"projectUpdate":{"success":true}})
    } else if query.contains("mutation RenameProject") {
        provider.projects.get_mut(id).unwrap()["name"] = vars["name"].clone();
        json!({"projectUpdate":{"success":true}})
    } else if query.contains("mutation ApplyProjectPlan") {
        provider.projects.get_mut(id).unwrap()["content"] = vars["content"].clone();
        json!({"projectUpdate":{"success":true}})
    } else if query.contains("mutation UpdateProject") {
        let project = provider.projects.get_mut(id).unwrap();
        for field in ["name", "description", "content"] {
            project[field] = vars[field].clone();
        }
        json!({"projectUpdate":{"success":true}})
    } else if query.contains("mutation SetProjectStatus") {
        provider.projects.get_mut(id).unwrap()["status"]["type"] = vars["statusId"].clone();
        json!({"projectUpdate":{"success":true}})
    } else if query.contains("mutation MoveIssueToProject") {
        let target = vars["projectId"].as_str().unwrap();
        let name = provider.projects[target]["name"].clone();
        provider.issues.get_mut(id).unwrap()["project"] = json!({"id":target,"name":name});
        json!({"issueUpdate":{"issue":{"id":id}}})
    } else if query.contains("mutation SetIssueState") {
        provider.issues.get_mut(id).unwrap()["state"]["type"] = json!("canceled");
        json!({"issueUpdate":{"issue":{"id":id}}})
    } else {
        panic!("unexpected fixture operation: {query}")
    };
    if query.contains("mutation SetProjectStatus") && vars["statusId"] == "started" {
        if let Some(path) = provider.binding_collision.take() {
            rusqlite::Connection::open(path).unwrap().execute_batch(
                "CREATE TRIGGER fail_selection BEFORE UPDATE OF current_project_id ON waves
                 WHEN NEW.current_project_id IS NOT NULL BEGIN SELECT RAISE(ABORT,'fixture selection failure'); END;"
            ).unwrap();
        }
    }
    if query.contains("mutation SetProjectStatus") && vars["statusId"] == "completed" {
        if let Some((path, project)) = provider.binding_replacement.take() {
            rusqlite::Connection::open(path).unwrap().execute(
                "UPDATE waves SET current_project_id=(SELECT id FROM projects WHERE external_project_id=?1 AND wave_id=waves.id) WHERE id=(SELECT wave_id FROM projects WHERE external_project_id=?1)", [&project]).unwrap();
        }
    }
    if query.starts_with("mutation") {
        provider.mutations += 1;
        provider.revision += 1;
        let revision = fixture_revision(provider.revision);
        for (id, project) in &mut provider.projects {
            if prior_projects.get(id) != Some(project) {
                project["updatedAt"] = revision.clone();
            }
        }
        for (id, issue) in &mut provider.issues {
            if prior_issues.get(id) != Some(issue) {
                issue["updatedAt"] = revision.clone();
            }
        }
        if provider.interrupt_after == Some(provider.mutations) {
            provider.unavailable = true;
            return Json(json!({"errors":[{"message":"fixture lost mutation response"}]}));
        }
    }
    Json(json!({"data":data}))
}

fn fixture_revision(revision: i64) -> Value {
    json!(
        (OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(revision))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    )
}

fn provider_fixture() -> Provider {
    let mut provider = Provider::default();
    for (wave, title) in [("a", "A"), ("b", "B")] {
        let id = old_id(wave).to_owned();
        let name = format!("{title} — previous");
        provider.projects.insert(id.clone(), json!({"id":id,"name":name,"description":"", "archivedAt":null,
            "content":"workflow: feature\n\n## KRs\n- [ ] Retain proof", "status":{"type":"started"},
            "teams":{"nodes":[{"id":"team-1"}]},"initiatives":{"nodes":[{"id":format!("initiative-{wave}")} ]}}));
        for (suffix, state) in [
            ("started", "started"),
            ("backlog", "unstarted"),
            ("done", "completed"),
        ] {
            let issue = format!("{wave}-{suffix}");
            provider.issues.insert(issue.clone(), json!({"id":issue,"identifier":format!("FIX-{issue}"),"url":null,
                "title":issue,"description":"","completedAt": null, "prioritySortOrder":0.0,"sortOrder":0.0,"assignee":null,
                "state":{"type":state},"project":{"id":id,"name":name},"team":{"id":"team-1"}}));
        }
    }
    provider
}

async fn context(path: &std::path::Path, repo: &std::path::Path, url: &str) -> PmTestContext {
    let store = Arc::new(
        open_ephemeral_store(&StorageConfig::sqlite(path.into()))
            .await
            .unwrap(),
    );
    for name in ["a", "b"] {
        ensure_wave_row(&store, repo, name).await.unwrap();
    }
    store
        .upsert_provider_token(&ProviderToken {
            provider: "linear".into(),
            access_token: "fixture".into(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: 1,
            credential_type: CredentialType::ApiKey,
        })
        .await
        .unwrap();
    PmTestContext {
        path: path.into(),
        store,
        graphql_url: url.into(),
    }
}

async fn seed_project(
    store: &crate::store::Store,
    wave: &crate::work::wave::Wave,
    plan: &PmProject,
) -> crate::work::project::Project {
    if let Some(existing) = store.get_project_by_project(&plan.id).await.unwrap() {
        return existing;
    }
    let now = OffsetDateTime::now_utc();
    let project = crate::work::project::Project {
        id: crate::work::project::ProjectId::new(),
        wave_id: wave.id().clone(),
        plan: crate::planning::ProjectPlan {
            summary: String::new(),
            linear_id: Some(crate::planning::LinearProjectId::new(plan.id.clone()).unwrap()),
            slug: plan.slug.clone(),
            name: plan.name.clone(),
            prompt_context: crate::pm::render_project_content(&crate::pm::ProjectContent {
                workflow: plan.workflow.clone(),
                krs: plan.krs.clone(),
                metric_targets: plan.metric_targets.clone(),
            }),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),
            workflow: plan.workflow.clone(),
            status: plan.status,
        },
        iteration: 0,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store.create_project(&project).await.unwrap();
    project
}

async fn local_task(
    context: &PmTestContext,
    repo: &std::path::Path,
    wave_name: &str,
    issue: &str,
    worktree: &std::path::Path,
    base: &str,
) -> (Task, TaskPr) {
    let wave = context
        .store
        .get_wave_at(&WaveLocator::discover(repo, wave_name).unwrap())
        .await
        .unwrap()
        .unwrap();
    let parent = seed_project(
        &context.store,
        &wave,
        &project(old_id(wave_name), "previous", ProjectStatus::Started),
    )
    .await;
    let now = OffsetDateTime::now_utc();
    let task = Task {
        id: TaskId::new(),
        plan: TaskPlan {
            revision: 0,
            linear_id: Some(LinearIssueId::new(issue).unwrap()),
            identifier: format!("FIX-{issue}"),
            title: "Retain execution".into(),
            description: String::new(),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: parent.id,
        worktree: Some(worktree.into()),
        workspace_slug: issue.into(),
        agent: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: Observation::default(),
    };
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: "retain".into(),
        branch: issue.into(),
        base_commit: base.into(),
        parent_pr_id: None,
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
        created_at: now,
        updated_at: now,
    };
    let selected = crate::store::sqlite::project_selection::read_project_binding(
        &context.store.sqlite,
        wave.id(),
    )
    .unwrap();
    crate::store::sqlite::project_selection::write_project_binding(
        &context.store.sqlite,
        wave.id(),
        selected.as_deref(),
        old_id(wave_name),
        &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
    )
    .unwrap();
    context.store.seed_task(&task, &pr).await.unwrap();
    (task, pr)
}

async fn local_started_task(
    context: &PmTestContext,
    repo: &std::path::Path,
) -> (Task, TaskPr, Vec<crate::ops::flow_process::FlowProcess>) {
    let (task, _) = local_task(
        context,
        repo,
        "a",
        "a-started",
        repo,
        "0000000000000000000000000000000000000000",
    )
    .await;
    assert!(!context.store.task_started(&task.id).await.unwrap());
    // A Flow process from the checkout is the Task's first recorded work.
    context.store.sqlite.test_flow(
        "captured",
        &repo.to_string_lossy(),
        &[("commit -m work", Some("succeeded"))],
        None,
    );
    context.store.sqlite.mark_task_started(&task.id).unwrap();
    assert!(context.store.task_started(&task.id).await.unwrap());
    let flows = context.store.sqlite.task_flows(&task.id).unwrap();
    assert_eq!(flows.len(), 1);
    let pr = context.store.task_prs(&task.id).await.unwrap().remove(0);
    // Compare persisted values: SQLite stores timestamps at second precision.
    let task = context.store.get_task(&task.id).await.unwrap().unwrap();
    (task, pr, flows)
}

fn clean_checkout(checkout: &std::path::Path) -> String {
    std::fs::create_dir_all(checkout).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "initial",
        ],
    ] {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(checkout)
            .status()
            .unwrap()
            .success());
    }
    crate::engine::git::rev_parse(checkout, "HEAD").unwrap()
}

async fn local_backlog_tasks(context: &PmTestContext, repo: &std::path::Path) {
    for wave in ["a", "b"] {
        let checkout = context.path.with_extension(format!("{wave}-checkout"));
        let base = clean_checkout(&checkout);
        local_task(
            context,
            repo,
            wave,
            &format!("{wave}-backlog"),
            &checkout,
            &base,
        )
        .await;
    }
}

fn fixture_repo(root: &std::path::Path) -> std::path::PathBuf {
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join(".lf")).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "remote",
            "add",
            "origin",
            "https://github.com/loopflowstudio/fixture.git",
        ],
    ] {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    }
    std::fs::write(
        repo.join(".lf/config.yaml"),
        "pm:\n  provider: linear\n  linear_team: team-1\n",
    )
    .unwrap();
    for wave in ["a", "b"] {
        std::fs::create_dir_all(repo.join(format!("wave/{wave}"))).unwrap();
        std::fs::write(
            repo.join(format!("wave/{wave}/GOAL.md")),
            format!("---\npm:\n  linear_initiative: initiative-{wave}\n---\nDurable mandate.\n"),
        )
        .unwrap();
    }
    std::fs::canonicalize(repo).unwrap()
}

async fn serve_fixture(provider: Arc<Mutex<Provider>>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().route("/", post(graphql)).with_state(provider);
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, server)
}

#[tokio::test]
async fn explicit_sync_converts_legacy_flow_without_renaming_or_losing_content() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let original = "Keep this prose.\n\n## Flows\nrecommended: custom\n\n## KRs\n- [ ] Keep this KR\n\n## Notes\nRetain this closing note.\n";
    let mut state = provider_fixture();
    let old = state
        .projects
        .get_mut("00000000-0000-4000-8000-000000000001")
        .unwrap();
    old["name"] = json!("previous");
    old["description"] = json!("Authored summary unrelated to the Flow or KRs.");
    old["content"] = json!(original);
    old["status"]["type"] = json!("planned");
    let mut expected = old.clone();
    expected["content"] = json!(original.replace("recommended:", "workflow:"));
    expected["status"]["type"] = json!("started");
    let provider = Arc::new(Mutex::new(state));
    let (url, server) = serve_fixture(provider.clone()).await;
    let home = context(&directory.path().join("sync.db"), &repo, &url).await;
    let (task, pr, flow) = local_started_task(&home, &repo).await;
    rusqlite::Connection::open(&home.path)
        .unwrap()
        .execute(
            "UPDATE projects SET legacy_current=1 WHERE external_project_id='00000000-0000-4000-8000-000000000001'",
            [],
        )
        .unwrap();
    let store = home.store.clone();
    // The public sync entry point owns a runtime; keep the fixture server running separately.
    tokio::task::spawn_blocking(move || {
        PM_TEST_CONTEXT.sync_scope(home, || {
            for _ in 0..2 {
                pm_sync(
                    &repo,
                    &PmSyncOptions {
                        wave: Some("a".into()),
                        plan: false,
                    },
                    &NullProgress,
                )
                .unwrap();
            }
        });
    })
    .await
    .unwrap();
    let mut observed =
        provider.lock().await.projects["00000000-0000-4000-8000-000000000001"].clone();
    assert_ne!(observed.as_object_mut().unwrap().remove("updatedAt"), None);
    assert_eq!(observed, expected);
    let snapshot = store.pm_snapshot(&task.wave_id).await.unwrap().unwrap();
    let snapshot = snapshot.snapshot;
    let synced = snapshot
        .projects
        .iter()
        .find(|project| project.id == "00000000-0000-4000-8000-000000000001")
        .unwrap();
    assert_eq!(synced.workflow, "custom");
    assert_eq!(synced.krs.len(), 1);
    assert_eq!(synced.krs[0].text, "Keep this KR");
    let retained = store.get_task(&task.id).await.unwrap().unwrap();
    assert_eq!(retained.project_id, task.project_id);
    assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
    assert_eq!(store.sqlite.task_flows(&task.id).unwrap(), flow);
    server.abort();
}

#[tokio::test]
async fn archived_predecessor_is_history_even_when_linear_still_says_started() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    {
        let mut state = provider.lock().await;
        let mut current = state.projects["00000000-0000-4000-8000-000000000001"].clone();
        current["id"] = json!("a-current");
        current["name"] = json!("A — current");
        state.projects.insert("a-current".into(), current);
        state
            .projects
            .get_mut("00000000-0000-4000-8000-000000000001")
            .unwrap()["archivedAt"] = json!("2026-09-01T00:00:00Z");
    }
    let (url, server) = serve_fixture(provider.clone()).await;
    let home = context(&directory.path().join("archive.db"), &repo, &url).await;
    let (task, pr, flow) = local_started_task(&home, &repo).await;
    PM_TEST_CONTEXT
        .scope(home, async {
            let ctx = resolve_context(&repo, "a").await.unwrap();
            let snapshot = crate::ops::pm::refresh_pm_snapshot(&repo, "a", &ctx)
                .await
                .unwrap();
            assert!(snapshot.projects.iter().any(
                |project| project.id == "a-current" && project.status == ProjectStatus::Started
            ));
            assert_eq!(
                snapshot
                    .projects
                    .iter()
                    .find(|project| project.id == "00000000-0000-4000-8000-000000000001")
                    .unwrap()
                    .status,
                ProjectStatus::Completed
            );
            assert!(snapshot.items.iter().any(|item| item.id == "a-started"
                && item.project_id.as_deref() == Some("00000000-0000-4000-8000-000000000001")));
            let store = super::pm_store().await.unwrap();
            let retained = store.get_task(&task.id).await.unwrap().unwrap();
            assert_eq!(retained.id, task.id);
            assert_eq!(retained.project_id, task.project_id);
            assert_eq!(retained.worktree, task.worktree);
            assert_eq!(retained.plan.linear_id, task.plan.linear_id);
            assert_eq!(retained.plan.title, "a-started");
            assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
            assert_eq!(store.sqlite.task_flows(&task.id).unwrap(), flow);
        })
        .await;
    assert_eq!(provider.lock().await.mutations, 0);
    server.abort();
}

async fn legacy_home(path: &std::path::Path, repo: &std::path::Path, url: &str) -> PmTestContext {
    let home = context(path, repo, url).await;
    local_started_task(&home, repo).await;
    local_backlog_tasks(&home, repo).await;
    let wave = home
        .store
        .get_wave_at(&WaveLocator::discover(repo, "a").unwrap())
        .await
        .unwrap()
        .unwrap();
    for (id, name) in [
        ("00000000-0000-4000-8000-000000000003", "next"),
        ("a-archived", "archive"),
    ] {
        seed_project(
            &home.store,
            &wave,
            &project(id, name, ProjectStatus::Planned),
        )
        .await;
    }
    // These are the migration's retained receipt identities, not provider status.
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute("UPDATE projects SET legacy_current=CASE external_project_id WHEN '00000000-0000-4000-8000-000000000001' THEN 1 ELSE 0 END WHERE wave_id=?1",
        [wave.id().as_str()]).unwrap();
    home
}

#[tokio::test]
async fn legacy_project_adoption_preserves_plans_across_lost_responses() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider.clone()).await;
    let original =
        "Keep this prose.\n\n## Flows\nrecommended: custom\n\n## KRs\n- [ ] Keep this KR\n";
    for status in ["backlog", "planned"] {
        for stop in 1..=3 {
            for second_home in [false, true] {
                {
                    let mut state = provider.lock().await;
                    *state = provider_fixture();
                    let old = state
                        .projects
                        .get_mut("00000000-0000-4000-8000-000000000001")
                        .unwrap();
                    old["status"]["type"] = json!(status);
                    old["content"] = json!(original);
                    for (id, name, archived, flow) in [
                        (
                            "00000000-0000-4000-8000-000000000003",
                            "next",
                            false,
                            "future-custom",
                        ),
                        ("a-archived", "archive", true, "historical-custom"),
                    ] {
                        let mut project =
                            state.projects["00000000-0000-4000-8000-000000000001"].clone();
                        project["id"] = json!(id);
                        project["name"] = json!(format!("A — {name}"));
                        project["status"]["type"] =
                            json!(if archived { "started" } else { "planned" });
                        project["content"] = json!(original.replace("custom", flow));
                        project["archivedAt"] = if archived {
                            json!("2026-09-01T00:00:00Z")
                        } else {
                            Value::Null
                        };
                        state.projects.insert(id.into(), project);
                    }
                    state.interrupt_after = Some(stop);
                }
                let first = legacy_home(
                    &directory
                        .path()
                        .join(format!("legacy-{status}-{stop}-{second_home}.db")),
                    &repo,
                    &url,
                )
                .await;
                PM_TEST_CONTEXT
                    .scope(
                        PmTestContext {
                            path: first.path.clone(),
                            store: first.store.clone(),
                            graphql_url: first.graphql_url.clone(),
                        },
                        async {
                            let ctx = resolve_context(&repo, "a").await.unwrap();
                            let plans = crate::ops::pm::checked_projects(&repo, &ctx, "a")
                                .await
                                .unwrap();
                            let current = plans
                                .iter()
                                .find(|p| p.id == "00000000-0000-4000-8000-000000000001")
                                .unwrap();
                            assert_eq!(
                                (&*current.id, &*current.workflow),
                                ("00000000-0000-4000-8000-000000000001", "custom")
                            );
                            assert_eq!(
                                plans
                                    .iter()
                                    .find(|p| p.id == "00000000-0000-4000-8000-000000000003")
                                    .unwrap()
                                    .status,
                                ProjectStatus::Planned
                            );
                            assert_eq!(provider.lock().await.mutations, 0);
                            assert!(super::adopt_legacy_projects(
                                &repo,
                                &first.store,
                                "a",
                                &ctx,
                                true
                            )
                            .await
                            .is_err());
                        },
                    )
                    .await;
                {
                    let mut state = provider.lock().await;
                    state.unavailable = false;
                    state.interrupt_after = None;
                }
                let resumed = if second_home {
                    legacy_home(
                        &directory.path().join(format!("resume-{status}-{stop}.db")),
                        &repo,
                        &url,
                    )
                    .await
                } else {
                    first
                };
                PM_TEST_CONTEXT
                    .scope(resumed, async {
                        let store = super::pm_store().await.unwrap();
                        let task = store.get_task_by_issue("a-started").await.unwrap().unwrap();
                        let prs = store.task_prs(&task.id).await.unwrap();
                        let flow = store.sqlite.task_flows(&task.id).unwrap();
                        let ctx = resolve_context(&repo, "a").await.unwrap();
                        super::adopt_legacy_projects(&repo, &store, "a", &ctx, true)
                            .await
                            .unwrap();
                        assert!(store
                            .projects_pending_adoption(&task.wave_id)
                            .await
                            .unwrap()
                            .is_empty());
                        let mutations = provider.lock().await.mutations;
                        super::adopt_legacy_projects(&repo, &store, "a", &ctx, true)
                            .await
                            .unwrap();
                        assert_eq!(provider.lock().await.mutations, mutations);
                        assert_eq!(store.get_task(&task.id).await.unwrap().unwrap(), task);
                        assert_eq!(store.task_prs(&task.id).await.unwrap(), prs);
                        assert_eq!(store.sqlite.task_flows(&task.id).unwrap(), flow);
                        {
                            let state = provider.lock().await;
                            assert_eq!(state.projects.len(), 4);
                            assert_eq!(state.issues.len(), 6);
                            assert_eq!(
                                state.projects["00000000-0000-4000-8000-000000000001"]["content"],
                                original.replace("recommended:", "workflow:")
                            );
                            assert_eq!(
                                state.projects["00000000-0000-4000-8000-000000000001"]["status"]
                                    ["type"],
                                "started"
                            );
                            assert_eq!(
                                state.projects["00000000-0000-4000-8000-000000000003"]["status"]
                                    ["type"],
                                "planned"
                            );
                            assert_eq!(
                                state.issues["a-started"]["project"]["id"],
                                "00000000-0000-4000-8000-000000000001"
                            );
                        }
                    })
                    .await;
            }
        }
    }
    server.abort();
}

#[tokio::test]
async fn legacy_adoption_leaves_foreign_team_projects_and_receipts_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    {
        let mut state = provider.lock().await;
        let old = state
            .projects
            .get_mut("00000000-0000-4000-8000-000000000001")
            .unwrap();
        old["status"]["type"] = json!("backlog");
        old["content"] = json!("## Flows\nrecommended: custom");
        for id in ["00000000-0000-4000-8000-000000000003", "a-archived"] {
            let mut foreign = state.projects["00000000-0000-4000-8000-000000000001"].clone();
            foreign["id"] = json!(id);
            foreign["teams"]["nodes"] = json!([{"id":"other-team"}]);
            foreign["status"]["type"] = json!("started");
            if id == "a-archived" {
                foreign["archivedAt"] = json!("2026-09-01T00:00:00Z");
            }
            state.projects.insert(id.into(), foreign);
        }
    }
    let (url, server) = serve_fixture(provider.clone()).await;
    let home = legacy_home(&directory.path().join("foreign.db"), &repo, &url).await;
    PM_TEST_CONTEXT
        .scope(home, async {
            let store = super::pm_store().await.unwrap();
            let ctx = resolve_context(&repo, "a").await.unwrap();
            let plans = crate::ops::pm::checked_projects(&repo, &ctx, "a")
                .await
                .unwrap();
            assert_eq!(plans.len(), 1);
            assert_eq!(
                plans
                    .iter()
                    .find(|p| p.id == "00000000-0000-4000-8000-000000000001")
                    .unwrap()
                    .id,
                "00000000-0000-4000-8000-000000000001"
            );
            let before = provider.lock().await.projects.clone();
            let converted = super::adopt_legacy_projects(&repo, &store, "a", &ctx, true)
                .await
                .unwrap();
            assert_eq!(converted.len(), 1);
            assert_eq!(converted[0].status, ProjectStatus::Started);
            let wave = store
                .get_wave_at(&WaveLocator::discover(&repo, "a").unwrap())
                .await
                .unwrap()
                .unwrap();
            let mut pending = store.projects_pending_adoption(wave.id()).await.unwrap();
            pending.sort();
            assert_eq!(
                pending,
                vec![
                    ("00000000-0000-4000-8000-000000000003".into(), 0),
                    ("a-archived".into(), 0)
                ]
            );
            let state = provider.lock().await;
            for id in ["00000000-0000-4000-8000-000000000003", "a-archived"] {
                assert_eq!(state.projects[id], before[id]);
            }
        })
        .await;
    server.abort();
}

#[tokio::test]
async fn legacy_adoption_without_a_receipt_never_guesses_between_plans() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    {
        let mut state = provider.lock().await;
        state
            .projects
            .get_mut("00000000-0000-4000-8000-000000000001")
            .unwrap()["status"]["type"] = json!("backlog");
        state
            .projects
            .get_mut("00000000-0000-4000-8000-000000000001")
            .unwrap()["content"] = json!("## Flows\nrecommended: custom");
    }
    let (url, server) = serve_fixture(provider.clone()).await;
    let home = context(&directory.path().join("unrecorded.db"), &repo, &url).await;
    local_started_task(&home, &repo).await;
    rusqlite::Connection::open(&home.path)
        .unwrap()
        .execute(
            "UPDATE projects SET legacy_current=-1 WHERE external_project_id='00000000-0000-4000-8000-000000000001'",
            [],
        )
        .unwrap();
    PM_TEST_CONTEXT
        .scope(home, async {
            let store = super::pm_store().await.unwrap();
            let ctx = resolve_context(&repo, "a").await.unwrap();
            let projects = super::adopt_legacy_projects(&repo, &store, "a", &ctx, false)
                .await
                .unwrap();
            assert_eq!(projects[0].status, ProjectStatus::Started);
            assert_eq!(projects[0].workflow, "custom");
            {
                let mut state = provider.lock().await;
                let mut future = state.projects["00000000-0000-4000-8000-000000000001"].clone();
                future["id"] = json!("a-future");
                future["name"] = json!("A — future");
                future["status"]["type"] = json!("planned");
                future["content"] = json!("flow: future-custom");
                state.projects.insert("a-future".into(), future);
            }
            assert!(super::adopt_legacy_projects(&repo, &store, "a", &ctx, true)
                .await
                .unwrap_err()
                .to_string()
                .contains("ambiguous"));
            assert_eq!(provider.lock().await.mutations, 0);
            provider.lock().await.projects.get_mut("a-future").unwrap()["status"]["type"] =
                json!("started");
            super::adopt_legacy_projects(&repo, &store, "a", &ctx, true)
                .await
                .unwrap();
            assert_eq!(
                provider.lock().await.projects["00000000-0000-4000-8000-000000000001"]["status"]
                    ["type"],
                "backlog"
            );
        })
        .await;
    server.abort();
}

fn old_id(wave: &str) -> &'static str {
    match wave {
        "a" => "00000000-0000-4000-8000-000000000001",
        "b" => "00000000-0000-4000-8000-000000000002",
        _ => panic!("fixture Wave"),
    }
}
