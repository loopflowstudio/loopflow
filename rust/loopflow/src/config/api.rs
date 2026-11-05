//! The destination-based editor used by CLI and Desktop.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::path::{Path, PathBuf};

use super::files::Destination;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Catalog,
    Declaration {
        destination: Destination,
        key: String,
        value: Option<String>,
        unset: bool,
    },
    Explain {
        destination: Destination,
        key: String,
    },
    /// Appearance owns its configured theme file and its Apply/Cancel transaction.
    Theme {
        path: PathBuf,
        value: Option<String>,
    },
}

pub fn run(request: Request) -> Result<serde_json::Value> {
    match request {
        Request::Catalog => Ok(serde_json::to_value(super::catalog::definitions())?),
        Request::Declaration {
            destination,
            key,
            value,
            unset,
        } => Ok(serde_json::to_value(declaration(
            &destination,
            &key,
            value.as_deref(),
            unset,
        )?)?),
        Request::Explain { destination, key } => {
            let destination = destination.resolve()?;
            super::settings::validate_scope(&key, destination.source, destination.scope)?;
            let parts: Vec<_> = key.split('.').collect();
            let preference = super::settings::preference_parts(&parts)?;
            let (skill, graph, step) = match &parts[..parts.len() - preference.len()] {
                ["skills", skill] => (Some(*skill), None, None),
                ["graphs", graph, "steps", step] => (None, Some(*graph), Some(*step)),
                _ => (None, None, None),
            };
            let config = super::load_wave_config(
                destination.repository.as_deref(),
                destination.wave.as_deref(),
            )?
            .unwrap_or_default();
            let home = crate::store::lf_home_dir();
            let rows = inspect_selection(
                &config,
                destination.repository.as_deref().unwrap_or(&home),
                skill,
                graph,
                step,
            )?;
            // Choices and fallbacks are explained together under their effective key.
            let effective_key = if preference == ["default_agent"] {
                "agent".into()
            } else {
                preference.join(".")
            };
            Ok(
                serde_json::json!({"path":destination.path, "key":effective_key, "inspections":rows}),
            )
        }
        Request::Theme { path, value } => Ok(serde_json::to_value(file_value(
            &path,
            &["theme"],
            "theme",
            value.as_deref(),
            false,
        )?)?),
    }
}

#[derive(Debug, Serialize)]
pub struct Declaration {
    pub path: PathBuf,
    pub key: String,
    pub value: Value,
    pub lifecycle: super::catalog::Lifecycle,
}

/// Read, set or unset a declaration at the same destination used by inspection.
pub fn declaration(
    destination: &Destination,
    key: &str,
    value: Option<&str>,
    unset: bool,
) -> Result<Declaration> {
    anyhow::ensure!(!(value.is_some() && unset), "choose a value or unset");
    let destination = destination.resolve()?;
    super::settings::validate_scope(key, destination.source, destination.scope)?;
    if let ("graph", Some(value), Some(repository)) =
        (key, value, destination.repository.as_deref())
    {
        let parsed = super::settings::parse_value(key, value)?;
        if let Some(name) = parsed.as_str() {
            let graph = crate::graph::load_authored_graph(name, repository)?;
            crate::graph::compile_graph(&graph, repository)?;
        }
    }
    let mut parts: Vec<_> = destination.prefix.iter().map(String::as_str).collect();
    parts.extend(key.split('.'));
    file_value(&destination.path, &parts, key, value, unset)
}

fn file_value(
    path: &Path,
    parts: &[&str],
    key: &str,
    value: Option<&str>,
    unset: bool,
) -> Result<Declaration> {
    let parsed = value
        .map(|value| super::settings::parse_value(key, value))
        .transpose()?;
    if parsed.is_some() || unset {
        super::files::edit(path, parts, parsed)?;
    }
    let document = super::files::read(path)?;
    let value = parts
        .iter()
        .try_fold(&document, |value, part| value.get(*part))
        .cloned()
        .unwrap_or(Value::Null);
    Ok(Declaration {
        path: path.to_owned(),
        key: parts.join("."),
        value,
        lifecycle: super::catalog::definition(&super::settings::preference_path(key)?.join("."))
            .expect("validated key")
            .lifecycle,
    })
}

/// The CLI and served Desktop read share this inspection.
pub fn inspect(
    repo: &std::path::Path,
    wave: Option<&str>,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    anyhow::ensure!(
        skill.is_none() || graph.is_none(),
        "choose --skill or --graph"
    );
    anyhow::ensure!(step.is_none() || graph.is_some(), "--step requires --graph");
    let config = crate::config::load_wave_config(Some(repo), wave)?.unwrap_or_default();
    inspect_selection(&config, repo, skill, graph, step)
}

