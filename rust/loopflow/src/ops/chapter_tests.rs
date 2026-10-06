use std::collections::BTreeMap;
use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::{
    classify_task, plan_rotation, rotate, successor_id, TaskDisposition, TaskStartEvidence,
};
use crate::durable::FlowSession;
use crate::engine::invocation::QueuedInvocation;
use crate::engine::{ConcreteSkill, ConcreteStep, ExecutionCursor, Skill};
use crate::ops::pm::{pm_sync, PmSyncOptions, PmTestContext, PM_TEST_CONTEXT};
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
        flow: "feature".into(),
        status,
        metric_targets: Vec::new(),
        krs: Vec::new(),
        initiative_ids: vec!["initiative".into()],
        team_ids: vec!["team-1".into()],
    }
}

#[test]
fn partial_status_flips_converge_without_selecting_a_newest_name() {
    use ProjectStatus::{Completed, Planned, Started};
    for (old_status, next_status) in [
        (Started, Planned),
        (Started, Started),
        (Completed, Planned),
        (Completed, Started),
    ] {
        let inventory = vec![
            (
                "a".into(),
                "initiative-a".into(),
                vec![project("a-old", "2026-09", Started)],
            ),
            (
                "b".into(),
                "initiative-b".into(),
                vec![
                    project("b-old", "2026-09", old_status),
                    project("b-new", "2026-10", next_status),
                ],
            ),
        ];
        let plan = plan_rotation("2026-10", &inventory).unwrap();
        assert_eq!(plan.waves[1].successor_id, "b-new");
        assert_eq!(plan.waves[0].predecessor.as_ref().unwrap().id, "a-old");
    }
    let conflicted = vec![
        (
            "a".into(),
            "initiative-a".into(),
            vec![project("one", "2026-09", Started)],
        ),
        (
            "b".into(),
            "initiative-b".into(),
            vec![project("two", "2027-01", Started)],
        ),
    ];
    assert!(plan_rotation("2026-10", &conflicted)
        .unwrap_err()
        .to_string()
        .contains("competing"));
}

#[test]
fn a_new_wave_can_join_with_an_empty_project_or_its_authored_plan() {
    let empty = vec![("new".into(), "initiative".into(), Vec::new())];
    let plan = plan_rotation("next", &empty).unwrap();
    assert!(plan.waves[0].predecessor.is_none());
    let planned = vec![(
        "new".into(),
        "initiative".into(),
        vec![project("authored", "next", ProjectStatus::Planned)],
    )];
    assert_eq!(
        plan_rotation("next", &planned).unwrap().waves[0].successor_id,
        "authored"
    );
}

#[test]
fn zero_current_without_shared_predecessor_evidence_stays_unresolved() {
    let inventory = vec![(
        "a".into(),
        "initiative-a".into(),
        vec![
            project("historic", "2026-01", ProjectStatus::Completed),
            project("next", "2026-10", ProjectStatus::Planned),
        ],
    )];
    assert!(plan_rotation("2026-10", &inventory)
        .unwrap_err()
        .to_string()
        .contains("unambiguous"));
}

#[test]
fn lost_creation_has_one_identity_on_every_home() {
    let parsed = uuid::Uuid::parse_str(&successor_id("initiative", "next")).unwrap();
    assert_eq!(parsed.get_version_num(), 4);
    assert_eq!(parsed.get_variant(), uuid::Variant::RFC4122);
    assert_eq!(
        successor_id("initiative-a", "next"),
        successor_id("initiative-a", "next")
    );
    assert_ne!(
        successor_id("initiative-a", "next"),
        successor_id("initiative-b", "next")
    );
    let duplicate = vec![(
        "a".into(),
        "initiative-a".into(),
        vec![
            project("one", "next", ProjectStatus::Planned),
            project("two", "next", ProjectStatus::Started),
        ],
    )];
    assert!(plan_rotation("next", &duplicate).is_err());
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
        team_id: "team-1".into(),
        assignee: None,
    }
}

