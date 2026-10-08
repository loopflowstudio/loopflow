use crate::store::sqlite::project_selection::{read_project_binding, write_project_binding};
use std::path::Path;

use super::{
    context, fixture_repo, local_backlog_tasks, local_started_task, old_id, provider_fixture,
    serve_fixture, task_started_at, Arc, Mutex, PmTestContext, PM_TEST_CONTEXT,
};
use crate::ops::chapter::{rotate, ChapterPlan, WaveChapterPlan};

use crate::pm::{PmKr, ProjectContent};
use crate::work::wave::WaveLocator;
use serde_json::json;

fn copy_context(context: &PmTestContext) -> PmTestContext {
    PmTestContext {
        path: context.path.clone(),
        store: context.store.clone(),
        graphql_url: context.graphql_url.clone(),
    }
}

async fn plan(context: &PmTestContext, repo: &Path, waves: &[&str]) -> ChapterPlan {
    let mut inputs = Vec::new();
    for name in waves {
        let wave = context
            .store
            .get_wave_at(&WaveLocator::discover(repo, name).unwrap())
            .await
            .unwrap()
            .unwrap();
        super::seed_project(
            &context.store,
            &wave,
            &super::project(old_id(name), "previous", crate::pm::ProjectStatus::Started),
        )
        .await;
        let guard = crate::ops::pm::lock_wave_planning(&wave).await.unwrap();
        write_project_binding(
            &context.store.sqlite,
            wave.id(),
            read_project_binding(&context.store.sqlite, wave.id())
                .unwrap()
                .as_deref(),
            old_id(name),
            &guard,
        )
        .unwrap();
        inputs.push(WaveChapterPlan {
            wave_id: wave.id().clone(),
            successor_id: uuid::Uuid::new_v4().to_string(),
            create: true,
            project_name: "Shared display name".into(),
            content: ProjectContent {
                workflow: String::new(),
                metric_targets: Vec::new(),
                krs: vec![PmKr {
                    text: "Retain work across the switch".into(),
                    holds: false,
                }],
            },
        });
    }
    ChapterPlan {
        name: "next".into(),
        waves: inputs,
    }
}

#[tokio::test]
async fn rotation_recovers_every_mutation_and_partial_repository_settlement() {
    for stop in 1..=10 {
        let directory = tempfile::tempdir().unwrap();
        let repo = fixture_repo(directory.path());
        let provider = Arc::new(Mutex::new(provider_fixture()));
        provider.lock().await.interrupt_after = Some(stop);
        let (url, server) = serve_fixture(provider.clone()).await;
        let context = context(&directory.path().join("registry.db"), &repo, &url).await;
        local_backlog_tasks(&context, &repo).await;
        let (task, pr, flow) = local_started_task(&context, &repo).await;
        let started = task_started_at(&context.path, &task.id);
        PM_TEST_CONTEXT
            .scope(copy_context(&context), async {
                let mut input = plan(&context, &repo, &["a", "b"]).await;
                let preview = rotate(&repo, &input, None, true).await.unwrap();
                assert_eq!(preview.waves.len(), 2);
                assert_eq!(provider.lock().await.mutations, 0);
                assert!(
                    rotate(&repo, &input, None, false).await.is_err(),
                    "stop {stop}"
                );
                for wave in &input.waves {
                    assert!(context
                        .store
                        .project_transition(&wave.wave_id, &wave.successor_id)
                        .await
                        .unwrap()
                        .is_some());
                }
                provider.lock().await.unavailable = false;
                if stop == 1 {
                    let mut changed_intent = input.clone();
                    changed_intent.waves[0].create = false;
                    let mutations = provider.lock().await.mutations;
                    assert!(rotate(&repo, &changed_intent, None, false)
                        .await
                        .unwrap_err()
                        .to_string()
                        .contains("retained transition"));
                    assert_eq!(provider.lock().await.mutations, mutations);
                    // Correct an unfinished plan without losing its reserved destinations.
                    for wave in &mut input.waves {
                        wave.content.krs[0].text = "Corrected outcome before selection".into();
                    }
                }

                rotate(&repo, &input, None, false).await.unwrap();
                let mutations = provider.lock().await.mutations;
                rotate(&repo, &input, None, false).await.unwrap();
                assert_eq!(provider.lock().await.mutations, mutations);
                assert_eq!(provider.lock().await.projects.len(), 4);
                for wave in &input.waves {
                    assert_eq!(
                        read_project_binding(&context.store.sqlite, &wave.wave_id).unwrap(),
                        Some(wave.successor_id.clone())
                    );
                    assert!(context
                        .store
                        .project_transition(&wave.wave_id, &wave.successor_id)
                        .await
                        .unwrap()
                        .unwrap()
                        .settled_at
                        .is_some());
                }
                let retained = context.store.get_task(&task.id).await.unwrap().unwrap();
                assert_eq!(
                    retained.worktree.as_ref().unwrap(),
                    task.worktree.as_ref().unwrap()
                );
                assert_eq!(context.store.task_prs(&task.id).await.unwrap(), vec![pr]);
                assert_eq!(context.store.sqlite.task_flows(&task.id).unwrap(), flow);
                assert_eq!(task_started_at(&context.path, &task.id), started);
                let state = provider.lock().await;
                for wave in ["a", "b"] {
                    assert_eq!(
                        state.issues[&format!("{wave}-backlog")]["project"]["id"],
                        old_id(wave)
                    );
                    assert_eq!(
                        state.issues[&format!("{wave}-backlog")]["state"]["type"],
                        "unstarted"
                    );
                    assert_eq!(state.projects[old_id(wave)]["status"]["type"], "completed");
                }
            })
            .await;
        server.abort();
    }
}