fn inspect_selection(
    config: &crate::config::Config,
    repo: &std::path::Path,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut selections = Vec::new();
    if let Some(graph) = graph {
        fn collect(
            steps: &[crate::graph::ConcreteStep],
            target: Option<(&str, &str)>,
            out: &mut Vec<crate::graph::Skill>,
        ) {
            for step in steps {
                match step {
                    crate::graph::ConcreteStep::Skill(skill)
                        if target.is_none_or(|(graph, id)| {
                            skill.skill.occurrence.as_ref().is_some_and(|occurrence| {
                                occurrence.graph == graph && occurrence.id.as_deref() == Some(id)
                            })
                        }) =>
                    {
                        out.push(skill.skill.clone())
                    }
                    crate::graph::ConcreteStep::Xor(branch) => {
                        if target.is_none() {
                            out.push(branch.router.clone());
                        }
                        for path in branch.paths.values() {
                            collect(&path.steps, target, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        let definition = crate::graph::load_authored_graph(graph, repo)?;
        collect(
            &crate::graph::compile_graph(&definition, repo)?,
            step.map(|id| (definition.name.as_str(), id)),
            &mut selections,
        );
        anyhow::ensure!(!selections.is_empty(), "no matching agent steps in {graph}");
    } else if let Some(skill) = skill {
        selections.push(crate::graph::load_skill(skill, repo)?);
    }
    let mut output = Vec::new();
    for selection in if selections.is_empty() {
        vec![None]
    } else {
        selections.iter().map(Some).collect()
    } {
        let resolved = crate::config::settings::resolve(config, selection)?;
        output.push(serde_json::json!({"skill": selection.map(|s| &s.name), "occurrence": selection.and_then(|s| s.occurrence.as_ref()), "settings": resolved.settings}));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{run, Request};
    use crate::config::files::Destination;
    use crate::lf::commands::graph::EnvVarGuard;
    use loopflow_test_support::TestRepo;
    use serde_json::{json, Value};
    use std::path::Path;

    #[test]
    fn catalogue_wire_shape_includes_feature_category() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/configuration_catalog.json"
        ))
        .unwrap();
        let theme = crate::config::catalog::definition("theme").unwrap();
        assert_eq!(serde_json::to_value([theme]).unwrap(), fixture);
    }

    #[test]
    fn destination_wire_shape_cannot_supply_independent_file_or_scope() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/configuration_destinations.json"
        ))
        .unwrap();
        let destinations: Vec<Destination> = serde_json::from_value(fixture.clone()).unwrap();
        assert_eq!(serde_json::to_value(destinations).unwrap(), fixture);
        for destination in fixture.as_array().unwrap() {
            let mut request = serde_json::json!({"action":"declaration", "destination":destination,
                "key":"agent", "value":"codex", "unset":false});
            assert!(serde_json::from_value::<Request>(request.clone()).is_ok());
            request["scope"] = "global".into();
            assert!(serde_json::from_value::<Request>(request).is_err());
            let mut invalid = destination.clone();
            invalid["path"] = "/another/config.yaml".into();
            assert!(serde_json::from_value::<Destination>(invalid).is_err());
        }
    }

    fn edit(
        destination: &Destination,
        key: &str,
        value: Option<&str>,
        unset: bool,
    ) -> anyhow::Result<Value> {
        run(Request::Declaration {
            destination: destination.clone(),
            key: key.into(),
            value: value.map(str::to_owned),
            unset,
        })
    }

    fn check(
        destination: &Destination,
        repo: &Path,
        wave: Option<&str>,
        key: &str,
        expected: &str,
    ) {
        let explained = run(Request::Explain {
            destination: destination.clone(),
            key: key.into(),
        })
        .unwrap();
        let config = crate::config::load_wave_config(Some(repo), wave)
            .unwrap()
            .unwrap();
        let skill = if key.starts_with("graphs.") {
            let graph = crate::graph::load_authored_graph("release", repo).unwrap();
            let steps = crate::graph::compile_graph(&graph, repo).unwrap();
            let crate::graph::ConcreteStep::Skill(step) = &steps[0] else {
                panic!("expected skill")
            };
            step.skill.clone()
        } else {
            crate::graph::load_skill("probe", repo).unwrap()
        };
        let prepared = crate::prompt::process::prepare_process_prompt(
            &config,
            crate::prompt::process::ProcessPromptInput {
                repo_root: repo.to_owned(),
                wave: wave.map(str::to_owned),
                resolved_skill: Some(skill),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(prepared.config.agent().unwrap(), expected);
        assert_eq!(
            explained["inspections"][0]["settings"]["agent"]["value"],
            expected
        );
    }

    #[test]
    fn destinations_bind_save_explanation_and_launch_across_worktrees_and_wave_renames() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = TestRepo::new();
        repo.create_file(".lf/skills/probe.md", "Probe settings.\n");
        repo.create_file(
            ".lf/graphs/release.yaml",
            "- step: {name: probe, id: work}\n",
        );
        let id = "8ae21a68-8940-4553-a3ab-135133925e35";
        repo.create_file(".lf/wave/old/config.yaml", &format!("id: {id}\n"));
        repo.stage_all();
        repo.commit("Authored fixture");
        let child = home.path().join("child");
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "-b", "child"])
            .arg(&child)
            .output()
            .unwrap()
            .status
            .success());
        let global = Destination::PersonalGlobal {};
        let private = Destination::PersonalRepository {
            repository: child.clone(),
        };
        let shared = Destination::SharedRepository {
            repository: child.clone(),
        };
        let wave = Destination::PersonalWave {
            repository: child.clone(),
            wave: "old".into(),
        };
        let other = Destination::PersonalWave {
            repository: child.clone(),
            wave: "different".into(),
        };
        std::fs::create_dir_all(child.join(".lf/wave/different")).unwrap();
        std::fs::write(
            child.join(".lf/wave/different/config.yaml"),
            "id: 9ae21a68-8940-4553-a3ab-135133925e35\n",
        )
        .unwrap();
        edit(&global, "default_agent", Some("codex:global"), false).unwrap();
        let global_before = std::fs::read(home.path().join("config.yaml")).unwrap();
        edit(&shared, "agent", Some("claude:shared"), false).unwrap();
        let saved = edit(&private, "skills.probe.agent", Some("codex:private"), false).unwrap();
        assert_eq!(
            saved["path"],
            json!(repo
                .path()
                .canonicalize()
                .unwrap()
                .join("lf.config.local.yaml"))
        );
        assert!(!child.join("lf.config.local.yaml").exists());
        check(
            &private,
            &child,
            None,
            "skills.probe.agent",
            "codex:private",
        );
        check(
            &private,
            repo.path(),
            None,
            "skills.probe.agent",
            "codex:private",
        );
        edit(&other, "skills.probe.agent", Some("claude:other"), false).unwrap();
        edit(&wave, "skills.probe.agent", Some("claude:wave"), false).unwrap();
        check(
            &wave,
            &child,
            Some("old"),
            "skills.probe.agent",
            "claude:wave",
        );
        let private_before = std::fs::read(repo.path().join("lf.config.local.yaml")).unwrap();
        assert!(edit(&private, "user.name", Some("Wrong scope"), false)
            .unwrap_err()
            .to_string()
            .contains("does not apply"));
        assert!(edit(&shared, "user.name", Some("Wrong owner"), false).is_err());
        assert_eq!(
            std::fs::read(repo.path().join("lf.config.local.yaml")).unwrap(),
            private_before
        );
        std::fs::rename(child.join(".lf/wave/old"), child.join(".lf/wave/new")).unwrap();
        std::fs::create_dir_all(child.join(".lf/wave/old")).unwrap();
        std::fs::write(
            child.join(".lf/wave/old/config.yaml"),
            "id: 7ae21a68-8940-4553-a3ab-135133925e35\n",
        )
        .unwrap();
        let renamed = Destination::PersonalWave {
            repository: child.clone(),
            wave: id.into(),
        };
        assert_eq!(
            edit(&renamed, "skills.probe.agent", None, false).unwrap()["value"],
            "claude:wave"
        );
        check(
            &renamed,
            &child,
            Some("new"),
            "skills.probe.agent",
            "claude:wave",
        );
        check(
            &wave,
            &child,
            Some("old"),
            "skills.probe.agent",
            "codex:private",
        );
        edit(&renamed, "skills.probe.agent", None, true).unwrap();
        check(
            &renamed,
            &child,
            Some("new"),
            "skills.probe.agent",
            "codex:private",
        );
        check(
            &other,
            &child,
            Some("different"),
            "skills.probe.agent",
            "claude:other",
        );
        let shared_wave = Destination::SharedWave {
            repository: child.clone(),
            wave: id.into(),
        };
        let saved = edit(&shared_wave, "chrome", Some("false"), false).unwrap();
        assert_eq!(
            saved["path"],
            json!(child
                .canonicalize()
                .unwrap()
                .join(".lf/wave/new/config.yaml"))
        );
        edit(
            &private,
            "graphs.release.steps.work.agent",
            Some("claude:occurrence"),
            false,
        )
        .unwrap();
        check(
            &private,
            &child,
            None,
            "graphs.release.steps.work.agent",
            "claude:occurrence",
        );
        edit(&private, "graphs.release.steps.work.agent", None, true).unwrap();
        edit(&private, "skills.probe.agent", None, true).unwrap();
        check(
            &private,
            &child,
            None,
            "skills.probe.agent",
            "claude:shared",
        );
        edit(&shared, "agent", None, true).unwrap();
        check(
            &private,
            &child,
            None,
            "skills.probe.default_agent",
            "codex:global",
        );
        assert_eq!(
            std::fs::read(home.path().join("config.yaml")).unwrap(),
            global_before
        );
        assert!(!home.path().join("loopflow.db").exists());
        let clone = TestRepo::new();
        assert_eq!(
            crate::config::load_config(Some(clone.path()))
                .unwrap()
                .unwrap()
                .agent()
                .unwrap(),
            "codex:global"
        );
    }
}