#[test]
fn unreviewed_backlog_remains_while_started_work_moves() {
    let mut evidence = TaskStartEvidence {
        begun: false,
        worker_claimed: false,
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
    evidence.worker_claimed = true;
    assert_eq!(
        classify_task(&task("canceled"), &evidence).0,
        TaskDisposition::Unresolved
    );
}

#[test]
fn unresolved_abandonment_requires_explicit_settlement() {
    let mut evidence = TaskStartEvidence {
        begun: false,
        worker_claimed: false,
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
    evidence.worker_claimed = true;
    assert_eq!(
        classify_task(&task("unstarted"), &evidence).0,
        TaskDisposition::Unresolved
    );
    evidence.worker_claimed = false;
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
    status_change_at_final_inventory: Option<(String, String)>,
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
    if query.contains("query ListInitiativeProjects") {
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
    // Apply an external status change only once the predecessor's dispositions finish.
    if provider.status_change_at_final_inventory.is_some()
        && vars["initiativeId"] == "initiative-a"
        && provider.issues["a-started"]["project"]["id"] != "a-old"
    {
        if let Some((id, status)) = provider.status_change_at_final_inventory.take() {
            provider.revision += 1;
            let revision = fixture_revision(provider.revision);
            let project = provider.projects.get_mut(&id).unwrap();
            project["status"]["type"] = json!(status);
            project["updatedAt"] = revision;
        }
    }
    let data = if query.contains("query ListTeams") {
        json!({"teams":page(vec![json!({"id":"team-1", "name":"Fixture", "key":"FIX", "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"})])})
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
            && issue["project"]["id"] != "a-old"
        {
            provider.interrupt_after_transfer_readback = false;
            provider.unavailable = true;
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
        let id = format!("{wave}-old");
        let name = format!("{title} — previous");
        provider.projects.insert(id.clone(), json!({"id":id,"name":name,"description":"", "archivedAt":null,
            "content":"flow: feature\n\n## KRs\n- [ ] Retain proof", "status":{"type":"started"},
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
    let now = OffsetDateTime::now_utc();
    let project = crate::work::project::Project {
        id: crate::work::project::ProjectId::new(),
        wave_id: wave.id().clone(),
        plan: crate::planning::ProjectPlan {
            id: crate::planning::LinearProjectId::new(plan.id.clone()).unwrap(),
            slug: plan.slug.clone(),
            name: plan.name.clone(),
            prompt_context: plan.prompt_context(),
            pm_snapshot_synced_at: now.unix_timestamp(),
            flow: plan.flow.clone(),
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
        &project(
            &format!("{wave_name}-old"),
            "previous",
            ProjectStatus::Started,
        ),
    )
    .await;
    let now = OffsetDateTime::now_utc();
    let task = Task {
        id: TaskId::new(),
        plan: TaskPlan {
            id: LinearIssueId::new(issue).unwrap(),
            identifier: format!("FIX-{issue}"),
            title: "Retain execution".into(),
            description: String::new(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: parent.id,
        worktree: worktree.into(),
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
    context.store.create_task(&task, &pr, None).await.unwrap();
    (task, pr)
}

async fn local_started_task(
    context: &PmTestContext,
    repo: &std::path::Path,
) -> (Task, TaskPr, FlowSession) {
    let (task, _) = local_task(
        context,
        repo,
        "a",
        "a-started",
        repo,
        "0000000000000000000000000000000000000000",
    )
    .await;
    let now = OffsetDateTime::now_utc();
    let flow = context
        .store
        .start_task_flow(
            &task.id,
            FlowSession {
                invocation: QueuedInvocation::new(
                    "captured",
                    vec![ConcreteStep::Skill(ConcreteSkill {
                        skill: Skill::named("implement"),
                        sources: Vec::new(),
                        id: Some("implement".into()),
                        human: false,
                        repeat: None,
                    })],
                )
                .unwrap(),
                cursor: ExecutionCursor::default(),
                version: 0,
                task_id: Some(task.id.clone()),
                wave_id: Some(task.wave_id.clone()),
                cwd: repo.into(),
                message: Some("original input".into()),
                model: None,
                current_attempt: None,
                pending_session_id: None,
                ready_summary: None,
                worker_generation: 0,
                claim: None,
                failure: None,
                finished: false,
                updated_at: now,
            },
        )
        .await
        .unwrap();
    assert!(!context.store.task_started(&task.id).await.unwrap());
    let flow = context
        .store
        .reserve_attempt(flow.id(), flow.version, None, None)
        .await
        .unwrap();
    assert!(context.store.task_started(&task.id).await.unwrap());
    let pr = context.store.task_prs(&task.id).await.unwrap().remove(0);
    // Compare persisted values: SQLite stores timestamps at second precision.
    let task = context.store.get_task(&task.id).await.unwrap().unwrap();
    (task, pr, flow)
}

fn task_started_at(path: &std::path::Path, task: &TaskId) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT started_at FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .expect("reserved Task work has a non-null Started timestamp")
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
async fn rotation_preserves_unreviewed_backlog_and_authored_project_content() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let mut fixture = provider_fixture();
    for wave in ["a", "b"] {
        let old = fixture.projects.get_mut(&format!("{wave}-old")).unwrap();
        // Isolate backlog preservation; differing predecessor names still require
        // the configured-ID selector tracked by created-successor recovery.
        old["name"] = json!("previous");
        old["content"] = json!("flow: feature\n\n## KRs\n- [x] Retain predecessor evidence");
        let mut next = old.clone();
        next["id"] = json!(format!("{wave}-next"));
        next["name"] = json!("next");
        next["status"]["type"] = json!("planned");
        next["content"] = json!("## KRs\n- [ ] Authored next outcome");
        fixture.projects.insert(format!("{wave}-next"), next);
    }
    let provider = Arc::new(Mutex::new(fixture));
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    local_backlog_tasks(&context, &repo).await;
    let store = context.store.clone();
    let mut retained = Vec::new();
    for wave in ["a", "b"] {
        let task = store
            .get_task_by_issue(&format!("{wave}-backlog"))
            .await
            .unwrap()
            .unwrap();
        let prs = store.task_prs(&task.id).await.unwrap();
        retained.push((task, prs));
    }
    PM_TEST_CONTEXT
        .scope(context, rotate(&repo, "next", false))
        .await
        .unwrap();
    for (task, prs) in retained {
        let after = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(after.project_id, task.project_id);
        assert_eq!(after.worktree, task.worktree);
        assert_eq!(store.task_prs(&task.id).await.unwrap(), prs);
        assert!(!store.task_started(&task.id).await.unwrap());
        assert_eq!(
            store
                .work_status(&crate::durable::WorkRef::Task(task.id))
                .await
                .unwrap(),
            crate::durable::WorkStatus::Ready
        );
    }
    let state = provider.lock().await;
    for wave in ["a", "b"] {
        assert_eq!(
            state.issues[&format!("{wave}-backlog")]["state"]["type"],
            "unstarted"
        );
        assert_eq!(
            state.issues[&format!("{wave}-backlog")]["project"]["id"],
            format!("{wave}-old")
        );
        assert_eq!(
            state.projects[&format!("{wave}-old")]["content"],
            "flow: feature\n\n## KRs\n- [x] Retain predecessor evidence"
        );
        assert_eq!(
            state.projects[&format!("{wave}-next")]["content"],
            "## KRs\n- [ ] Authored next outcome"
        );
    }
    server.abort();
}

#[tokio::test]
async fn rotation_excludes_checkout_starts_and_failed_reset_retries_preserving_sessions() {
    for start_first in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let repo = fixture_repo(directory.path());
        let mut fixture = provider_fixture();
        fixture.issues.retain(|id, _| id == "a-backlog");
        fixture.projects.get_mut("b-old").unwrap()["name"] = json!("next");
        let mut next = fixture.projects["a-old"].clone();
        next["id"] = json!("a-next");
        next["name"] = json!("next");
        next["status"]["type"] = json!("planned");
        fixture.projects.insert("a-next".into(), next);
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        fixture.inventory_pause = Some((entered.clone(), release.clone()));
        let provider = Arc::new(Mutex::new(fixture));
        let (url, server) = serve_fixture(provider.clone()).await;
        let context = context(&directory.path().join("registry.db"), &repo, &url).await;
        let checkout = directory.path().join("missing-checkout");
        let (task, _) = local_task(&context, &repo, "a", "a-backlog", &checkout, "base").await;
        let pr = context.store.task_prs(&task.id).await.unwrap().remove(0);
        let session = crate::session::AgentSession {
            captured: None,
            id: uuid::Uuid::new_v4().to_string(),
            artifact_key: crate::session_record::new_artifact_key(),
            caller_artifact_key: None,
            input_published: false,
            cwd: checkout.join("nested"),
            skill: None,
            provider: None,
            model: None,
            node: None,
            iterations: None,
            task_id: None,
            wave_id: None,
            flow_session_id: None,
            work_source: None,
            bound_at: None,
            kind: crate::session::SessionKind::Conversation,
            interactive: false,
            repo: None,
            title: "Independent conversation".into(),
            title_source: crate::session::TitleSource::Generated,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at: 100,
        };
        let store = context.store.clone();
        if start_first {
            store.create_session(session.clone(), None).await.unwrap();
        }
        let rotating_repo = repo.clone();
        let rotating_context = PmTestContext {
            path: context.path.clone(),
            store: context.store.clone(),
            graphql_url: context.graphql_url.clone(),
        };
        let rotating = tokio::spawn(PM_TEST_CONTEXT.scope(rotating_context, async move {
            rotate(&rotating_repo, "next", false).await
        }));
        tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        if !start_first {
            let error = store
                .create_session(session.clone(), None)
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("checkout admission unavailable"),
                "{error}"
            );
            assert!(store.session(&session.id).await.unwrap().is_none());
        }
        provider.lock().await.unavailable = true;
        release.notify_one();
        assert!(rotating
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("fixture interrupted connection"));
        if !start_first {
            store.create_session(session.clone(), None).await.unwrap();
        }
        let saved = store.session(&session.id).await.unwrap().unwrap();
        assert!(saved.task_id.is_none());
        assert!(!store.task_started(&task.id).await.unwrap());
        provider.lock().await.unavailable = false;
        PM_TEST_CONTEXT
            .scope(context, rotate(&repo, "next", false))
            .await
            .unwrap();
        let retained = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(retained.id, task.id);
        assert_eq!(retained.worktree, checkout);
        assert_eq!(
            store
                .get_project(&retained.project_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .id
                .as_str(),
            "a-next"
        );
        assert_eq!(store.session(&session.id).await.unwrap(), Some(saved));
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        assert_eq!(store.sqlite.task_work(&task.id).unwrap().sessions.len(), 1);
        assert_eq!(
            provider.lock().await.issues["a-backlog"]["state"]["type"],
            "unstarted"
        );
        server.abort();
    }
}

#[tokio::test]
async fn rotation_persists_confirmed_transfer_before_final_refresh() {
    confirmed_transfer_recovery(false).await;
}

#[tokio::test]
async fn rotation_recovers_a_created_successor_after_confirmed_transfer() {
    confirmed_transfer_recovery(true).await;
}

async fn confirmed_transfer_recovery(create_successor: bool) {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let mut fixture = provider_fixture();
    fixture.issues.retain(|id, _| id == "a-started");
    fixture.projects.get_mut("b-old").unwrap()["name"] = json!("next");
    if !create_successor {
        let mut next = fixture.projects["a-old"].clone();
        next["id"] = json!("a-next");
        next["name"] = json!("next");
        next["content"] = json!("## KRs\n- [ ] Retain proof");
        next["status"]["type"] = json!("planned");
        fixture.projects.insert("a-next".into(), next);
    }
    fixture.interrupt_after_transfer_readback = true;
    let provider = Arc::new(Mutex::new(fixture));
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    let (task, pr, flow) = local_started_task(&context, &repo).await;
    let store = context.store.clone();
    PM_TEST_CONTEXT
        .scope(context, async {
            let error = rotate(&repo, "next", false).await.unwrap_err();
            assert!(
                error.to_string().contains("fixture interrupted connection"),
                "{error}"
            );
            let transferred = store.get_task(&task.id).await.unwrap().unwrap();
            let successor = store
                .get_project(&transferred.project_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(transferred.project_id, successor.id);
            assert_eq!(transferred.worktree, task.worktree);
            if !create_successor {
                assert_eq!(successor.plan.name, "next");
                assert!(successor.plan.flow.is_empty());
            }
            assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr.clone()]);
            assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), flow);
            let accepted = store
                .pm_task_observation(repo.to_str().unwrap(), "linear", task.plan.id.as_str())
                .await
                .unwrap();
            assert_eq!(
                accepted.record.unwrap().item.project_id.as_deref(),
                Some(successor.plan.id.as_str())
            );
            provider.lock().await.unavailable = false;
            rotate(&repo, "next", false).await.unwrap();
            assert_eq!(
                store.get_task(&task.id).await.unwrap().unwrap().project_id,
                successor.id
            );
            assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
            assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), flow);
            assert_eq!(
                store
                    .get_project_by_project("a-old")
                    .await
                    .unwrap()
                    .unwrap()
                    .plan
                    .status,
                ProjectStatus::Completed
            );
        })
        .await;
    server.abort();
}

#[tokio::test]
async fn rotation_preserves_conflicting_statuses_observed_in_the_final_inventory() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider.clone()).await;
    let successor = successor_id("initiative-a", "next");
    for (index, (id, status)) in [
        ("a-old", "canceled"),
        ("a-old", "planned"),
        (successor.as_str(), "canceled"),
        (successor.as_str(), "planned"),
        (successor.as_str(), "completed"),
    ]
    .into_iter()
    .enumerate()
    {
        {
            let mut state = provider.lock().await;
            *state = provider_fixture();
            state.status_change_at_final_inventory = Some((id.into(), status.into()));
        }
        let home = context(
            &directory.path().join(format!("conflict-{index}.db")),
            &repo,
            &url,
        )
        .await;
        local_backlog_tasks(&home, &repo).await;
        let failure = PM_TEST_CONTEXT
            .scope(home, rotate(&repo, "next", false))
            .await
            .unwrap_err()
            .to_string();
        assert!(
            failure.contains(if id == "a-old" {
                "predecessor status changed"
            } else {
                "successor is no longer the intended In Progress Project"
            }),
            "{failure}"
        );
        let state = provider.lock().await;
        assert!(state.status_change_at_final_inventory.is_none());
        assert_eq!(
            state.projects["a-old"]["status"]["type"],
            if id == "a-old" { status } else { "started" }
        );
        assert_eq!(
            state.projects[&successor]["status"]["type"],
            if id == "a-old" { "started" } else { status }
        );
        assert_eq!(state.issues["a-started"]["project"]["id"], successor);
        assert_eq!(state.issues["a-backlog"]["state"]["type"], "unstarted");
        assert_eq!(state.projects["b-old"]["status"]["type"], "started");
    }
    server.abort();
}

#[tokio::test]
async fn rotation_accepts_an_already_completed_predecessor_at_final_inventory() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let mut state = provider_fixture();
    state.status_change_at_final_inventory = Some(("a-old".into(), "completed".into()));
    let provider = Arc::new(Mutex::new(state));
    let (url, server) = serve_fixture(provider.clone()).await;
    let home = context(&directory.path().join("completed.db"), &repo, &url).await;
    local_backlog_tasks(&home, &repo).await;
    PM_TEST_CONTEXT
        .scope(home, async {
            rotate(&repo, "next", false).await.unwrap();
            rotate(&repo, "next", false).await.unwrap();
        })
        .await;
    let state = provider.lock().await;
    assert!(state.status_change_at_final_inventory.is_none());
    for wave in ["a", "b"] {
        assert_eq!(
            state.projects[&format!("{wave}-old")]["status"]["type"],
            "completed"
        );
        let successor = successor_id(&format!("initiative-{wave}"), "next");
        assert_eq!(state.projects[&successor]["status"]["type"], "started");
        assert_eq!(
            state.issues[&format!("{wave}-started")]["project"]["id"],
            successor
        );
    }
    server.abort();
}

#[tokio::test]
async fn explicit_sync_renames_legacy_projects_without_rewriting_authored_content() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let original = "Keep this prose.\n\n## Flows\nrecommended: custom\n\n## KRs\n- [ ] Keep this KR\n\n## Notes\nRetain this closing note.\n";
    let mut state = provider_fixture();
    let old = state.projects.get_mut("a-old").unwrap();
    old["name"] = json!("previous");
    old["description"] = json!("Authored summary unrelated to the Flow or KRs.");
    old["content"] = json!(original);
    old["status"]["type"] = json!("planned");
    let mut expected = old.clone();
    expected["name"] = json!("A — previous");
    expected["content"] = json!(original.replace("recommended:", "flow:"));
    expected["status"]["type"] = json!("started");
    let provider = Arc::new(Mutex::new(state));
    let (url, server) = serve_fixture(provider.clone()).await;
    let home = context(&directory.path().join("sync.db"), &repo, &url).await;
    let (task, pr, flow) = local_started_task(&home, &repo).await;
    rusqlite::Connection::open(&home.path)
        .unwrap()
        .execute(
            "UPDATE projects SET legacy_current=1 WHERE external_project_id='a-old'",
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
    let mut observed = provider.lock().await.projects["a-old"].clone();
    assert_ne!(observed.as_object_mut().unwrap().remove("updatedAt"), None);
    assert_eq!(observed, expected);
    let snapshot = store.pm_snapshot(&task.wave_id).await.unwrap().unwrap();
    let snapshot = snapshot.snapshot;
    let synced = snapshot
        .projects
        .iter()
        .find(|project| project.id == "a-old")
        .unwrap();
    assert_eq!(synced.flow, "custom");
    assert_eq!(synced.krs.len(), 1);
    assert_eq!(synced.krs[0].text, "Keep this KR");
    let retained = store.get_task(&task.id).await.unwrap().unwrap();
    assert_eq!(retained.project_id, task.project_id);
    assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
    assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), flow);
    server.abort();
}