#[tokio::test]
async fn rotation_preflights_every_destination_and_conversion_before_writes() {
    for failure in ["krs", "foreign", "missing", "legacy", "accepted"] {
        let directory = tempfile::tempdir().unwrap();
        let repo = fixture_repo(directory.path());
        let provider = Arc::new(Mutex::new(provider_fixture()));
        let (url, server) = serve_fixture(provider.clone()).await;
        let context = context(&directory.path().join("registry.db"), &repo, &url).await;
        local_backlog_tasks(&context, &repo).await;
        PM_TEST_CONTEXT
            .scope(copy_context(&context), async {
                let mut input = plan(&context, &repo, &["a", "b"]).await;
                input
                    .waves
                    .sort_by(|a, b| a.wave_id.as_str().cmp(b.wave_id.as_str()));
                let last = input.waves.last_mut().unwrap();
                if failure == "krs" {
                    last.content.krs.clear();
                } else if failure == "legacy" {
                    let wave = context
                        .store
                        .get_wave(&last.wave_id)
                        .await
                        .unwrap()
                        .unwrap();
                    let id = old_id(wave.slug());
                    rusqlite::Connection::open(&context.path)
                        .unwrap()
                        .execute(
                            "UPDATE projects SET legacy_current=1 WHERE external_project_id=?1",
                            [id],
                        )
                        .unwrap();
                    provider.lock().await.projects.get_mut(id).unwrap()["content"] =
                        json!("## Metric targets\nnot json");
                } else {
                    last.create = false;
                    if matches!(failure, "foreign" | "accepted") {
                        let mut project = provider.lock().await.projects[old_id("a")].clone();
                        project["id"] = json!(last.successor_id);
                        let wave = context
                            .store
                            .get_wave(&last.wave_id)
                            .await
                            .unwrap()
                            .unwrap();
                        project["initiatives"]["nodes"] =
                            json!([{"id":format!("initiative-{}", wave.slug())}]);
                        if failure == "foreign" {
                            project["teams"]["nodes"] = json!([{"id":"foreign-team"}]);
                        }
                        provider
                            .lock()
                            .await
                            .projects
                            .insert(last.successor_id.clone(), project);
                    }
                }
                if failure == "accepted" {
                    let wave = context
                        .store
                        .get_wave(&last.wave_id)
                        .await
                        .unwrap()
                        .unwrap();
                    let ctx = crate::ops::pm::resolve_context(&repo, wave.slug())
                        .await
                        .unwrap();
                    let mut accepted = ctx
                        .client
                        .project_ownership(&last.successor_id)
                        .await
                        .unwrap();
                    accepted.status = crate::pm::ProjectStatus::Completed;
                    accepted.revision = Some("2100-01-01T00:00:00Z".into());
                    context
                        .store
                        .put_pm_project(wave.id(), "linear", &ctx.initiative, accepted, 1, None)
                        .await
                        .unwrap();
                }
                assert!(
                    rotate(&repo, &input, None, false).await.is_err(),
                    "{failure}"
                );
                assert_eq!(provider.lock().await.mutations, 0, "{failure}");
                for wave in &input.waves {
                    assert!(context
                        .store
                        .pending_project_transition(&wave.wave_id)
                        .await
                        .unwrap()
                        .is_none());
                }
            })
            .await;
        server.abort();
    }
}

