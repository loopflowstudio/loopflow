use std::path::Path;

use crate::config::{remote, settings, Config};
use crate::graph::{ConcreteStep, Skill};
use crate::lf::commands::graph::EnvVarGuard;
use crate::prompt::process::{prepare_process_prompt, ProcessPromptInput};
use loopflow_test_support::TestRepo;

fn home(path: &Path) -> Vec<EnvVarGuard> {
    std::fs::create_dir_all(path.join(".lf")).unwrap();
    [
        ("HOME", path.to_owned()),
        ("LF_HOME", path.join(".lf")),
        ("CODEX_HOME", path.join(".codex")),
        ("CLAUDE_CONFIG_DIR", path.join(".claude")),
    ]
    .into_iter()
    .map(|(key, value)| EnvVarGuard::set(key, value.to_str().unwrap()))
    .collect()
}

fn write(home: &Path, file: &str, text: &str) {
    let path = home.join(".lf").join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn resolve(repo: &Path, skill: Skill) -> settings::ResolvedSettings {
    let config = crate::config::load_config(Some(repo))
        .unwrap()
        .unwrap_or_default();
    let inspection = settings::resolve(&config, Some(&skill)).unwrap();
    let prepared = prepare_process_prompt(
        &config,
        ProcessPromptInput {
            repo_root: repo.to_owned(),
            resolved_skill: Some(skill),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        prepared.config.agent().unwrap(),
        inspection.config.agent().unwrap()
    );
    assert_eq!(prepared.config.skip_permissions, inspection.config.yolo);
    assert_eq!(prepared.config.chrome, inspection.config.chrome);
    for path in &inspection.config.context.docs {
        assert!(prepared.components.docs.iter().any(|doc| &doc.path == path));
    }
    inspection
}

fn probe(repo: &Path) -> settings::ResolvedSettings {
    let resolved = resolve(repo, crate::graph::load_skill("probe", repo).unwrap());
    let inspected = crate::config::api::inspect(repo, None, Some("probe"), None, None).unwrap();
    assert_eq!(
        inspected[0]["settings"]["agent"]["value"],
        resolved.config.agent().unwrap()
    );
    assert_eq!(
        inspected[0]["settings"]["yolo"]["value"],
        resolved.config.yolo
    );
    resolved
}

#[test]
fn forwarded_general_choice_excludes_executor_definition_preferences() {
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let repo = TestRepo::new();
    let origin = tempfile::tempdir().unwrap();
    let executor = tempfile::tempdir().unwrap();
    let profile = {
        let _home = home(origin.path());
        write(
            origin.path(),
            "config.yaml",
            "agent: codex:origin\nyolo: false\n",
        );
        remote::outgoing(Some(repo.path()), "laptop").unwrap()
    };
    let _home = home(executor.path());
    let body = "---\n# independent preferences\nagent: claude:executor\nyolo: true\nchrome: true\n---\nExecutor instructions.\n";
    write(executor.path(), "skills/probe.md", body);
    assert_eq!(
        probe(repo.path()).config.agent().unwrap(),
        "claude:executor"
    );
    {
        let _profile = EnvVarGuard::set(
            remote::PROFILE_ENV,
            &serde_json::to_string(&profile).unwrap(),
        );
        let resolved = probe(repo.path());
        assert_eq!(resolved.config.agent().unwrap(), "codex:origin");
        assert!(!resolved.config.yolo);
        assert!(resolved.config.chrome);
        assert!(!executor.path().join(".lf/profile-retention.json").exists());
        remote::receive(Some(repo.path())).unwrap();
        assert_eq!(probe(repo.path()).config.agent().unwrap(), "codex:origin");
        assert_eq!(
            std::fs::read_to_string(executor.path().join(".lf/skills/probe.md")).unwrap(),
            body
        );
        let receipts = remote::inspect(false, false).unwrap();
        assert!(receipts
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["receipt"]["status"] == "conflict"));
    }
    assert_eq!(
        probe(repo.path()).config.agent().unwrap(),
        "claude:executor"
    );
    remote::inspect(true, false).unwrap();
    assert_eq!(probe(repo.path()).config.agent().unwrap(), "codex:origin");
    let retained = std::fs::read_to_string(executor.path().join(".lf/skills/probe.md")).unwrap();
    assert!(retained.contains("Executor instructions."));
    assert!(retained.contains("chrome: true"));
    assert!(retained.contains("# independent preferences"));
}

#[test]
fn origin_skill_fallback_survives_transport_and_explicit_adoption() {
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let repo = TestRepo::new();
    repo.create_file("reference.md", "Reference from the executor checkout.");
    let origin = tempfile::tempdir().unwrap();
    let executor = tempfile::tempdir().unwrap();
    let profile = {
        let _home = home(origin.path());
        write(origin.path(), "skills/probe.md", "---\ndefault_agent: codex:origin\nyolo: true\ndocs: [reference.md]\n---\nOrigin instructions stay here.");
        remote::outgoing(Some(repo.path()), "laptop").unwrap()
    };
    assert!(!serde_json::to_string(&profile)
        .unwrap()
        .contains("Origin instructions"));
    let _home = home(executor.path());
    write(
        executor.path(),
        "skills/probe.md",
        "---\nagent: claude:executor\n---\nExecutor body.",
    );
    {
        let _profile = EnvVarGuard::set(
            remote::PROFILE_ENV,
            &serde_json::to_string(&profile).unwrap(),
        );
        let resolved = probe(repo.path());
        assert_eq!(resolved.config.agent().unwrap(), "codex:origin");
        assert!(resolved.config.yolo);
        assert_eq!(
            resolved.settings["agent"].selected().unwrap().key,
            "default_agent"
        );
        assert!(resolved.settings["agent"]
            .selected()
            .unwrap()
            .locator
            .starts_with("laptop:"));
        remote::receive(Some(repo.path())).unwrap();
        repo.create_file(
            ".lf/skills/probe.md",
            "---\nagent: claude:repository\n---\nRepository body.",
        );
        assert_eq!(
            probe(repo.path()).config.agent().unwrap(),
            "claude:repository"
        );
    }
    remote::inspect(true, false).unwrap();
    assert_eq!(
        probe(repo.path()).config.agent().unwrap(),
        "claude:repository"
    );
    std::fs::remove_file(repo.path().join(".lf/skills/probe.md")).unwrap();
    assert_eq!(probe(repo.path()).config.agent().unwrap(), "codex:origin");
    let retained = std::fs::read_to_string(executor.path().join(".lf/skills/probe.md")).unwrap();
    assert!(retained.contains("default_agent: codex:origin"));
    assert!(retained.ends_with("Executor body."));
    // Transport carries references, not their files; preparation keeps its actionable error.
    std::fs::remove_file(repo.path().join("reference.md")).unwrap();
    let config = crate::config::load_config(Some(repo.path()))
        .unwrap()
        .unwrap();
    let skill = crate::graph::load_skill("probe", repo.path()).unwrap();
    let error = prepare_process_prompt(
        &config,
        ProcessPromptInput {
            repo_root: repo.path().to_owned(),
            resolved_skill: Some(skill),
            ..Default::default()
        },
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("reference.md"));
}

fn steps(repo: &Path) -> Vec<Skill> {
    let graph = crate::graph::load_authored_graph("outer", repo).unwrap();
    crate::graph::compile_graph(&graph, repo)
        .unwrap()
        .into_iter()
        .filter_map(|step| match step {
            ConcreteStep::Skill(step) => Some(step.skill),
            _ => None,
        })
        .collect()
}

#[test]
fn graph_preferences_keep_authored_identity_through_composition_capture_and_retention() {
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let repo = TestRepo::new();
    repo.create_file(".lf/skills/probe.md", "Repository instructions.");
    repo.create_file(".lf/graphs/outer.yaml", "- graph: inner\n- graph: inner\n");
    let origin = tempfile::tempdir().unwrap();
    let executor = tempfile::tempdir().unwrap();
    let profile = {
        let _home = home(origin.path());
        write(origin.path(), "graphs/inner.yaml", "# origin topology\n- step: {name: probe, id: work, default_agent: codex:named, yolo: true}\n- step: {name: probe, agent: codex:anonymous}\n");
        remote::outgoing(Some(repo.path()), "laptop").unwrap()
    };
    let _home = home(executor.path());
    let topology = "# executor topology\n- step: {name: probe, id: work, agent: claude:executor, chrome: true}\n- step: {name: probe, agent: claude:anonymous}\n";
    write(executor.path(), "graphs/inner.yaml", topology);
    let captured = serde_json::to_string(&steps(repo.path())).unwrap();
    let selected = || {
        serde_json::from_str::<Vec<Skill>>(&captured)
            .unwrap()
            .into_iter()
            .map(|skill| resolve(repo.path(), skill).config)
            .collect::<Vec<Config>>()
    };
    assert_eq!(selected()[0].agent().unwrap(), "claude:executor");
    {
        let _profile = EnvVarGuard::set(
            remote::PROFILE_ENV,
            &serde_json::to_string(&profile).unwrap(),
        );
        let inspected =
            crate::config::api::inspect(repo.path(), None, None, Some("outer"), None).unwrap();
        assert_eq!(inspected.len(), 4);
        for index in [0, 2] {
            let pair = &inspected[index..index + 2];
            assert_eq!(pair[0]["settings"]["agent"]["value"], "codex:named");
            assert_eq!(pair[1]["settings"]["agent"]["value"], "codex:anonymous");
        }
        for skills in [steps(repo.path()), serde_json::from_str(&captured).unwrap()] {
            let resolved: Vec<_> = skills
                .into_iter()
                .map(|skill| resolve(repo.path(), skill))
                .collect();
            assert_eq!(resolved.len(), 4);
            for index in [0, 2] {
                let pair = &resolved[index..index + 2];
                assert_eq!(pair[0].config.agent().unwrap(), "codex:named");
                assert!(pair[0].config.yolo);
                assert!(pair[0].config.chrome);
                assert_eq!(pair[1].config.agent().unwrap(), "codex:anonymous");
                assert_eq!(
                    pair[0].settings["agent"].selected().unwrap().key,
                    "default_agent"
                );
            }
        }
        repo.create_file(
            ".lf/graphs/inner.yaml",
            "- step: {name: probe, id: work, agent: claude:repository}\n",
        );
        let repository =
            crate::config::api::inspect(repo.path(), None, None, Some("inner"), Some("work"))
                .unwrap();
        assert_eq!(
            repository[0]["settings"]["agent"]["value"],
            "claude:repository"
        );
        assert_eq!(
            resolve(repo.path(), steps(repo.path())[0].clone())
                .config
                .agent()
                .unwrap(),
            "claude:repository"
        );
        std::fs::remove_file(repo.path().join(".lf/graphs/inner.yaml")).unwrap();
        remote::receive(Some(repo.path())).unwrap();
        assert_eq!(
            std::fs::read_to_string(executor.path().join(".lf/graphs/inner.yaml")).unwrap(),
            topology
        );
        std::fs::remove_file(executor.path().join(".lf/graphs/inner.yaml")).unwrap();
        assert_eq!(selected()[0].agent().unwrap(), "codex:named");
        write(executor.path(), "graphs/inner.yaml", topology);
    }
    assert_eq!(selected()[0].agent().unwrap(), "claude:executor");
    remote::inspect(true, false).unwrap();
    let configs: Vec<_> = steps(repo.path())
        .into_iter()
        .map(|skill| resolve(repo.path(), skill).config)
        .collect();
    assert_eq!(configs[0].agent().unwrap(), "codex:named");
    assert_eq!(configs[1].agent().unwrap(), "codex:anonymous");
    let retained = std::fs::read_to_string(executor.path().join(".lf/graphs/inner.yaml")).unwrap();
    assert!(retained.contains("# executor topology"));
    assert!(retained.contains("chrome: true"));
    assert!(retained.contains("id: work"));
    // General origin choices also exclude every executor occurrence preference.
    let mut general = profile;
    general.definitions.clear();
    general.global = serde_yaml_ng::from_str("agent: codex:general").unwrap();
    let _profile = EnvVarGuard::set(
        remote::PROFILE_ENV,
        &serde_json::to_string(&general).unwrap(),
    );
    assert!(selected()
        .iter()
        .all(|config| config.agent().unwrap() == "codex:general"));
}

#[test]
fn captured_origin_instructions_keep_executor_host_preferences() {
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let repo = TestRepo::new();
    repo.create_file(".lf/graphs/outer.yaml", "- graph: inner\n");
    let origin = tempfile::tempdir().unwrap();
    let executor = tempfile::tempdir().unwrap();
    let (profile, captured) = {
        let _home = home(origin.path());
        write(
            origin.path(),
            "skills/probe.md",
            "---\nagent: codex:origin\nchrome: true\n---\nCaptured origin instructions.",
        );
        write(
            origin.path(),
            "graphs/inner.yaml",
            "- step: {name: probe, id: work, chrome: true}\n",
        );
        (
            remote::outgoing(Some(repo.path()), "laptop").unwrap(),
            serde_json::to_string(&steps(repo.path())).unwrap(),
        )
    };
    let _home = home(executor.path());
    write(
        executor.path(),
        "skills/probe.md",
        "---\nagent: claude:executor\nchrome: false\n---\nExecutor body.",
    );
    write(
        executor.path(),
        "graphs/inner.yaml",
        "- step: {name: probe, id: work, chrome: false}\n",
    );
    let _profile = EnvVarGuard::set(
        remote::PROFILE_ENV,
        &serde_json::to_string(&profile).unwrap(),
    );
    let captured: Vec<Skill> = serde_json::from_str(&captured).unwrap();
    assert_eq!(
        captured[0].content.as_deref(),
        Some("Captured origin instructions.")
    );
    let selected = resolve(repo.path(), captured[0].clone());
    assert_eq!(selected.config.agent().unwrap(), "codex:origin");
    assert!(!selected.config.chrome);
    assert_eq!(
        selected.settings["chrome"].selected().unwrap().locator,
        executor
            .path()
            .join(".lf/graphs/inner.yaml")
            .display()
            .to_string()
    );
}

#[test]
fn retained_definitions_refresh_and_remove_only_unchanged_preferences() {
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let repo = TestRepo::new();
    let origin = tempfile::tempdir().unwrap();
    let executor = tempfile::tempdir().unwrap();
    let mut profile = {
        let _home = home(origin.path());
        write(
            origin.path(),
            "skills/probe.md",
            "---\ndefault_agent: codex:first\nexclude: [first]\n---\nOrigin.",
        );
        remote::outgoing(Some(repo.path()), "laptop").unwrap()
    };
    let _home = home(executor.path());
    write(
        executor.path(),
        "skills/probe.md",
        "---\n# executor declarations\nchrome: true\nmodel: native-model\n---\nExecutor body.",
    );
    let receive = |profile: &remote::Profile| {
        let _profile = EnvVarGuard::set(
            remote::PROFILE_ENV,
            &serde_json::to_string(profile).unwrap(),
        );
        remote::receive(Some(repo.path())).unwrap();
    };
    receive(&profile);
    assert_eq!(probe(repo.path()).config.agent().unwrap(), "codex:first");
    let first = profile.clone();
    profile.revision += 1;
    profile.definitions[0].values =
        serde_yaml_ng::from_str("default_agent: codex:second\ncontext: {exclude: []}").unwrap();
    receive(&profile);
    receive(&first);
    assert_eq!(probe(repo.path()).config.agent().unwrap(), "codex:second");
    // Body edits do not transfer preference ownership. Preference deletion does.
    write(
        executor.path(),
        "skills/probe.md",
        "---\nchrome: true\nmodel: native-model\n---\nChanged executor body.",
    );
    profile.revision += 1;
    receive(&profile);
    let saved = std::fs::read_to_string(executor.path().join(".lf/skills/probe.md")).unwrap();
    assert!(!saved.contains("default_agent"));
    let receipt = remote::inspect(false, false).unwrap();
    assert!(receipt
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["receipt"]["status"] == "conflict"));
    remote::inspect(true, false).unwrap();
    assert_eq!(probe(repo.path()).config.agent().unwrap(), "codex:second");
    profile.revision += 1;
    profile.definitions.clear();
    receive(&profile);
    let saved = std::fs::read_to_string(executor.path().join(".lf/skills/probe.md")).unwrap();
    assert!(!saved.contains("default_agent"));
    assert!(!saved.contains("exclude"));
    assert!(saved.contains("chrome: true"));
    assert!(saved.contains("model: native-model"));
    assert!(saved.ends_with("Changed executor body."));
}