#[tokio::test]
async fn every_provider_mutation_recovers_on_the_same_or_a_second_home() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/", post(graphql))
        .with_state(provider.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    // Five effects per Wave: create, attach, activate, move, complete.
    for stop in 1..=10 {
        for second_home in [false, true] {
            *provider.lock().await = provider_fixture();
            provider.lock().await.interrupt_after = Some(stop);
            let first = context(
                &directory
                    .path()
                    .join(format!("first-{stop}-{second_home}.db")),
                &repo,
                &url,
            )
            .await;
            local_backlog_tasks(&first, &repo).await;
            let retained = if stop == 4 {
                let (task, pr, flow) = local_started_task(&first, &repo).await;
                let started_at = task_started_at(&first.path, &task.id);
                Some((task, pr, flow, started_at))
            } else {
                None
            };
            let original = PmTestContext {
                path: first.path.clone(),
                store: first.store.clone(),
                graphql_url: first.graphql_url.clone(),
            };
            PM_TEST_CONTEXT
                .scope(
                    PmTestContext {
                        path: first.path.clone(),
                        store: first.store.clone(),
                        graphql_url: first.graphql_url.clone(),
                    },
                    async {
                        let preview = rotate(&repo, "next", true).await.unwrap();
                        assert_eq!(preview.waves.len(), 2);
                        assert_eq!(provider.lock().await.mutations, 0);
                        assert!(rotate(&repo, "next", false).await.is_err());
                    },
                )
                .await;
            {
                let mut state = provider.lock().await;
                state.unavailable = false;
                state.interrupt_after = None;
            }
            let resumed = if second_home {
                context(
                    &directory.path().join(format!("second-{stop}.db")),
                    &repo,
                    &url,
                )
                .await
            } else {
                first
            };
            if second_home {
                local_backlog_tasks(&resumed, &repo).await;
            }
            PM_TEST_CONTEXT
                .scope(resumed, async {
                    rotate(&repo, "next", false).await.unwrap();
                    let mutations = provider.lock().await.mutations;
                    rotate(&repo, "next", false).await.unwrap();
                    assert_eq!(provider.lock().await.mutations, mutations);
                })
                .await;
            if let Some((task, pr, flow, started_at)) = retained {
                let path = original.path.clone();
                PM_TEST_CONTEXT
                    .scope(original, async {
                        rotate(&repo, "next", false).await.unwrap();
                        let store = super::pm_store().await.unwrap();
                        let moved = store.get_task(&task.id).await.unwrap().unwrap();
                        assert_ne!(moved.project_id, task.project_id);
                        assert_eq!(moved.id, task.id);
                        assert_eq!(moved.worktree, task.worktree);
                        assert_eq!(moved.plan, task.plan);
                        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
                        assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), flow);
                        assert_eq!(task_started_at(&path, &task.id), started_at);
                    })
                    .await;
            }
            let state = provider.lock().await;
            assert_eq!(state.projects.len(), 4);
            assert_eq!(state.issues.len(), 6);
            for wave in ["a", "b"] {
                let successor = successor_id(&format!("initiative-{wave}"), "next");
                assert_eq!(state.projects[&successor]["status"]["type"], "started");
                assert_eq!(
                    state.projects[&format!("{wave}-old")]["status"]["type"],
                    "completed"
                );
                assert_eq!(
                    state.issues[&format!("{wave}-started")]["project"]["id"],
                    successor
                );
                assert_eq!(
                    state.issues[&format!("{wave}-backlog")]["state"]["type"],
                    "unstarted"
                );
                assert_eq!(
                    state.issues[&format!("{wave}-done")]["project"]["id"],
                    format!("{wave}-old")
                );
            }
        }
    }
    // An authored successor keeps its identity, Flow and KR content.
    *provider.lock().await = provider_fixture();
    let content = "flow: custom\n\n## KRs\n- [ ] Authored next proof";
    provider.lock().await.projects.insert(
        "authored-next".into(),
        json!({
            "id":"authored-next", "name":"B — next", "description":"", "archivedAt":null,
            "content":content, "status":{"type":"planned"},
            "teams":{"nodes":[{"id":"team-1"}]}, "initiatives":{"nodes":[{"id":"initiative-b"}]}
        }),
    );
    let authored = context(&directory.path().join("authored.db"), &repo, &url).await;
    local_backlog_tasks(&authored, &repo).await;
    PM_TEST_CONTEXT
        .scope(authored, async {
            provider.lock().await.issues.get_mut("a-backlog").unwrap()["state"]["type"] =
                json!("unknown");
            assert!(rotate(&repo, "next", false)
                .await
                .unwrap_err()
                .to_string()
                .contains("unavailable"));
            assert_eq!(provider.lock().await.mutations, 0);
            provider.lock().await.issues.get_mut("a-backlog").unwrap()["state"]["type"] =
                json!("unstarted");
            let preview = rotate(&repo, "next", true).await.unwrap();
            assert_eq!(
                preview
                    .waves
                    .iter()
                    .find(|wave| wave.wave == "b")
                    .unwrap()
                    .successor_id,
                "authored-next"
            );
            rotate(&repo, "next", false).await.unwrap();
        })
        .await;
    assert_eq!(
        provider.lock().await.projects["authored-next"]["content"],
        content
    );
    server.abort();
}