#[tokio::test]
async fn rotation_after_switch_retains_selected_membership_and_external_moves() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    provider.lock().await.interrupt_after = Some(5); // completion, after the binding switch
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    local_backlog_tasks(&context, &repo).await;
    let (task, pr, flow) = local_started_task(&context, &repo).await;
    PM_TEST_CONTEXT
        .scope(copy_context(&context), async {
            let input = plan(&context, &repo, &["a"]).await;
            let target = &input.waves[0];
            assert!(rotate(&repo, &input, None, false).await.is_err());
            assert_eq!(
                read_project_binding(&context.store.sqlite, &target.wave_id).unwrap(),
                Some(target.successor_id.clone())
            );
            assert_eq!(
                context
                    .store
                    .project_transition_items(&target.wave_id, &target.successor_id)
                    .await
                    .unwrap(),
                vec!["a-started"]
            );
            {
                let mut state = provider.lock().await;
                state.unavailable = false;
                state.issues.get_mut("a-backlog").unwrap()["state"]["type"] = json!("started");
                state.issues.get_mut("a-started").unwrap()["project"]["id"] = json!(old_id("a"));
            }
            let mutations = provider.lock().await.mutations;
            let error = rotate(&repo, &input, None, false).await.unwrap_err();
            assert!(error.to_string().contains("moved after"), "{error}");
            assert_eq!(provider.lock().await.mutations, mutations);
            provider.lock().await.issues.get_mut("a-started").unwrap()["project"]["id"] =
                json!(target.successor_id);
            rotate(&repo, &input, None, false).await.unwrap();
            assert_eq!(
                provider.lock().await.issues["a-backlog"]["project"]["id"],
                old_id("a")
            );
            assert_eq!(context.store.task_prs(&task.id).await.unwrap(), vec![pr]);
            assert_eq!(context.store.sqlite.task_flows(&task.id).unwrap(), flow);
        })
        .await;
    server.abort();
}

#[tokio::test]
async fn rotation_before_switch_reclassifies_new_work_after_lost_transfer_readback() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    provider.lock().await.interrupt_after_transfer_readback = true;
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    local_backlog_tasks(&context, &repo).await;
    let (task, pr, flow) = local_started_task(&context, &repo).await;
    PM_TEST_CONTEXT
        .scope(copy_context(&context), async {
            let input = plan(&context, &repo, &["a"]).await;
            let target = &input.waves[0];
            assert!(rotate(&repo, &input, None, false).await.is_err());
            assert_eq!(
                read_project_binding(&context.store.sqlite, &target.wave_id)
                    .unwrap()
                    .as_deref(),
                Some(old_id("a"))
            );
            assert_ne!(
                context
                    .store
                    .get_task(&task.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .project_id,
                task.project_id
            );
            {
                let mut state = provider.lock().await;
                state.unavailable = false;
                state.issues.get_mut("a-backlog").unwrap()["state"]["type"] = json!("started");
            }
            rotate(&repo, &input, None, false).await.unwrap();
            assert_eq!(
                provider.lock().await.issues["a-backlog"]["project"]["id"],
                target.successor_id
            );
            assert_eq!(context.store.task_prs(&task.id).await.unwrap(), vec![pr]);
            assert_eq!(context.store.sqlite.task_flows(&task.id).unwrap(), flow);
        })
        .await;
    server.abort();
}

