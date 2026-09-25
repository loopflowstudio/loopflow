use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::{empty_plan, read_chapter, rotate, NewChapterRequest};
use crate::controller::wave::metrics::{
    load_metric_contract, MetricContractIssueDto, MetricEvidenceDto, MetricObservation,
    MetricTarget,
};
use crate::id::WaveId;
use crate::ops::pm::{PmTestContext, PM_TEST_CONTEXT};
use crate::planning::{LinearIssueId, TaskPlan};
use crate::pm::ChapterMetricTarget;
use crate::pm::{PmKr, ProjectContent};
use crate::store::{open_ephemeral_store, CredentialType, ProviderToken, StorageConfig};
use crate::work::chapter::{ChapterId, ChapterPhase, TaskDisposition};
use crate::work::task::{Observation, PmWritebackState, Task, TaskId, TaskPr, TaskPrId};
use crate::work::wave::Wave;
use time::{Duration, OffsetDateTime};

#[derive(Default)]
struct Provider {
    projects: BTreeMap<String, Value>,
    issues: BTreeMap<String, Value>,
    archived: BTreeSet<String>,
    mutations: usize,
    lose_create: bool,
    lose_move: bool,
    lose_archive: bool,
    lose_cancel: BTreeSet<String>,
    unlisted: BTreeSet<String>,
    unreadable: BTreeSet<String>,
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
    let mut provider = state.lock().await;
    if query.starts_with("mutation") {
        provider.mutations += 1;
    }
    let data = if query.contains("query ListTeams") {
        json!({"teams": page(vec![json!({"id":"team-1", "name":"Fixture", "key":"FIX", "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"})])})
    } else if query.contains("query ListInitiativeProjects") {
        json!({"initiative": {"projects": page(provider.projects.iter().filter(|(id, _)| !provider.archived.contains(*id) && !provider.unlisted.contains(*id)).map(|(_, project)| project.clone()).collect())}})
    } else if query.contains("query ListProjectIssues") {
        json!({"project": {"issues": page(provider.issues.iter().filter(|(id, issue)| !provider.unlisted.contains(*id) && issue["project"]["id"] == vars["projectId"]).map(|(_, issue)| issue.clone()).collect())}})
    } else if query.contains("query ProjectOwnership") {
        if provider.unreadable.contains(id) {
            return Json(json!({"data":{"project":null}}));
        }
        json!({"project": provider.projects.get(id)})
    } else if query.contains("query IssueOwnership") {
        if provider.unreadable.contains(id) {
            return Json(json!({"data":{"issue":null}}));
        }
        let mut issue = provider.issues[id].clone();
        let project = issue["project"]["id"].as_str().unwrap();
        issue["project"] = provider.projects[project].clone();
        json!({"issue": issue})
    } else if query.contains("query IssueTeam") {
        json!({"issue": {"team": {"id":"team-1"}}})
    } else if query.contains("query CanceledWorkflowStates") {
        json!({"workflowStates": {"nodes":[{"id":"canceled"}]}})
    } else if query.contains("query UnstartedWorkflowStates") {
        json!({"workflowStates":{"nodes":[{"id":"unstarted","position":1.0}]}})
    } else if query.contains("mutation CreateIssue") {
        let project_id = vars["projectId"].as_str().unwrap();
        let name = provider.projects[project_id]["name"]
            .as_str()
            .unwrap()
            .to_string();
        let mut item = issue("filed", "unstarted", project_id, &name);
        item["description"] = vars["description"].clone();
        provider.issues.insert("filed".into(), item);
        json!({"issueCreate":{"issue":{"id":"filed"}}})
    } else if query.contains("mutation CreateProject") {
        let project = json!({"id": id, "name":vars["name"], "description":vars["description"], "content":vars["content"], "initiatives":{"nodes":[]}, "teams":{"nodes":[{"id":"team-1"}]}});
        assert!(
            provider.projects.insert(id.into(), project).is_none(),
            "retry duplicated successor"
        );
        if provider.lose_create {
            provider.lose_create = false;
            return Json(json!({"errors":[{"message":"lost create response"}]}));
        }
        json!({"projectCreate":{"project":{"id":id}}})
    } else if query.contains("mutation AttachProject") {
        let project = vars["projectId"].as_str().unwrap();
        provider.projects.get_mut(project).unwrap()["initiatives"] =
            json!({"nodes":[{"id":"initiative-1"}]});
        json!({"initiativeToProjectCreate":{"initiativeToProject":{"id":"link"}}})
    } else if query.contains("mutation MoveIssueToProject") {
        let project = provider.projects[vars["projectId"].as_str().unwrap()].clone();
        provider.issues.get_mut(id).unwrap()["project"] =
            json!({"id":project["id"],"name":project["name"]});
        if provider.lose_move {
            provider.lose_move = false;
            return Json(json!({"errors":[{"message":"lost move response"}]}));
        }
        json!({"issueUpdate":{"issue":{"id":id}}})
    } else if query.contains("mutation SetIssueState") {
        provider.issues.get_mut(id).unwrap()["state"] = json!({"type":"canceled"});
        if provider.lose_cancel.remove(id) {
            return Json(json!({"errors":[{"message":"lost cancel response"}]}));
        }
        json!({"issueUpdate":{"issue":{"id":id}}})
    } else if query.contains("mutation ArchiveProject") {
        provider.archived.insert(id.into());
        provider.projects.get_mut(id).unwrap()["archivedAt"] = json!("2026-09-24T12:00:00Z");
        if provider.lose_archive {
            provider.lose_archive = false;
            return Json(json!({"errors":[{"message":"lost archive response"}]}));
        }
        json!({"projectArchive":{"success":true}})
    } else {
        panic!("unexpected fixture operation: {query}")
    };
    Json(json!({"data":data}))
}