#[tokio::test]
async fn a_second_home_adopts_completed_rotation_through_planning_sync() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider.clone()).await;
    let first = context(&directory.path().join("rotating.db"), &repo, &url).await;
    local_backlog_tasks(&first, &repo).await;
    let second = context(&directory.path().join("syncing.db"), &repo, &url).await;
    let (task, pr, flow) = local_started_task(&second, &repo).await;
    let path = second.path.clone();
    let started_at = task_started_at(&path, &task.id);
    let store = second.store.clone();

    PM_TEST_CONTEXT
        .scope(first, rotate(&repo, "next", false))
        .await
        .unwrap();
    assert_eq!(store.get_task(&task.id).await.unwrap().unwrap(), task);
    let (mutations, projects, issues) = {
        let state = provider.lock().await;
        (
            state.mutations,
            state.projects.clone(),
            state.issues.clone(),
        )
    };
    let sync_repo = repo.clone();
    // Operation-level sync owns a runtime; this does not invoke the public CLI.
    tokio::task::spawn_blocking(move || {
        PM_TEST_CONTEXT.sync_scope(second, || {
            pm_sync(
                &sync_repo,
                &PmSyncOptions {
                    wave: None,
                    plan: false,
                },
                &NullProgress,
            )
            .unwrap();
        });
    })
    .await
    .unwrap();

    for name in ["a", "b"] {
        let wave = store
            .get_wave_at(&WaveLocator::discover(&repo, name).unwrap())
            .await
            .unwrap()
            .unwrap();
        let snapshot = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        let snapshot = snapshot.snapshot;
        let successor = successor_id(&format!("initiative-{name}"), "next");
        let current = super::select_current(name, &snapshot.projects).unwrap();
        assert_eq!(current.id, successor);
        assert_eq!(current.status, ProjectStatus::Started);
        let previous = store
            .get_project_by_project(&format!("{name}-old"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(previous.plan.status, ProjectStatus::Completed);
        let adopted = store
            .get_project_by_project(&successor)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(adopted.wave_id, *wave.id());
        assert_eq!(adopted.plan.id.as_str(), successor);
        assert_eq!(adopted.plan.status, ProjectStatus::Started);
        assert!(snapshot.items.iter().any(|item| {
            item.id == format!("{name}-started")
                && item.project_id.as_deref() == Some(successor.as_str())
        }));
        if name == "a" {
            assert_eq!(previous.id, task.project_id);
            let moved = store.get_task(&task.id).await.unwrap().unwrap();
            assert_eq!(moved.project_id, adopted.id);
            assert_ne!(moved.project_id, task.project_id);
            assert_eq!(moved.id, task.id);
            assert_eq!(moved.wave_id, task.wave_id);
            assert_eq!(moved.worktree, task.worktree);
            assert_eq!(moved.plan, task.plan);
        }
    }
    assert_eq!(task_started_at(&path, &task.id), started_at);
    assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
    assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), flow);
    let state = provider.lock().await;
    assert_eq!(state.mutations, mutations);
    assert_eq!(state.projects, projects);
    assert_eq!(state.issues, issues);
    server.abort();
}