#[tokio::test]
async fn rotation_existing_destination_keeps_unrelated_text_and_same_name_plans_stay_distinct() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    local_backlog_tasks(&context, &repo).await;
    PM_TEST_CONTEXT
        .scope(copy_context(&context), async {
            let mut first = plan(&context, &repo, &["a"]).await;
            first.waves[0].create = false;
            let id = first.waves[0].successor_id.clone();
            let mut existing = provider.lock().await.projects[old_id("a")].clone();
            existing["id"] = json!(id);
            existing["name"] = json!("Ordinary customer work");
            existing["description"] = json!("Keep this summary");
            existing["content"] = json!(
                "Keep this opening.\n\n## KRs\n- [ ] Prior plan\n\n## Notes\nKeep these notes.\n"
            );
            existing["status"]["type"] = json!("planned");
            provider.lock().await.projects.insert(id.clone(), existing);
            rotate(&repo, &first, None, false).await.unwrap();
            let retained = provider.lock().await.projects[&id].clone();
            assert_eq!(retained["name"], "Ordinary customer work");
            assert_eq!(retained["description"], "Keep this summary");
            let body = retained["content"].as_str().unwrap();
            assert!(body.contains("Keep this opening.\n"));
            assert!(body.contains("## Notes\nKeep these notes.\n"));
            assert!(body.contains("Retain work across the switch"));
            let mut second = first.clone();
            second.waves[0].successor_id = uuid::Uuid::new_v4().to_string();
            second.waves[0].create = true;
            rotate(&repo, &second, None, false).await.unwrap();
            let mutations = provider.lock().await.mutations;
            assert!(rotate(&repo, &first, None, false)
                .await
                .unwrap_err()
                .to_string()
                .contains("intervening decision"));
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(provider.lock().await.projects.len(), 4);
        })
        .await;
    server.abort();
}
#[tokio::test]
async fn rotation_excludes_checkout_starts_and_failed_reset_retries_preserving_sessions() {
    for start_first in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let repo = fixture_repo(directory.path());
        let mut fixture = provider_fixture();
        fixture.issues.retain(|id, _| id == "a-backlog");
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        fixture.inventory_pause = Some((entered.clone(), release.clone()));
        let provider = Arc::new(Mutex::new(fixture));
        let (url, server) = serve_fixture(provider.clone()).await;
        let context = context(&directory.path().join("registry.db"), &repo, &url).await;
        let checkout = directory.path().join("missing-checkout");
        let (task, _) =
            super::local_task(&context, &repo, "a", "a-backlog", &checkout, "base").await;
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
            flow_process_lfid: None,
            work_source: None,
            bound_at: None,
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
            store.create_session(session.clone()).await.unwrap();
        }
        let input = PM_TEST_CONTEXT
            .scope(copy_context(&context), plan(&context, &repo, &["a"]))
            .await;
        let successor = input.waves[0].successor_id.clone();
        let rotating_input = input.clone();
        let rotating_repo = repo.clone();
        let rotating_context = PmTestContext {
            path: context.path.clone(),
            store: context.store.clone(),
            graphql_url: context.graphql_url.clone(),
        };
        let rotating = tokio::spawn(PM_TEST_CONTEXT.scope(rotating_context, async move {
            rotate(&rotating_repo, &rotating_input, None, false).await
        }));
        tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        if !start_first {
            let error = store.create_session(session.clone()).await.unwrap_err();
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
            store.create_session(session.clone()).await.unwrap();
        }
        let saved = store.session(&session.id).await.unwrap().unwrap();
        assert!(saved.task_id.is_none());
        assert!(!store.task_started(&task.id).await.unwrap());
        provider.lock().await.unavailable = false;
        PM_TEST_CONTEXT
            .scope(context, rotate(&repo, &input, None, false))
            .await
            .unwrap();
        let retained = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(retained.id, task.id);
        assert_eq!(retained.worktree.as_ref(), Some(&checkout));
        assert_eq!(
            store
                .get_project(&retained.project_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .linear_id
                .as_ref()
                .unwrap()
                .as_str(),
            successor
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
async fn cancelled_transfer_selection_retains_checkout_exclusion_until_persisted() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    PM_TEST_CONTEXT
        .scope(copy_context(&context), async {
            let input = plan(&context, &repo, &["a"]).await;
            let target = input.waves[0].clone();
            let wave = context
                .store
                .get_wave(&target.wave_id)
                .await
                .unwrap()
                .unwrap();
            let wave_guard = crate::ops::pm::lock_wave_planning(&wave).await.unwrap();
            let root = directory.path().join("checkout");
            let checkouts = context
                .store
                .lock_checkout_roots(vec![root.clone()])
                .await
                .unwrap();
            let acquisition = Arc::new(wave_guard.with_checkouts(&checkouts));
            drop(checkouts);
            drop(wave_guard);
            context
                .store
                .reserve_project_transition(
                    crate::store::project_transitions::ProjectTransition {
                        wave_id: target.wave_id.clone(),
                        successor_id: target.successor_id.clone(),
                        predecessor_id: Some(old_id("a").into()),
                        reset_name: Some(input.name),
                        create_successor: Some(true),
                        created_at: 1,
                        settled_at: None,
                    },
                    acquisition.clone(),
                )
                .await
                .unwrap();
            let entered = Arc::new(tokio::sync::Notify::new());
            let (release, blocked) = std::sync::mpsc::channel();
            let gate = (entered.clone(), Arc::new(std::sync::Mutex::new(blocked)));
            let writer_store = context.store.clone();
            let writer_target = target.clone();
            let writer = tokio::spawn(crate::store::PLANNING_ACCEPTANCE_GATE.scope(
                gate,
                async move {
                    writer_store
                        .select_project_transition_item(
                            &writer_target.wave_id,
                            &writer_target.successor_id,
                            "a-started",
                            acquisition,
                        )
                        .await
                },
            ));
            tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
                .await
                .unwrap();
            writer.abort();
            assert!(writer.await.unwrap_err().is_cancelled());
            let excluded = context
                .store
                .lock_checkout_roots(vec![root.clone()])
                .await
                .is_err();
            release.send(()).unwrap();
            let _admitted = context.store.lock_checkout_roots(vec![root]).await.unwrap();
            assert!(
                excluded,
                "cancelled selection released the checkout before commit"
            );
            assert_eq!(
                context
                    .store
                    .project_transition_items(&target.wave_id, &target.successor_id)
                    .await
                    .unwrap(),
                vec!["a-started"]
            );
        })
        .await;
    server.abort();
}

#[tokio::test]
async fn rotation_keeps_the_binding_when_a_project_is_canceled_during_transfer() {
    for successor in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let repo = fixture_repo(directory.path());
        let provider = Arc::new(Mutex::new(provider_fixture()));
        let (url, server) = serve_fixture(provider.clone()).await;
        let context = context(&directory.path().join("registry.db"), &repo, &url).await;
        local_backlog_tasks(&context, &repo).await;
        PM_TEST_CONTEXT
            .scope(copy_context(&context), async {
                let input = plan(&context, &repo, &["a"]).await;
                let target = &input.waves[0];
                let changed = if successor {
                    &target.successor_id
                } else {
                    old_id("a")
                };
                provider.lock().await.status_after_transfer =
                    Some((changed.into(), "canceled".into()));
                assert!(rotate(&repo, &input, None, false).await.is_err());
                assert_eq!(
                    read_project_binding(&context.store.sqlite, &target.wave_id)
                        .unwrap()
                        .as_deref(),
                    Some(old_id("a"))
                );
                assert_eq!(
                    provider.lock().await.projects[changed]["status"]["type"],
                    "canceled"
                );
            })
            .await;
        server.abort();
    }
}