#[test]
fn unavailable_retention_keeps_forwarded_work_and_never_installs_definitions() {
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let repo = TestRepo::new();
    repo.create_file(".lf/skills/probe.md", "Repository instructions.");
    let origin = tempfile::tempdir().unwrap();
    let executor = tempfile::tempdir().unwrap();
    let profile = {
        let _home = home(origin.path());
        write(
            origin.path(),
            "skills/probe.md",
            "---\nagent: codex:origin\n---\nPrivate body.",
        );
        write(
            origin.path(),
            "graphs/inner.yaml",
            "- step: {name: probe, id: work, default_agent: codex:origin}\n",
        );
        remote::outgoing(Some(repo.path()), "laptop").unwrap()
    };
    let _home = home(executor.path());
    let topology = "# different executor graph\n- step: {name: probe, id: other}\n";
    write(executor.path(), "graphs/inner.yaml", topology);
    let _profile = EnvVarGuard::set(
        remote::PROFILE_ENV,
        &serde_json::to_string(&profile).unwrap(),
    );
    remote::receive(Some(repo.path())).unwrap();
    assert_eq!(probe(repo.path()).config.agent().unwrap(), "codex:origin");
    assert!(!executor.path().join(".lf/skills/probe.md").exists());
    assert_eq!(
        std::fs::read_to_string(executor.path().join(".lf/graphs/inner.yaml")).unwrap(),
        topology
    );
    assert!(remote::inspect(true, false)
        .unwrap_err()
        .to_string()
        .contains("missing incoming occurrences"));
    // Invalid unrelated executor preferences may prevent retention, not forwarded work.
    write(
        executor.path(),
        "skills/unrelated.md",
        "---\nagent: invalid-provider\n---\nUnrelated.",
    );
    remote::receive(Some(repo.path())).unwrap();
    assert_eq!(probe(repo.path()).config.agent().unwrap(), "codex:origin");
}