#[tokio::test]
async fn a_second_home_cannot_expire_backlog_with_unobserved_work() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    provider.lock().await.issues.get_mut("a-started").unwrap()["state"]["type"] =
        json!("unstarted");
    let (url, server) = serve_fixture(provider.clone()).await;
    let first = context(&directory.path().join("owner.db"), &repo, &url).await;
    let checkout = directory.path().join("working-checkout");
    let base = clean_checkout(&checkout);
    let (work, _) = local_task(&first, &repo, "a", "a-started", &checkout, &base).await;
    std::fs::write(
        checkout.join("unfinished.txt"),
        "work exists only on the owning Home",
    )
    .unwrap();
    let second = context(&directory.path().join("observer.db"), &repo, &url).await;
    PM_TEST_CONTEXT
        .scope(first, async {
            let plan = rotate(&repo, "next", true).await.unwrap();
            let task = plan.waves[0]
                .tasks
                .iter()
                .find(|task| task.task.id == "a-started")
                .unwrap();
            assert_eq!(task.disposition, TaskDisposition::Move);
        })
        .await;
    PM_TEST_CONTEXT
        .scope(second, async {
            let plan = rotate(&repo, "next", true).await.unwrap();
            let task = plan.waves[0]
                .tasks
                .iter()
                .find(|task| task.task.id == "a-started")
                .unwrap();
            assert_eq!(task.disposition, TaskDisposition::Unresolved);
            assert!(rotate(&repo, "next", false).await.is_err());
            let store = super::pm_store().await.unwrap();
            assert!(store.get_task(&work.id).await.unwrap().is_none());
        })
        .await;
    let state = provider.lock().await;
    assert_eq!(state.mutations, 0);
    assert_eq!(state.issues["a-started"]["state"]["type"], "unstarted");
    assert_eq!(state.issues["a-started"]["project"]["id"], "a-old");
    server.abort();
}