#[tokio::test]
async fn rotation_without_a_predecessor_creates_once_but_unknown_work_still_blocks() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    PM_TEST_CONTEXT
        .scope(copy_context(&context), async {
            let input = plan(&context, &repo, &["a"]).await;
            assert!(rotate(&repo, &input, None, false)
                .await
                .unwrap_err()
                .to_string()
                .contains("unavailable"));
            assert_eq!(provider.lock().await.mutations, 0);
            let target = &input.waves[0];
            rusqlite::Connection::open(&context.path)
                .unwrap()
                .execute(
                    "UPDATE waves SET current_project_id=NULL WHERE id=?1",
                    [target.wave_id.as_str()],
                )
                .unwrap();
            let result = rotate(&repo, &input, None, false).await.unwrap();
            assert!(result.waves[0].predecessor.is_none());
            assert!(result.waves[0].tasks.is_empty());
            let mutations = provider.lock().await.mutations;
            rotate(&repo, &input, None, false).await.unwrap();
            assert_eq!(provider.lock().await.mutations, mutations);
            assert_eq!(
                provider.lock().await.projects[old_id("a")]["status"]["type"],
                "started"
            );
            assert_eq!(
                read_project_binding(&context.store.sqlite, &target.wave_id).unwrap(),
                Some(target.successor_id.clone())
            );
        })
        .await;
    server.abort();
}

