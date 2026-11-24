//! The `config` namespace: the typed settings catalogue, what a scope's
//! settings resolve to, and the files that declare them. The CLI (`lf config
//! catalog|settings|get|set|unset|explain`) and the server's `config.*` kinds
//! call these functions; Desktop's Settings and Appearance panes use them
//! through the server.
pub mod promotion;
mod scope;
pub use scope::{scope, Authored, Scope, ScopeFile, ScopeTarget, Slot};

use anyhow::Result;
use serde::Serialize;
use serde_yaml_ng::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::catalog::Definition;
use crate::config::files::{edit, Destination};
use crate::config::settings::{EffectiveSetting, ResolvedSettings};
use crate::graph::Skill;

/// `config.catalog`: every typed setting, its schema and its policies.
pub fn catalog() -> Vec<&'static Definition> {
    crate::config::catalog::definitions()
}

pub fn checked_edit(
    destination: &Destination,
    key: &str,
    expected: &[edit::Entry],
    change: &edit::Change,
) -> Result<edit::Applied> {
    let address = destination.resolve()?;
    validate_edit(&address, key)?;
    edit::Setting::catalogued(&address, key).apply(expected, change)
}

/// Render the exact YAML patch the checked edit would write.
pub fn preview_edit(
    destination: &Destination,
    key: &str,
    expected: &[edit::Entry],
    change: &edit::Change,
) -> Result<String> {
    let address = destination.resolve()?;
    validate_edit(&address, key)?;
    let mut document = edit::Document::read(&address)?;
    edit::Setting::catalogued(&address, key).prepare(&mut document.value, expected, change)?;
    Ok(edit::preview(
        &document.original,
        &crate::config::files::edited_text(&document.original, &document.value)?,
    ))
}

fn validate_edit(address: &crate::config::files::ResolvedDestination, key: &str) -> Result<()> {
    crate::config::settings::validate_scope(key, address.source, address.scope)?;
    let typed = crate::config::keys::typed_key(key);
    let preference = crate::config::settings::preference_path(&typed)?.join(".");
    let definition = crate::config::catalog::definition(&preference).expect("validated key");
    anyhow::ensure!(
        definition.key == preference,
        "select the catalog key {}",
        definition.canonical_key()
    );
    Ok(())
}

/// `config.get`: the declaration of `key` at `destination`, null when none.
/// Every `key` here is a canonical path or one of its shorthands
/// (`config::keys`); the declaration reports the canonical one.
pub fn get(destination: &Destination, key: &str) -> Result<Declaration> {
    declaration(destination, key, None, false)
}

/// `config.set`: declare `key` at `destination`; `value` is YAML.
pub fn set(destination: &Destination, key: &str, value: &str) -> Result<Declaration> {
    declaration(destination, key, Some(value), false)
}

/// `config.unset`: remove `key`'s declaration at `destination`, so other
/// sources choose.
pub fn unset(destination: &Destination, key: &str) -> Result<Declaration> {
    declaration(destination, key, None, true)
}

/// The effective settings of one selection: general, or a skill and its
/// Graph occurrence, each with every source's candidate.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Inspection {
    pub skill: Option<String>,
    pub occurrence: Option<crate::config::settings::OccurrenceSettings>,
    pub settings: BTreeMap<String, EffectiveSetting>,
}

/// What `config.explain` returns: every source's candidate for `key` and the
/// effective value, as a launch at the destination's scope resolves it.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Explanation {
    pub path: PathBuf,
    pub key: String,
    pub inspections: Vec<Inspection>,
}

/// The resolver selectors encoded in a typed setting path before its preference key.
fn target_options<'a>(prefix: &[&'a str]) -> (Option<&'a str>, Option<&'a str>, Option<&'a str>) {
    match prefix {
        ["skills", skill] => (Some(*skill), None, None),
        ["graphs", graph, "steps", step] => (None, Some(*graph), Some(*step)),
        _ => (None, None, None),
    }
}