#[tokio::test]
async fn archived_predecessor_is_history_even_when_linear_still_says_started() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    {
        let mut state = provider.lock().await;
        let mut current = state.projects["a-old"].clone();
        current["id"] = json!("a-current");
        current["name"] = json!("A — current");
        state.projects.insert("a-current".into(), current);
        state.projects.get_mut("a-old").unwrap()["archivedAt"] = json!("2026-09-01T00:00:00Z");
    }
    let (url, server) = serve_fixture(provider.clone()).await;
    let home = context(&directory.path().join("archive.db"), &repo, &url).await;
    let (task, pr, flow) = local_started_task(&home, &repo).await;
    PM_TEST_CONTEXT
        .scope(home, async {
            let ctx = super::resolve_context(&repo, "a").await.unwrap();
            let snapshot = crate::ops::pm::refresh_pm_snapshot(&repo, "a", &ctx)
                .await
                .unwrap();
            assert_eq!(
                super::select_current("a", &snapshot.projects).unwrap().id,
                "a-current"
            );
            assert_eq!(
                snapshot
                    .projects
                    .iter()
                    .find(|project| project.id == "a-old")
                    .unwrap()
                    .status,
                ProjectStatus::Completed
            );
            assert!(snapshot
                .items
                .iter()
                .any(|item| item.id == "a-started" && item.project_id.as_deref() == Some("a-old")));
            let store = super::pm_store().await.unwrap();
            assert_eq!(store.get_task(&task.id).await.unwrap().unwrap(), task);
            assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
            assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), flow);
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
    for (id, name) in [("a-next", "next"), ("a-archived", "archive")] {
        seed_project(
            &home.store,
            &wave,
            &project(id, name, ProjectStatus::Planned),
        )
        .await;
    }
    // These are the migration's retained receipt identities, not provider status.
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute("UPDATE projects SET legacy_current=CASE external_project_id WHEN 'a-old' THEN 1 ELSE 0 END WHERE wave_id=?1",
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
                    let old = state.projects.get_mut("a-old").unwrap();
                    old["status"]["type"] = json!(status);
                    old["content"] = json!(original);
                    for (id, name, archived, flow) in [
                        ("a-next", "next", false, "future-custom"),
                        ("a-archived", "archive", true, "historical-custom"),
                    ] {
                        let mut project = state.projects["a-old"].clone();
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
                            let ctx = super::resolve_context(&repo, "a").await.unwrap();
                            let plans = super::checked_projects(&repo, &ctx, "a").await.unwrap();
                            let current = super::select_current("a", &plans).unwrap();
                            assert_eq!((&*current.id, &*current.flow), ("a-old", "custom"));
                            assert_eq!(
                                plans.iter().find(|p| p.id == "a-next").unwrap().status,
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
                        let flow = store.task_flow(&task.id).await.unwrap();
                        let ctx = super::resolve_context(&repo, "a").await.unwrap();
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
                        assert_eq!(store.task_flow(&task.id).await.unwrap(), flow);
                        {
                            let state = provider.lock().await;
                            assert_eq!(state.projects.len(), 4);
                            assert_eq!(state.issues.len(), 6);
                            assert_eq!(
                                state.projects["a-old"]["content"],
                                original.replace("recommended:", "flow:")
                            );
                            assert_eq!(state.projects["a-old"]["status"]["type"], "started");
                            assert_eq!(state.projects["a-next"]["status"]["type"], "planned");
                            assert_eq!(state.issues["a-started"]["project"]["id"], "a-old");
                        }
                        rotate(&repo, "next", false).await.unwrap();
                        let moved = store.get_task(&task.id).await.unwrap().unwrap();
                        assert_ne!(moved.project_id, task.project_id);
                        assert_eq!(moved.worktree, task.worktree);
                        assert_eq!(store.task_prs(&task.id).await.unwrap(), prs);
                        assert_eq!(store.task_flow(&task.id).await.unwrap(), flow);
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
        let old = state.projects.get_mut("a-old").unwrap();
        old["status"]["type"] = json!("backlog");
        old["content"] = json!("## Flows\nrecommended: custom");
        for id in ["a-next", "a-archived"] {
            let mut foreign = state.projects["a-old"].clone();
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
            let ctx = super::resolve_context(&repo, "a").await.unwrap();
            let plans = super::checked_projects(&repo, &ctx, "a").await.unwrap();
            assert_eq!(plans.len(), 1);
            assert_eq!(super::select_current("a", &plans).unwrap().id, "a-old");
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
                vec![("a-archived".into(), 0), ("a-next".into(), 0)]
            );
            let state = provider.lock().await;
            for id in ["a-next", "a-archived"] {
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
        state.projects.get_mut("a-old").unwrap()["status"]["type"] = json!("backlog");
        state.projects.get_mut("a-old").unwrap()["content"] =
            json!("## Flows\nrecommended: custom");
    }
    let (url, server) = serve_fixture(provider.clone()).await;
    let home = context(&directory.path().join("unrecorded.db"), &repo, &url).await;
    local_started_task(&home, &repo).await;
    rusqlite::Connection::open(&home.path)
        .unwrap()
        .execute(
            "UPDATE projects SET legacy_current=-1 WHERE external_project_id='a-old'",
            [],
        )
        .unwrap();
    PM_TEST_CONTEXT
        .scope(home, async {
            let store = super::pm_store().await.unwrap();
            let ctx = super::resolve_context(&repo, "a").await.unwrap();
            let projects = super::adopt_legacy_projects(&repo, &store, "a", &ctx, false)
                .await
                .unwrap();
            assert_eq!(projects[0].status, ProjectStatus::Started);
            assert_eq!(projects[0].flow, "custom");
            {
                let mut state = provider.lock().await;
                let mut future = state.projects["a-old"].clone();
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
                provider.lock().await.projects["a-old"]["status"]["type"],
                "backlog"
            );
        })
        .await;
    server.abort();
}