fn issue(id: &str, state: &str, project: &str, project_name: &str) -> Value {
    json!({"id":id,"identifier":format!("FIX-{id}"),"url":null,"title":id,"description":"",
        "prioritySortOrder":0.0,"sortOrder":0.0,"assignee":null,"state":{"type":state},
        "project":{"id":project,"name":project_name},"team":{"id":"team-1"}})
}

#[tokio::test]
async fn chapter_rotation_previews_retries_and_preserves_dated_history() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    std::fs::create_dir_all(repo.join(".lf")).unwrap();
    // Disposable local history; neither the installed Home nor real Linear is used.
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
    std::fs::create_dir_all(repo.join("wave/product")).unwrap();
    std::fs::write(
        repo.join("wave/product/GOAL.md"),
        "---\npm:\n  linear_initiative: initiative-1\n---\nDurable mandate.\n",
    )
    .unwrap();
    let repo = std::fs::canonicalize(repo).unwrap();
    let metric_path = repo.join("wave/product/metrics/throughput.md");
    std::fs::create_dir_all(metric_path.parent().unwrap()).unwrap();
    std::fs::write(&metric_path, "---\nschema: 1\nid: throughput\nstage: installed\ninstrument: fixture\nunit: ratio\nwindow: 7d\nfreshness: 6h\n---\n# Throughput\nDelivered work.\n").unwrap();
    let goal = std::fs::read_to_string(repo.join("wave/product/GOAL.md")).unwrap();
    let task_checkout = directory.path().join("task");
    std::fs::create_dir(&task_checkout).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "--allow-empty",
            "-qm",
            "Prepared baseline",
        ],
    ] {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(&task_checkout)
            .status()
            .unwrap()
            .success());
    }
    let database = directory.path().join("registry.db");
    let store = Arc::new(
        open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap(),
    );
    let wave = Wave::new(WaveId::new(), "product".into(), repo.display().to_string());
    store.create_wave(&wave).await.unwrap();
    store
        .apply_migration_for_test("project_metric_observations")
        .unwrap();
    let contract = load_metric_contract(&metric_path, wave.id().as_str()).unwrap();
    let measured_at = OffsetDateTime::now_utc();
    store
        .register_metric_instrument(&contract.identity, &contract.instrument, measured_at)
        .await
        .unwrap();
    let mut observation = MetricObservation::Observed {
        identity: contract.identity.clone(),
        contract_revision: contract.contract_revision.clone(),
        instrument: contract.instrument.clone(),
        observation_id: String::new(),
        value: 0.95,
        source_window_start: measured_at - Duration::days(7),
        source_window_end: measured_at,
        complete: true,
    };
    let id = observation.expected_observation_id().unwrap();
    if let MetricObservation::Observed { observation_id, .. } = &mut observation {
        *observation_id = id;
    }
    store
        .accept_metric_observation(&contract, observation.clone(), measured_at)
        .await
        .unwrap();
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
    let provider = Arc::new(Mutex::new(Provider::default()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/", post(graphql))
        .with_state(provider.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let context = PmTestContext {
        path: database,
        store: store.clone(),
        graphql_url: url,
    };
    PM_TEST_CONTEXT
        .scope(context, async {
            let first = NewChapterRequest {
                wave: Some("product".into()),
                chapter: ChapterId::parse("one").unwrap(),
                content: ProjectContent {
                    metric_targets: vec![ChapterMetricTarget {
                        metric_id: "throughput".into(),
                        target: MetricTarget::AtLeast { value: 0.9 },
                    }],
                    krs: vec![PmKr {
                        text: "First proof".into(),
                        holds: false,
                    }],
                    ..empty_plan()
                },
            };
            let preview = rotate(&repo, &first, true).await.unwrap();
            assert_eq!(preview.phase, ChapterPhase::Preview);
            assert_eq!(provider.lock().await.mutations, 0);
            assert!(store.chapters(wave.id()).await.unwrap().is_empty());
            provider.lock().await.lose_create = true;
            let initial = rotate(&repo, &first, false).await.unwrap();
            assert_eq!(initial.error, None);
            assert_eq!(initial.phase, ChapterPhase::Complete);
            assert_eq!(provider.lock().await.projects.len(), 1);
            let filed = crate::ops::pm::pm_update_async(
                &repo,
                &crate::ops::pm::PmUpdateOptions {
                    wave: Some("product".into()),
                    id: None,
                    title: Some("Untouched work".into()),
                    notes: None,
                    status: None,
                    pr: None,
                },
                &crate::ops::NullProgress,
            )
            .await
            .unwrap();
            assert_eq!(
                provider.lock().await.issues[&filed.id]["project"]["id"],
                initial.project_id
            );
            let name = provider.lock().await.projects[&initial.project_id]["name"]
                .as_str()
                .unwrap()
                .to_string();
            {
                let mut provider = provider.lock().await;
                provider.issues.insert(
                    "active".into(),
                    issue("active", "started", &initial.project_id, &name),
                );
                provider.issues.insert(
                    "backlog".into(),
                    issue("backlog", "unstarted", &initial.project_id, &name),
                );
                provider.issues.insert(
                    "done".into(),
                    issue("done", "completed", &initial.project_id, &name),
                );
                provider.issues.insert(
                    "racing".into(),
                    issue("racing", "unstarted", &initial.project_id, &name),
                );
                provider.lose_move = true;
                provider.lose_cancel.insert("backlog".into());
                provider.lose_cancel.insert("filed".into());
            }
            let second = NewChapterRequest {
                wave: Some("product".into()),
                chapter: ChapterId::parse("two").unwrap(),
                content: ProjectContent {
                    metric_targets: vec![ChapterMetricTarget {
                        metric_id: "throughput".into(),
                        target: MetricTarget::AtLeast { value: 0.99 },
                    }],
                    ..empty_plan()
                },
            };
            let project = store.get_project_by_project(&initial.project_id).await.unwrap().unwrap();
            let now = OffsetDateTime::now_utc();
            let backlog = Task {
                id: TaskId::new(),
                plan: TaskPlan {
                    id: LinearIssueId::new("backlog").unwrap(),
                    identifier: "FIX-backlog".into(),
                    title: "Untouched backlog".into(),
                    description: String::new(),
                    pm_snapshot_synced_at: now.unix_timestamp(),
                },
                pm_writeback: PmWritebackState::Current,
                wave_id: wave.id().clone(),
                project_id: project.id,
                worktree: task_checkout.clone(),
                workspace_slug: "backlog".into(),
                abandon_intent: None,
                created_at: now,
                updated_at: now,
                observation: Observation::default(),
            };
            let pr = TaskPr {
                id: TaskPrId::new(),
                task_id: backlog.id.clone(),
                sequence: 1,
                slug: "backlog".into(),
                branch: "backlog".into(),
                base_commit: crate::engine::git::rev_parse(&task_checkout, "HEAD").unwrap(),
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
            store.create_task(&backlog, &pr).await.unwrap();
            // The current binding survives omission of its whole provider Project.
            provider.lock().await.unlisted.insert(initial.project_id.clone());
            let original_content = provider.lock().await.projects[&initial.project_id]["content"].clone();
            provider.lock().await.projects.get_mut(&initial.project_id).unwrap()["content"] =
                json!(original_content.as_str().unwrap().replace("First proof", "Edited provider proof"));
            let mutations = provider.lock().await.mutations;
            let recovered = rotate(&repo, &second, true).await.unwrap();
            assert_eq!(recovered.predecessors.len(), 1);
            assert_eq!(recovered.predecessors[0].id, initial.project_id);
            assert_eq!(recovered.predecessors[0].metric_targets, first.content.metric_targets);
            assert_eq!(recovered.predecessors[0].krs[0].text, "Edited provider proof");
            assert_eq!(recovered.predecessors[0].slug, "chapter-one");
            assert!(recovered.tasks.iter().any(|task| task.task.id == "active"));
            assert!(recovered.tasks.iter().any(|task| task.task.id == "backlog"));
            assert_eq!(provider.lock().await.mutations, mutations);
            provider.lock().await.unreadable.insert(initial.project_id.clone());
            let unavailable = rotate(&repo, &second, true).await.unwrap_err();
            assert!(unavailable.to_string().contains(&initial.project_id));
            assert!(rotate(&repo, &second, false).await.is_err());
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(store.chapter(wave.id(), None).await.unwrap().unwrap().id, initial.id);
            assert!(store.chapter(wave.id(), Some(&second.chapter)).await.unwrap().is_none());
            provider.lock().await.unreadable.clear();
            provider.lock().await.projects.get_mut(&initial.project_id).unwrap()["content"] = original_content;
            // Direct recovery keeps the same ownership rules as the portfolio list.
            provider.lock().await.projects.get_mut(&initial.project_id).unwrap()["initiatives"] =
                json!({"nodes":[{"id":"elsewhere"}]});
            let conflict = rotate(&repo, &second, false).await.unwrap_err();
            assert!(conflict.to_string().contains("elsewhere"));
            assert_eq!(provider.lock().await.mutations, mutations);
            provider.lock().await.projects.get_mut(&initial.project_id).unwrap()["initiatives"] =
                json!({"nodes":[{"id":"initiative-1"}]});
            // A provider list omission must not erase a prepared durable Task.
            provider.lock().await.unlisted.insert("backlog".into());
            let omitted = rotate(&repo, &second, true).await.unwrap();
            assert!(omitted.tasks.iter().any(|task| task.task.id == "backlog"));
            provider.lock().await.unreadable.insert("backlog".into());
            let mutations = provider.lock().await.mutations;
            assert!(rotate(&repo, &second, true).await.is_err());
            assert!(rotate(&repo, &second, false).await.is_err());
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(store.chapter(wave.id(), None).await.unwrap().unwrap().id, initial.id);
            provider.lock().await.unreadable.clear();
            let mut invalid = second.clone();
            invalid.content.metric_targets[0].metric_id = "missing".into();
            let before = provider.lock().await.mutations;
            assert!(rotate(&repo, &invalid, false).await.is_err());
            assert_eq!(provider.lock().await.mutations, before);
            let preview = rotate(&repo, &second, true).await.unwrap();
            assert_eq!(
                preview
                    .tasks
                    .iter()
                    .find(|t| t.task.id == "active")
                    .unwrap()
                    .disposition,
                TaskDisposition::Move
            );
            assert_eq!(
                preview
                    .tasks
                    .iter()
                    .find(|t| t.task.id == "backlog")
                    .unwrap()
                    .disposition,
                TaskDisposition::Abandon
            );
            provider.lock().await.issues.insert(
                "uncertain".into(),
                issue("uncertain", "unknown", &initial.project_id, &name),
            );
            let unresolved = rotate(&repo, &second, false).await.unwrap();
            assert_eq!(unresolved.phase, ChapterPhase::Preparing);
            assert!(unresolved.error.as_ref().unwrap().contains("uncertain"));
            assert_eq!(provider.lock().await.projects.len(), 1);
            assert!(unresolved.tasks.iter().any(|task| task.task.id == "backlog"));
            // A preparing retry previews fresh membership without changing its receipt.
            provider.lock().await.issues.insert(
                "late".into(),
                issue("late", "unstarted", &initial.project_id, &name),
            );
            let original_content = provider.lock().await.projects[&initial.project_id]["content"].clone();
            provider.lock().await.projects.get_mut(&initial.project_id).unwrap()["content"] =
                json!(original_content.as_str().unwrap().replace("First proof", "Revised while preparing"));
            let mutations = provider.lock().await.mutations;
            let refreshed = rotate(&repo, &second, true).await.unwrap();
            assert!(refreshed.tasks.iter().any(|task| task.task.id == "late" && task.disposition == TaskDisposition::Abandon));
            assert_eq!(refreshed.predecessors[0].krs[0].text, "Revised while preparing");
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(store.chapter(wave.id(), Some(&second.chapter)).await.unwrap().unwrap(), unresolved);
            provider.lock().await.unreadable.insert(initial.project_id.clone());
            let mutations = provider.lock().await.mutations;
            let unavailable = rotate(&repo, &second, false).await.unwrap();
            assert_eq!(unavailable.phase, ChapterPhase::Preparing);
            assert!(unavailable.error.as_ref().unwrap().contains(&initial.project_id));
            assert_eq!(unavailable.predecessors, unresolved.predecessors);
            assert_eq!(unavailable.tasks, unresolved.tasks);
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(store.chapter(wave.id(), None).await.unwrap().unwrap().id, initial.id);
            provider.lock().await.unreadable.clear();
            provider.lock().await.unreadable.insert("backlog".into());
            let mutations = provider.lock().await.mutations;
            assert!(rotate(&repo, &second, true).await.is_err());
            let unavailable = rotate(&repo, &second, false).await.unwrap();
            assert_eq!(unavailable.phase, ChapterPhase::Preparing);
            assert!(unavailable.error.as_ref().unwrap().contains("FIX-backlog"));
            assert_eq!(unavailable.predecessors, unresolved.predecessors);
            assert_eq!(unavailable.tasks, unresolved.tasks);
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(store.chapter(wave.id(), None).await.unwrap().unwrap().id, initial.id);
            provider.lock().await.unreadable.clear();
            provider.lock().await.projects.get_mut(&initial.project_id).unwrap()["content"] = original_content;
            provider.lock().await.unlisted.remove("backlog");
            provider.lock().await.issues.get_mut("uncertain").unwrap()["state"] =
                json!({"type":"canceled"});
            // Work begun after preview must carry, never follow the stale retirement.
            provider.lock().await.issues.get_mut("racing").unwrap()["state"] =
                json!({"type":"started"});
            // The first two applications lose responses after the remote side effect.
            let partial = rotate(&repo, &second, false).await.unwrap();
            assert!(partial.error.is_some());
            // External reassignment is neither a lost move nor permission to cancel.
            for id in ["active", "backlog"] {
                let original = provider.lock().await.issues[id]["project"].clone();
                {
                    let mut state = provider.lock().await;
                    let mut outside = state.projects[&initial.project_id].clone();
                    outside["id"] = json!("outside");
                    outside["name"] = json!("Other work");
                    state.projects.insert("outside".into(), outside);
                    state.unlisted.insert("outside".into());
                    state.issues.get_mut(id).unwrap()["project"] =
                        json!({"id":"outside", "name":"Other work"});
                }
                let mutations = provider.lock().await.mutations;
                let preview = rotate(&repo, &second, true).await.unwrap();
                assert_eq!(preview.tasks.iter().find(|task| task.task.id == id).unwrap().disposition, TaskDisposition::Unresolved);
                let conflict = rotate(&repo, &second, false).await.unwrap();
                assert_eq!(conflict.phase, ChapterPhase::Transferring);
                assert!(conflict.error.as_ref().unwrap().contains("membership"));
                assert_eq!(conflict.project_id, partial.project_id);
                let task = conflict.tasks.iter().find(|task| task.task.id == id).unwrap();
                assert_eq!(task.disposition, TaskDisposition::Unresolved);
                assert!(!task.applied);
                assert!(task.at_boundary);
                assert_eq!(task.task.project_id, initial.project_id);
                assert!(!store.chapter_task_evidence(&backlog.id).await.unwrap().abandoned);
                assert_eq!(store.get_task(&backlog.id).await.unwrap().unwrap().project_id, backlog.project_id);
                let mut state = provider.lock().await;
                assert_eq!(state.mutations, mutations);
                assert_eq!(state.issues[id]["project"]["id"], "outside");
                assert!(!state.archived.contains(&initial.project_id));
                state.issues.get_mut(id).unwrap()["project"] = original;
                state.projects.remove("outside");
                state.unlisted.remove("outside");
            }
            let instrument_file = std::fs::read_to_string(&metric_path).unwrap();
            std::fs::write(&metric_path, "instrument changed during transfer").unwrap();
            let again = rotate(&repo, &second, false).await.unwrap();
            assert_eq!(again.project_id, partial.project_id);
            assert!(again.error.is_some());
            assert!(store.chapter_task_evidence(&backlog.id).await.unwrap().abandoned);
            let mutations = provider.lock().await.mutations;
            let preview = rotate(&repo, &second, true).await.unwrap();
            assert_eq!(preview.tasks.iter().find(|task| task.task.id == "backlog").unwrap().disposition, TaskDisposition::Abandon);
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(store.chapter(wave.id(), Some(&second.chapter)).await.unwrap().unwrap(), again);
            provider.lock().await.issues.get_mut("backlog").unwrap()["state"] =
                json!({"type":"started"});
            let mutations = provider.lock().await.mutations;
            let conflict = rotate(&repo, &second, false).await.unwrap();
            assert_eq!(conflict.phase, ChapterPhase::Transferring);
            assert!(conflict.error.as_ref().unwrap().contains("start evidence"));
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(conflict.tasks.iter().find(|task| task.task.id == "backlog").unwrap().disposition, TaskDisposition::Unresolved);
            // A completion after the lost cancel response must not be canceled again.
            provider.lock().await.issues.get_mut("backlog").unwrap()["state"] =
                json!({"type":"completed"});
            let canceled = rotate(&repo, &second, false).await.unwrap();
            assert!(canceled.error.as_ref().unwrap().contains("lost cancel response"));
            assert_eq!(provider.lock().await.issues["backlog"]["state"]["type"], "completed");
            assert_eq!(provider.lock().await.issues["filed"]["state"]["type"], "canceled");
            let mutations = provider.lock().await.mutations;
            let preview = rotate(&repo, &second, true).await.unwrap();
            assert_eq!(preview.tasks.iter().find(|task| task.task.id == "filed").unwrap().disposition, TaskDisposition::Abandon);
            assert_eq!(preview.tasks.iter().find(|task| task.task.id == "backlog").unwrap().disposition, TaskDisposition::Historical);
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(store.chapter(wave.id(), Some(&second.chapter)).await.unwrap().unwrap(), canceled);
            provider.lock().await.lose_archive = true;
            let archived = rotate(&repo, &second, false).await.unwrap();
            assert_eq!(archived.phase, ChapterPhase::Transferring);
            assert!(archived.error.as_ref().unwrap().contains("lost archive response"));
            assert!(provider.lock().await.archived.contains(&initial.project_id));
            let mutations = provider.lock().await.mutations;
            let complete = rotate(&repo, &second, false).await.unwrap();
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(
                complete.phase,
                ChapterPhase::Complete,
                "{:?}",
                complete.error
            );
            assert!(complete.tasks.iter().all(|task| task.applied));
            std::fs::write(&metric_path, instrument_file).unwrap();
            let mutations = provider.lock().await.mutations;
            assert_eq!(rotate(&repo, &second, false).await.unwrap(), complete);
            assert_eq!(provider.lock().await.mutations, mutations);
            let state = provider.lock().await;
            assert_eq!(state.projects.len(), 2);
            assert!(state.archived.contains(&initial.project_id));
            assert_eq!(state.issues["active"]["project"]["id"], complete.project_id);
            assert_eq!(state.issues["backlog"]["state"]["type"], "completed");
            assert_eq!(state.issues["filed"]["state"]["type"], "canceled");
            assert_eq!(state.issues["late"]["state"]["type"], "canceled");
            assert_eq!(complete.tasks.iter().find(|task| task.task.id == "backlog").unwrap().disposition, TaskDisposition::Historical);
            assert_eq!(state.issues["done"]["project"]["id"], initial.project_id);
            assert_eq!(state.issues["racing"]["project"]["id"], complete.project_id);
            drop(state);
            provider.lock().await.issues.get_mut("active").unwrap()["state"] =
                json!({"type":"completed"});
            let ctx = crate::ops::pm::resolve_context(&repo, "product")
                .await
                .unwrap();
            crate::ops::pm::refresh_pm_snapshot(&repo, "product", &ctx)
                .await
                .unwrap();
            let historical = read_chapter(&store, &wave, Some(&initial.id))
                .await
                .unwrap();
            assert_eq!(historical.content, first.content);
            assert!(historical.closed_at.is_some());
            let retired = historical.tasks.iter().find(|task| task.task.id == "backlog").unwrap();
            assert!(!retired.task.completed);
            assert_eq!(retired.task.state.as_deref(), Some("unstarted"));
            assert!(
                !historical
                    .tasks
                    .iter()
                    .find(|task| task.task.id == "active")
                    .unwrap()
                    .task
                    .completed
            );
            let current = read_chapter(&store, &wave, None).await.unwrap();
            assert_eq!(current.content, second.content);
            assert!(matches!(
                historical.metrics.metrics[0].evidence,
                MetricEvidenceDto::Met { value: 0.95, .. }
            ));
            assert!(matches!(
                current.metrics.metrics[0].evidence,
                MetricEvidenceDto::Missed { value: 0.95, .. }
            ));
            assert_eq!(
                historical.metrics.metrics[0].contract_revision,
                current.metrics.metrics[0].contract_revision
            );
            assert_eq!(
                historical.metrics.metrics[0].identity,
                current.metrics.metrics[0].identity
            );
            assert!(historical.metrics_evaluated_at <= current.metrics_evaluated_at);
            assert_eq!(current.tasks.len(), 2);
            assert!(
                current
                    .tasks
                    .iter()
                    .find(|task| task.task.id == "active")
                    .unwrap()
                    .task
                    .completed
            );
            let third = NewChapterRequest {
                wave: Some("product".into()),
                chapter: ChapterId::parse("three").unwrap(),
                content: empty_plan(),
            };
            // Historical durable Tasks are outside the next boundary's membership.
            provider.lock().await.unreadable.insert("backlog".into());
            provider.lock().await.unreadable.insert(initial.project_id.clone());
            let instrument_file = std::fs::read_to_string(&metric_path).unwrap();
            std::fs::remove_file(&metric_path).unwrap();
            let last = rotate(&repo, &third, false).await.unwrap();
            std::fs::write(&metric_path, instrument_file).unwrap();
            assert_eq!(last.phase, ChapterPhase::Complete, "{:?}", last.error);
            let untargeted = read_chapter(&store, &wave, None).await.unwrap();
            assert_eq!(untargeted.metrics.metrics[0].target, None);
            assert!(matches!(
                untargeted.metrics.metrics[0].evidence,
                MetricEvidenceDto::Untargeted { value: 0.95, .. }
            ));
            assert_eq!(untargeted.metrics.metrics[0].identity, contract.identity);
            assert_eq!(
                untargeted.metrics.metrics[0].contract_revision,
                contract.contract_revision
            );
            assert_eq!(untargeted.tasks[0].task.id, "racing");
            assert_eq!(
                provider.lock().await.issues["racing"]["project"]["id"],
                last.project_id
            );
            let old = read_chapter(&store, &wave, Some(&second.chapter))
                .await
                .unwrap();
            assert_eq!(old.content, second.content);
            assert!(old.metrics.metrics.is_empty());
            assert!(matches!(&old.metrics.contract_issues[..],
                [MetricContractIssueDto::UnresolvedTarget { metric_id, .. }] if metric_id == "throughput"));
            // Later instrument changes cannot rewrite the closed chapter's dated verdict.
            std::fs::write(&metric_path, "broken contract after the boundary").unwrap();
            let unchanged = read_chapter(&store, &wave, Some(&first.chapter))
                .await
                .unwrap();
            assert_eq!(unchanged.metrics, historical.metrics);
            assert_eq!(
                unchanged.metrics_evaluated_at,
                historical.metrics_evaluated_at
            );
            assert_eq!(
                std::fs::read_to_string(repo.join("wave/product/GOAL.md")).unwrap(),
                goal
            );
        })
        .await;
    server.abort();
}