#[tokio::test]
async fn rotation_does_not_settle_over_an_intervening_binding() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    local_backlog_tasks(&context, &repo).await;
    PM_TEST_CONTEXT
        .scope(copy_context(&context), async {
            let input = plan(&context, &repo, &["a"]).await;
            let target = &input.waves[0];
            let replacement = uuid::Uuid::new_v4().to_string();
            let wave = context
                .store
                .get_wave(&target.wave_id)
                .await
                .unwrap()
                .unwrap();
            super::seed_project(
                &context.store,
                &wave,
                &super::project(
                    &replacement,
                    "Intervening plan",
                    crate::pm::ProjectStatus::Started,
                ),
            )
            .await;
            provider.lock().await.binding_replacement =
                Some((context.path.clone(), replacement.clone()));
            assert!(rotate(&repo, &input, None, false)
                .await
                .unwrap_err()
                .to_string()
                .contains("binding changed before settlement"));
            assert_eq!(
                read_project_binding(&context.store.sqlite, &target.wave_id).unwrap(),
                Some(replacement)
            );
            assert!(context
                .store
                .project_transition(&target.wave_id, &target.successor_id)
                .await
                .unwrap()
                .unwrap()
                .settled_at
                .is_none());
            let mutations = provider.lock().await.mutations;
            assert!(rotate(&repo, &input, None, false)
                .await
                .unwrap_err()
                .to_string()
                .contains("intervening decision"));
            assert_eq!(provider.lock().await.mutations, mutations);
        })
        .await;
    server.abort();
}

#[tokio::test]
async fn wave_rotation_uses_its_entry_when_an_unselected_plan_is_invalid() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    local_backlog_tasks(&context, &repo).await;
    PM_TEST_CONTEXT
        .scope(copy_context(&context), async {
            let mut input = plan(&context, &repo, &["a", "b"]).await;
            input.waves[1].content.krs.clear();
            assert!(rotate(&repo, &input, None, false).await.is_err());
            assert_eq!(provider.lock().await.mutations, 0);
            let result = rotate(&repo, &input, Some("a"), false).await.unwrap();
            assert_eq!(result.waves.len(), 1);
            assert_eq!(result.waves[0].wave, "a");
            assert_eq!(
                provider.lock().await.projects[old_id("b")]["status"]["type"],
                "started"
            );
            assert!(context
                .store
                .pending_project_transition(&input.waves[1].wave_id)
                .await
                .unwrap()
                .is_none());
        })
        .await;
    server.abort();
}

#[tokio::test]
async fn changing_the_workflow_keeps_the_projects_krs() {
    let directory = tempfile::tempdir().unwrap();
    let repo = fixture_repo(directory.path());
    let provider = Arc::new(Mutex::new(provider_fixture()));
    let (url, server) = serve_fixture(provider.clone()).await;
    let context = context(&directory.path().join("registry.db"), &repo, &url).await;
    PM_TEST_CONTEXT
        .scope(copy_context(&context), async {
            let selected = plan(&context, &repo, &["a"]).await;
            let result = crate::ops::project::workflow(&repo, old_id("a"), Some("research"))
                .await
                .unwrap();
            assert_eq!(result.workflow, "research");
            assert_eq!(selected.waves.len(), 1);
        })
        .await;
    let state = provider.lock().await;
    let content = state.projects[old_id("a")]["content"].as_str().unwrap();
    let plan = crate::pm::parse_project_content(content).unwrap();
    assert_eq!(plan.workflow, "research");
    assert_eq!(plan.krs[0].text, "Retain proof");
    assert!(state.projects[old_id("b")]["content"]
        .as_str()
        .unwrap()
        .contains("workflow: feature"));
    server.abort();
}