/// `config.explain`: how `key` resolves at the scope `destination` names.
pub fn explain(destination: &Destination, key: &str) -> Result<Explanation> {
    let destination = destination.resolve()?;
    let key = &crate::config::keys::typed_key(key);
    crate::config::settings::validate_scope(key, destination.source, destination.scope)?;
    let parts: Vec<_> = key.split('.').collect();
    let preference = crate::config::settings::preference_parts(&parts)?;
    let (skill, graph, step) = target_options(&parts[..parts.len() - preference.len()]);
    let resolved = crate::config::resolved(
        destination.repository.as_deref(),
        destination.wave.as_deref(),
    )?;
    let home = crate::store::lf_home_dir();
    let inspections = inspect_selection(
        &resolved,
        destination.repository.as_deref().unwrap_or(&home),
        skill,
        graph,
        step,
    )?;
    // Choices and fallbacks are explained together under their effective key.
    let key = if preference == ["default_agent"] {
        "agent".into()
    } else {
        preference.join(".")
    };
    Ok(Explanation {
        path: destination.path,
        key,
        inspections,
    })
}

/// The same theme declaration read by the phone, with its exact slot for Desktop edits.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ThemeDeclaration {
    #[serde(flatten)]
    pub declaration: Declaration,
    pub entries: Vec<edit::Entry>,
}

/// `config.theme`: read or set the personal global theme.
pub fn theme(value: Option<&str>) -> Result<ThemeDeclaration> {
    let address = Destination::PersonalGlobal {}.resolve()?;
    if let Some(value) = value {
        let parsed = crate::config::settings::parse_value("theme", value)?;
        crate::config::files::edit(&address.path, &["theme"], Some(parsed), address.root())?;
    }
    let raw = crate::config::files::read(&address.path)?;
    let entries = edit::Setting::catalogued(&address, "theme").entries(&raw)?;
    Ok(ThemeDeclaration {
        declaration: Declaration {
            key: "theme".into(),
            value: raw.get("theme").cloned().unwrap_or(Value::Null),
            path: address.path,
            lifecycle: crate::config::catalog::definition("theme")
                .expect("theme is catalogued")
                .lifecycle,
        },
        entries,
    })
}

/// `config.settings`: the effective settings at `directory` for a Wave, a
/// skill or one Graph step, with each value's sources.
pub fn settings(
    directory: &Path,
    wave: Option<&str>,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
) -> Result<Vec<Inspection>> {
    let repo = crate::repo::working_directory_in(directory)?;
    inspect(&repo, wave, skill, graph, step)
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Declaration {
    pub path: PathBuf,
    pub key: String,
    /// The YAML declaration, as JSON.
    #[schemars(with = "serde_json::Value")]
    pub value: Value,
    pub lifecycle: crate::config::catalog::Lifecycle,
}

/// Read, set or unset a declaration at the same destination used by inspection.
fn declaration(
    destination: &Destination,
    key: &str,
    value: Option<&str>,
    unset: bool,
) -> Result<Declaration> {
    let destination = destination.resolve()?;
    let spelled: Vec<_> = key.split('.').collect();
    if let Some(fields) = crate::config::keys::tracker_path(&spelled) {
        crate::config::settings::validate_scope("planning", destination.source, destination.scope)?;
        return tracker_declaration(&destination.path, fields, value, unset);
    }
    let key = &crate::config::keys::typed_key(key);
    crate::config::settings::validate_scope(key, destination.source, destination.scope)?;
    let parsed = value
        .map(|text| destination.parse_value(key, text))
        .transpose()?;
    let mut parts: Vec<_> = destination.prefix.iter().map(String::as_str).collect();
    parts.extend(key.split('.'));
    let root = destination.root();
    let path = &destination.path;
    if parsed.is_some() || unset {
        crate::config::files::edit(path, &parts, parsed, root)?;
    }
    let text = crate::config::files::settings_text(path, root)?;
    let mut document = Value::Mapping(crate::config::reader::parse(&text, path));
    crate::config::keys::fold(&mut document);
    let preference = crate::config::settings::preference_path(key)?;
    if preference.len() > 1 && preference[0] == "context" {
        crate::config::normalize_context(&mut document);
    }
    let value = parts
        .iter()
        .try_fold(&document, |value, part| value.get(*part))
        .cloned()
        .unwrap_or(Value::Null);
    let canonical = match parts.as_slice() {
        ["planning", field] if *field != "provider" => document["planning"]["provider"]
            .as_str()
            .map(|tag| format!("repo.tracker.{tag}.{field}")),
        _ => None,
    };
    Ok(Declaration {
        path: destination.path,
        key: canonical.unwrap_or_else(|| crate::config::keys::canonical(&parts, root).join(".")),
        value,
        lifecycle: crate::config::catalog::definition(&preference.join("."))
            .expect("validated key")
            .lifecycle,
    })
}

fn tracker_declaration(
    path: &Path,
    fields: &[&str],
    value: Option<&str>,
    unset: bool,
) -> Result<Declaration> {
    use crate::config::{files, keys};
    anyhow::ensure!(
        crate::config::catalog::tracker_field(fields),
        "unknown tracker setting {}",
        fields.join(".")
    );
    if value.is_some() || unset {
        files::update(path, |document| {
            keys::gather(document, &["planning"], keys::Root::General);
            keys::edit_tracker(
                document,
                fields,
                value.map(serde_yaml_ng::from_str).transpose()?,
            )
        })?;
    }
    let tracker = keys::tracker_value(&files::read(path)?);
    Ok(Declaration {
        path: path.into(),
        key: [&["repo", "tracker"][..], fields].concat().join("."),
        value: fields
            .iter()
            .try_fold(&tracker, |node, field| node.get(*field))
            .cloned()
            .unwrap_or(Value::Null),
        lifecycle: crate::config::catalog::definition("planning")
            .expect("tracker is catalogued")
            .lifecycle,
    })
}

/// The effective settings in `repo`, as `config.settings` reads them.
pub fn inspect(
    repo: &std::path::Path,
    wave: Option<&str>,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
) -> anyhow::Result<Vec<Inspection>> {
    anyhow::ensure!(
        skill.is_none() || graph.is_none(),
        "choose --skill or --graph"
    );
    anyhow::ensure!(step.is_none() || graph.is_some(), "--step requires --graph");
    let resolved = crate::config::resolved(Some(repo), wave)?;
    inspect_selection(&resolved, repo, skill, graph, step)
}

fn graph_steps(repo: &Path, graph: &str, step: Option<&str>) -> Result<Vec<Skill>> {
    use crate::graph::ConcreteStep;

    fn collect(steps: &[ConcreteStep], target: Option<(&str, &str)>, out: &mut Vec<Skill>) {
        for step in steps {
            let skill = match step {
                ConcreteStep::Skill(skill) => &skill.skill,
                ConcreteStep::Xor(branch) => &branch.router,
                ConcreteStep::Command(_) => continue,
            };
            if target.is_none_or(|(graph, id)| {
                skill.occurrence.as_ref().is_some_and(|occ| {
                    occ.graph == graph
                        && (occ.id.as_deref() == Some(id) || occ.address().as_deref() == Some(id))
                })
            }) {
                out.push(skill.clone());
            }
            if let ConcreteStep::Xor(branch) = step {
                for path in branch.paths.values() {
                    collect(&path.steps, target, out);
                }
            }
        }
    }
    let mut selections = Vec::new();
    let definition = crate::graph::load_authored_graph(graph, repo)?;
    collect(
        &crate::graph::compile_graph(&definition, repo)?,
        step.map(|id| (definition.name.as_str(), id)),
        &mut selections,
    );
    anyhow::ensure!(!selections.is_empty(), "no matching agent steps in {graph}");
    Ok(selections)
}

fn inspect_selection(
    general: &ResolvedSettings,
    repo: &std::path::Path,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
) -> anyhow::Result<Vec<Inspection>> {
    let inspect = |selection: Option<&Skill>| -> Result<Inspection> {
        Ok(Inspection {
            skill: selection.map(|skill| skill.name.clone()),
            occurrence: selection.and_then(|skill| skill.occurrence.as_deref().cloned()),
            settings: match selection {
                Some(skill) => {
                    crate::config::settings::resolve(&general.config, Some(skill))?.settings
                }
                None => general.settings.clone(),
            },
        })
    };
    if let Some(graph) = graph {
        graph_steps(repo, graph, step)?
            .iter()
            .map(|skill| inspect(Some(skill)))
            .collect()
    } else {
        let skill = skill
            .map(|name| crate::graph::load_skill(name, repo))
            .transpose()?;
        Ok(vec![inspect(skill.as_ref())?])
    }
}

#[cfg(test)]
mod tests {
    use super::{explain, get, set, unset};
    use crate::config::environment::EnvVarGuard;
    use crate::config::files::Destination;
    use loopflow_test_support::TestRepo;
    use std::path::Path;

    #[test]
    fn served_scope_sees_each_checked_edit_before_the_next_write() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let cache = crate::observe::Cache::shared();
        let key = "context.diff";
        let read =
            || cache.capture(|| super::scope(None, None, &super::ScopeTarget::General).unwrap());
        let (mut scope, mut dependencies) = read();
        for enabled in [true, false, true] {
            let file = &scope.files[0];
            super::checked_edit(
                &file.destination,
                key,
                &file.slots[key].entries,
                &crate::config::files::edit::Change::Set {
                    value: enabled.to_string(),
                },
            )
            .unwrap();
            assert!(dependencies.changed());
            (scope, dependencies) = read();
            assert_eq!(scope.settings[key].value.as_bool(), Some(enabled));
        }
    }

    #[test]
    fn checked_graph_edits_validate_both_spellings_in_the_checkout() {
        let repo = tempfile::tempdir().unwrap();
        let destination = Destination::SharedRepository {
            repository: repo.path().into(),
        };
        let change = super::edit::Change::Set {
            value: "missing-graph".into(),
        };
        for key in ["wave.graph", "graph"] {
            let error = super::checked_edit(&destination, key, &[], &change).unwrap_err();
            assert!(error.to_string().contains("missing-graph"), "{error}");
        }
        assert!(!destination.resolve().unwrap().path.exists());
    }

    #[test]
    fn catalogue_wire_shape_includes_category_and_unset_meaning() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/configuration_catalog.json"
        ))
        .unwrap();
        let definitions = ["theme", "agent", "default_agent"]
            .map(|key| crate::config::catalog::definition(key).unwrap());
        assert_eq!(serde_json::to_value(definitions).unwrap(), fixture);
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
            let mut invalid = destination.clone();
            invalid["path"] = "/another/config.yaml".into();
            assert!(serde_json::from_value::<Destination>(invalid).is_err());
        }
    }

    fn check(
        destination: &Destination,
        repo: &Path,
        wave: Option<&str>,
        key: &str,
        expected: &str,
    ) {
        let explained = explain(destination, key).unwrap();
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
        assert_eq!(prepared.config.agent.to_string(), expected);
        assert_eq!(explained.inspections[0].settings["agent"].value, expected);
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
        set(&global, "default_agent", "codex:global").unwrap();
        let global_before = std::fs::read(home.path().join("config.yaml")).unwrap();
        set(&shared, "agent", "claude:shared").unwrap();
        let saved = set(&private, "skills.probe.agent", "codex:private").unwrap();
        assert_eq!(
            saved.path,
            repo.path()
                .canonicalize()
                .unwrap()
                .join(".lf/config.local.yaml")
        );
        assert!(!child.join(".lf/config.local.yaml").exists());
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
        set(&other, "skills.probe.agent", "claude:other").unwrap();
        set(&wave, "skills.probe.agent", "claude:wave").unwrap();
        check(
            &wave,
            &child,
            Some("old"),
            "skills.probe.agent",
            "claude:wave",
        );
        let private_before = std::fs::read(repo.path().join(".lf/config.local.yaml")).unwrap();
        assert!(set(&private, "user.name", "Wrong scope")
            .unwrap_err()
            .to_string()
            .contains("does not apply"));
        assert!(set(&shared, "user.name", "Wrong owner").is_err());
        assert_eq!(
            std::fs::read(repo.path().join(".lf/config.local.yaml")).unwrap(),
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
            get(&renamed, "skills.probe.agent").unwrap().value,
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
        unset(&renamed, "skills.probe.agent").unwrap();
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
        let saved = set(&shared_wave, "chrome", "false").unwrap();
        assert_eq!(
            saved.path,
            child
                .canonicalize()
                .unwrap()
                .join(".lf/wave/new/config.yaml")
        );
        set(
            &private,
            "graphs.release.steps.work.agent",
            "claude:occurrence",
        )
        .unwrap();
        check(
            &private,
            &child,
            None,
            "graphs.release.steps.work.agent",
            "claude:occurrence",
        );
        unset(&private, "graphs.release.steps.work.agent").unwrap();
        unset(&private, "skills.probe.agent").unwrap();
        check(
            &private,
            &child,
            None,
            "skills.probe.agent",
            "claude:shared",
        );
        unset(&shared, "agent").unwrap();
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
