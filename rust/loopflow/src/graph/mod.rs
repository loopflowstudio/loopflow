pub mod compiled;
pub mod instructions;
pub mod output;
pub mod runner;
pub mod transitions;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::definition::name::{portable_name, resolve_name};
use crate::definition::{resolve_definition, DefinitionKind, Target};
use crate::error::LoadError;
use crate::skills::catalog::{SkillCatalog, SkillOrigin};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SkillOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_style: Option<String>,
    /// Docs the skill includes, resolved like `--docs`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl Skill {
    pub fn source_text(&self) -> String {
        let body = self.content.as_deref().unwrap_or_default();
        match self
            .source
            .as_ref()
            .and_then(|source| source.frontmatter.as_ref())
        {
            Some(frontmatter) => format!("---{frontmatter}---\n{body}"),
            None => body.to_string(),
        }
    }

    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            source: None,
            agent: None,
            default_agent: None,
            action_style: None,
            docs: Vec::new(),
            content: None,
        }
    }
}

/// The skill a `loop:` item runs when it names none.
const DEFAULT_DECIDER: &str = "loop-or-next";

fn validate_step_settings(id: Option<&str>) -> Result<(), LoadError> {
    if id.is_some_and(|id| id.trim().is_empty()) {
        return Err(LoadError::InvalidGraph(
            "step id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

/// An executable occurrence. Template names resolve to Target during loading;
/// interactive and loop settings belong to this occurrence, not to the definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Step {
    pub target: Target,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// An interactive step: a person takes part here, in the Task conversation. Written
    /// `interactive: <skill>`.
    #[serde(default, alias = "human", skip_serializing_if = "std::ops::Not::not")]
    pub interactive: bool,
    /// A deciding occurrence's authored target: the occurrence name or skill
    /// of an earlier step it can return to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
}

impl Step {
    pub fn new(target: Target) -> Self {
        Self {
            target,
            id: None,
            interactive: false,
            returns: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Command {
    pub command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

// Authored and saved invocations outlive CLI spellings. Both take the current
// operation owner, keeping their arguments and topology, so a fresh Graph and a
// retained Graph record the same step.
impl<'de> Deserialize<'de> for Command {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct SavedCommand {
            command: String,
            #[serde(default)]
            args: Vec<String>,
        }
        let saved = SavedCommand::deserialize(deserializer)?;
        Ok(Self {
            command: saved.command,
            args: saved.args,
        }
        .current())
    }
}

impl Command {
    /// This command under the spelling the CLI accepts today.
    pub fn current(mut self) -> Self {
        if self.command == "rebase" {
            self.command = "sync".into();
        }
        if self.command == "task"
            && self
                .args
                .first()
                .is_some_and(|name| matches!(name.as_str(), "pr" | "wt" | "sync" | "commit"))
        {
            self.command = self.args.remove(0);
        }
        // `lf repo` dissolved; its commands keep their names under new owners.
        if self.command == "repo" && !self.args.is_empty() {
            self.command = self.args.remove(0);
        }
        // Captured commands keep their arguments, but use the current owner.
        if self.command == "monitor"
            && self
                .args
                .first()
                .is_some_and(|name| matches!(name.as_str(), "list" | "show" | "usage" | "activity"))
        {
            self.command = "history".into();
            if self.args.first().is_some_and(|name| name == "activity") {
                self.args.remove(0);
            }
        } else if self.command == "activity" {
            self.command = "history".into();
        }
        let owner = match self.command.as_str() {
            "doctor" | "install" => Some("self"),
            "ps" | "top" => Some("monitor"),
            "usage" | "replay" => Some("history"),
            "release" | "ci" | "tokens" => Some("ops"),
            _ => None,
        };
        if let Some(owner) = owner {
            self.args.insert(0, self.command);
            self.command = owner.into();
        }
        self
    }

    pub fn argv(&self) -> Vec<String> {
        std::iter::once("lf".to_string())
            .chain(std::iter::once(self.command.clone()))
            .chain(self.args.iter().cloned())
            .collect()
    }

    pub fn display_name(&self) -> String {
        if self.args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.args.join(" "))
        }
    }
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd: {}", self.display_name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XorDef {
    /// Optional router skill. If set, this skill runs first and writes the
    /// verdict. Path descriptions are appended to the skill's prompt as routing
    /// instructions. If absent, a generic routing agent is used.
    pub router: Option<String>,
    pub paths: HashMap<String, XorPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XorPath {
    pub steps: Vec<Step>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphDefinition {
    pub name: String,
    pub items: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConcreteSkill {
    pub skill: Skill,
    pub id: Option<String>,
    /// An interactive step, where a person takes part; its id is `id`, else the skill name.
    #[serde(alias = "human")]
    pub interactive: bool,
    /// A deciding occurrence returns this many steps back in its own body.
    pub returns: Option<usize>,
    pub sources: Vec<String>,
}

impl ConcreteSkill {
    /// The interactive step this occurrence is, when a person takes part at it.
    pub fn interactive_step(&self) -> Option<&str> {
        self.interactive
            .then(|| self.id.as_deref().unwrap_or(&self.skill.name))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConcreteXor {
    pub router: Skill,
    pub paths: HashMap<String, ConcretePath>,
    pub sources: Vec<String>,
}

/// A captured branch body; unresolved authored paths cannot decode as this type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConcretePath {
    pub description: String,
    pub steps: Vec<ConcreteStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConcreteCommand {
    pub item: Command,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConcreteStep {
    Skill(ConcreteSkill),
    Command(ConcreteCommand),
    Xor(ConcreteXor),
}

/// Resolved template composition. Flattening is the sole execution expansion.
#[derive(Debug, Clone, Serialize)]
pub(crate) enum ResolvedGraphItem {
    Skill(ConcreteSkill),
    Command(ConcreteCommand),
    Group {
        name: String,
        items: Vec<ResolvedGraphItem>,
    },
    Xor {
        router: Skill,
        paths: BTreeMap<String, ResolvedGraphPath>,
        sources: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolvedGraphPath {
    pub description: String,
    pub items: Vec<ResolvedGraphItem>,
}

pub(crate) fn flatten_resolved(items: &[ResolvedGraphItem]) -> Vec<ConcreteStep> {
    items
        .iter()
        .flat_map(|item| match item {
            ResolvedGraphItem::Skill(skill) => vec![ConcreteStep::Skill(skill.clone())],
            ResolvedGraphItem::Command(op) => vec![ConcreteStep::Command(op.clone())],
            ResolvedGraphItem::Group { items, .. } => flatten_resolved(items),
            ResolvedGraphItem::Xor {
                router,
                paths,
                sources,
            } => vec![ConcreteStep::Xor(ConcreteXor {
                router: router.clone(),
                sources: sources.clone(),
                paths: paths
                    .iter()
                    .map(|(name, path)| {
                        (
                            name.clone(),
                            ConcretePath {
                                description: path.description.clone(),
                                steps: flatten_resolved(&path.items),
                            },
                        )
                    })
                    .collect(),
            })],
        })
        .collect()
}

pub fn load_graph(name: &str, repo: &Path) -> Result<GraphDefinition, LoadError> {
    resolve_definition(repo, name, None).map(Target::into_graph)
}

/// Copy a builtin Graph only when the repository has no local definition.
pub fn customize(name: &str, repo: &Path) -> Result<PathBuf, LoadError> {
    if let Some(path) = find_graph_source_path(name, repo)? {
        return Ok(path);
    }
    let name = crate::builtins::resolve_builtin_graph(name)
        .ok_or_else(|| LoadError::GraphNotFound(name.to_string()))?;
    let content = crate::builtins::get_builtin_graph(name)
        .ok_or_else(|| LoadError::GraphNotFound(name.to_string()))?;
    let path = repo.join(format!(".lf/graphs/{name}.yaml"));
    std::fs::create_dir_all(path.parent().expect("definition has a parent"))?;
    std::fs::write(&path, content)?;
    Ok(path)
}

pub fn available_graph_names(repo: &Path) -> Result<Vec<String>, LoadError> {
    let mut names: Vec<String> = crate::builtins::builtin_graph_names()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    names.extend(repo_graph_names(repo)?);
    names.sort();
    names.dedup();
    Ok(names)
}

pub(crate) fn repo_graph_names(repo: &Path) -> Result<Vec<String>, LoadError> {
    Ok(repo_graph_sources(repo)?.into_keys().collect())
}

/// Load an authored graph without adapting a skill into a graph.
pub fn load_authored_graph(name: &str, repo: &Path) -> Result<GraphDefinition, LoadError> {
    let catalog = SkillCatalog::discover(Some(repo))?;
    DefinitionLoader::new(Some(repo), &catalog).load_graph(name)
}

/// Retains the current composition path while resolving nested definitions.
#[derive(Debug)]
pub(crate) struct DefinitionLoader<'a> {
    repo: Option<&'a Path>,
    catalog: &'a SkillCatalog,
    sources: Vec<String>,
}

impl<'a> DefinitionLoader<'a> {
    pub(crate) fn new(repo: Option<&'a Path>, catalog: &'a SkillCatalog) -> Self {
        Self {
            repo,
            catalog,
            sources: Vec::new(),
        }
    }

    pub(crate) fn resolve(
        &mut self,
        name: &str,
        kind: Option<DefinitionKind>,
    ) -> Result<Target, LoadError> {
        if kind.is_none() {
            let catalog = self.catalog;
            let graphs = match self.repo {
                Some(repo) => available_graph_names(repo)?,
                None => crate::builtins::builtin_graph_names()
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            };
            let names = graphs
                .iter()
                .map(String::as_str)
                .chain(catalog.entries().map(|source| source.name.as_str()));
            if let Some(selected) = resolve_name(name, names).map_err(|names| {
                LoadError::InvalidGraph(format!(
                    "ambiguous definition {name:?}: {}",
                    names.join(", ")
                ))
            })? {
                // Kind precedence applies only after selecting the literal identity.
                return if graphs.iter().any(|name| name == selected) {
                    self.load_graph(selected).map(Target::Graph)
                } else {
                    catalog
                        .exact(selected)
                        .expect("selected definition is a skill")
                        .load()
                        .map(Target::Skill)
                };
            }
        }
        if kind != Some(DefinitionKind::Skill) {
            match self.load_graph(name) {
                Ok(graph) => return Ok(Target::Graph(graph)),
                Err(LoadError::GraphNotFound(_)) if kind.is_none() => {}
                Err(error) => return Err(error),
            }
        }
        match self.catalog.load_skill(name) {
            Ok(skill) => Ok(Target::Skill(skill)),
            Err(LoadError::SkillNotFound(_)) if kind.is_none() => {
                Err(LoadError::TargetNotFound(name.to_string()))
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn load_graph(&mut self, name: &str) -> Result<GraphDefinition, LoadError> {
        let (resolved_name, content) = match self
            .repo
            .map(|repo| find_repo_graph(name, repo))
            .transpose()?
            .flatten()
        {
            Some((key, path)) => (key, fs::read_to_string(path)?),
            None => {
                let key = crate::builtins::resolve_builtin_graph(name)
                    .ok_or_else(|| LoadError::GraphNotFound(name.to_string()))?;
                let content =
                    crate::builtins::get_builtin_graph(key).expect("resolved builtin graph exists");
                (key.to_string(), content.to_string())
            }
        };
        self.parse_source(resolved_name, &content)
    }

    /// Parse authored Graph text as the definition named `name`.
    fn parse_source(&mut self, name: String, content: &str) -> Result<GraphDefinition, LoadError> {
        let value: Value = serde_yaml_ng::from_str(content)
            .map_err(|err| LoadError::InvalidGraph(err.to_string()))?;
        validate_graph_nesting(&self.sources, &name)?;
        self.sources.push(name.clone());
        let items = parse_graph_items(&value, self);
        self.sources.pop();
        // The outer definition exists. A missing child must not trigger
        // top-level fallback to a same-named local or cached skill.
        let items = items.map_err(|error| LoadError::InvalidGraph(format!("{name}: {error}")))?;
        Ok(GraphDefinition { name, items })
    }
}

pub fn compile_graph(graph: &GraphDefinition, repo: &Path) -> Result<Vec<ConcreteStep>, LoadError> {
    Ok(flatten_resolved(&resolve_graph(graph, repo)?))
}

pub(crate) fn compile_graph_with_catalog(
    graph: &GraphDefinition,
    catalog: &SkillCatalog,
) -> Result<Vec<ConcreteStep>, LoadError> {
    Ok(flatten_resolved(&resolve_graph_with_catalog(
        graph, catalog,
    )?))
}

pub(crate) fn resolve_graph(
    graph: &GraphDefinition,
    repo: &Path,
) -> Result<Vec<ResolvedGraphItem>, LoadError> {
    let catalog = SkillCatalog::discover(Some(repo))?;
    resolve_graph_with_catalog(graph, &catalog)
}

fn resolve_graph_with_catalog(
    graph: &GraphDefinition,
    catalog: &SkillCatalog,
) -> Result<Vec<ResolvedGraphItem>, LoadError> {
    let resolved = compile_with_sources(graph, catalog, &[])?;
    let items = flatten_resolved(&resolved);
    let mut ids = HashSet::new();
    validate_occurrence_ids(&items, &mut ids)?;
    validate_interactive_steps(&items, &mut HashSet::new())?;
    Ok(resolved)
}

/// Every interactive step has its own id, and no autonomous loop returns across one: a
/// person decides there, so nothing automatic may run back over it.
fn validate_interactive_steps(
    items: &[ConcreteStep],
    steps: &mut HashSet<String>,
) -> Result<(), LoadError> {
    for (index, item) in items.iter().enumerate() {
        match item {
            ConcreteStep::Skill(skill) => {
                if let Some(step) = skill.interactive_step() {
                    if !steps.insert(step.to_string()) {
                        return Err(LoadError::InvalidGraph(format!(
                            "two interactive steps are named {step:?}; give one `interactive: {{name: {}, id: <step>}}`",
                            skill.skill.name
                        )));
                    }
                } else if let Some(target) = return_target(items, index) {
                    if let Some(crossed) = items[target..index].iter().find_map(|step| match step {
                        ConcreteStep::Skill(step) => step.interactive_step(),
                        _ => None,
                    }) {
                        return Err(LoadError::InvalidGraph(format!(
                            "loop at {} returns across interactive step {crossed:?}; a person decides there",
                            skill.skill.name
                        )));
                    }
                }
            }
            ConcreteStep::Xor(branch) => {
                for path in branch.paths.values() {
                    validate_interactive_steps(&path.steps, steps)?;
                }
            }
            ConcreteStep::Command(_) => {}
        }
    }
    Ok(())
}

/// Steps back from the end of `preceding` to the one occurrence `target`
/// names: an occurrence id, else the only step running that skill, else the
/// first step of the one composed graph with that name.
fn resolve_loop_target(preceding: &[ConcreteStep], target: &str) -> Result<usize, LoadError> {
    let positions = |names: &dyn Fn(&ConcreteSkill) -> bool| -> Vec<usize> {
        preceding
            .iter()
            .enumerate()
            .filter(|(_, step)| matches!(step, ConcreteStep::Skill(skill) if names(skill)))
            .map(|(index, _)| index)
            .collect()
    };
    let mut found = positions(&|skill| skill.id.as_deref() == Some(target));
    if found.is_empty() {
        found = positions(&|skill| skill.skill.name == target);
    }
    if found.is_empty() {
        let target_key = portable_name(target);
        found = positions(&|skill| portable_name(&skill.skill.name) == target_key);
    }
    if found.is_empty() {
        // A composed graph begins where its name first appears among a step's sources.
        let composed = |step: &ConcreteStep| {
            let sources = match step {
                ConcreteStep::Skill(skill) => &skill.sources,
                ConcreteStep::Command(command) => &command.sources,
                ConcreteStep::Xor(branch) => &branch.sources,
            };
            sources.iter().skip(1).any(|source| source == target)
        };
        found = preceding
            .iter()
            .enumerate()
            .filter(|(index, step)| {
                composed(step) && (*index == 0 || !composed(&preceding[index - 1]))
            })
            .map(|(index, _)| index)
            .collect();
    }
    match found[..] {
        [index] => Ok(preceding.len() - index),
        [] => Err(LoadError::InvalidGraph(format!(
            "loop target {target:?} must name a preceding step"
        ))),
        _ => Err(LoadError::InvalidGraph(format!(
            "loop target {target:?} is ambiguous: steps {} of {} preceding all run it; give one an id and name that",
            found
                .iter()
                .map(|index| (index + 1).to_string())
                .collect::<Vec<_>>()
                .join(", "),
            preceding.len()
        ))),
    }
}

/// Index of the earlier step the deciding skill at `index` returns to.
pub(crate) fn return_target(steps: &[ConcreteStep], index: usize) -> Option<usize> {
    let ConcreteStep::Skill(skill) = steps.get(index)? else {
        return None;
    };
    index
        .checked_sub(skill.returns?)
        .filter(|target| *target < index)
}

fn validate_occurrence_ids(
    items: &[ConcreteStep],
    ids: &mut HashSet<String>,
) -> Result<(), LoadError> {
    for item in items {
        match item {
            ConcreteStep::Skill(skill) => {
                if let Some(id) = skill.id.as_ref() {
                    if !ids.insert(id.clone()) {
                        return Err(LoadError::InvalidGraph(format!(
                            "graph occurrence id {id:?} is not unique after expansion"
                        )));
                    }
                }
            }
            ConcreteStep::Xor(branch) => {
                for path in branch.paths.values() {
                    validate_occurrence_ids(&path.steps, ids)?;
                }
            }
            ConcreteStep::Command(_) => {}
        }
    }
    Ok(())
}

pub fn load_skill(name: &str, repo: &Path) -> Result<Skill, LoadError> {
    SkillCatalog::discover(Some(repo))?.load_skill(name)
}

pub(crate) fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    if !content.starts_with("---") {
        return None;
    }
    let mut parts = content.splitn(3, "---");
    let _ = parts.next();
    let frontmatter = parts.next()?;
    let rest = parts.next()?;
    let body = rest.strip_prefix('\n').unwrap_or(rest);
    Some((frontmatter, body))
}

fn repo_graph_sources(repo: &Path) -> Result<BTreeMap<String, PathBuf>, LoadError> {
    let root = repo.join(".lf/graphs");
    let mut sources = BTreeMap::<String, PathBuf>::new();
    if !root.exists() {
        return Ok(sources);
    }
    for entry in ignore::WalkBuilder::new(&root)
        .standard_filters(false)
        .follow_links(false)
        .sort_by_file_path(|a, b| a.cmp(b))
        .build()
    {
        let entry = entry.map_err(|error| LoadError::InvalidGraph(error.to_string()))?;
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml" || ext == "json")
        {
            continue;
        }
        let name = path
            .strip_prefix(&root)
            .expect("source under root")
            .with_extension("")
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        if let Some(previous) = sources.get(&name) {
            if fs::canonicalize(previous)? == fs::canonicalize(path)? {
                continue;
            }
            return Err(LoadError::InvalidGraph(format!(
                "ambiguous graph {name:?}: {} and {}",
                previous.display(),
                path.display()
            )));
        }
        sources.insert(name, path.to_path_buf());
    }
    Ok(sources)
}

pub fn find_graph_source_path(name: &str, repo: &Path) -> Result<Option<PathBuf>, LoadError> {
    Ok(find_repo_graph(name, repo)?.map(|(_, path)| path))
}

fn find_repo_graph(name: &str, repo: &Path) -> Result<Option<(String, PathBuf)>, LoadError> {
    let mut sources = repo_graph_sources(repo)?;
    let names = sources
        .keys()
        .map(String::as_str)
        .chain(crate::builtins::builtin_graph_names());
    let selected = resolve_name(name, names)
        .map_err(|names| {
            LoadError::InvalidGraph(format!("ambiguous graph {name:?}: {}", names.join(", ")))
        })?
        .map(str::to_string)
        .or_else(|| crate::builtins::resolve_builtin_graph(name).map(str::to_string));
    Ok(selected.and_then(|name| sources.remove_entry(&name)))
}

// -----------------------------------------------------------------------------
// YAML parsing helpers
// -----------------------------------------------------------------------------

fn key(s: &str) -> Value {
    Value::String(s.to_string())
}

fn parse_graph_items(
    value: &Value,
    loader: &mut DefinitionLoader<'_>,
) -> Result<Vec<Step>, LoadError> {
    match value {
        Value::Sequence(seq) => seq
            .iter()
            .map(|item| parse_graph_item(item, loader))
            .collect(),
        Value::Mapping(map) => {
            if let Some(skills) = map.get(key("steps")) {
                return parse_graph_items(skills, loader);
            }
            Err(LoadError::InvalidGraph(
                "graph root must be a list".to_string(),
            ))
        }
        _ => Err(LoadError::InvalidGraph(
            "graph root must be a list".to_string(),
        )),
    }
}

fn parse_graph_item(value: &Value, loader: &mut DefinitionLoader<'_>) -> Result<Step, LoadError> {
    match value {
        Value::String(name) => Ok(Step::new(loader.resolve(name, None)?)),
        Value::Mapping(map) => parse_graph_mapping(map, loader),
        _ => Err(LoadError::InvalidGraph(
            "graph item must be string or mapping".to_string(),
        )),
    }
}

fn parse_graph_mapping(
    map: &serde_yaml_ng::Mapping,
    loader: &mut DefinitionLoader<'_>,
) -> Result<Step, LoadError> {
    if let Some(target) = map.get(key("loop")) {
        return parse_loop(map, target);
    }
    if let Some(skill_value) = map.get(key("step")) {
        if map.len() != 1 {
            return Err(LoadError::InvalidGraph(
                "step metadata belongs inside the step mapping".to_string(),
            ));
        }
        return parse_skill_value(skill_value, false);
    }
    if let Some(skill_value) = map.get(key("interactive")) {
        if map.len() != 1 {
            return Err(LoadError::InvalidGraph(
                "step metadata belongs inside the interactive mapping: `interactive: {name, id, agent}`"
                    .to_string(),
            ));
        }
        return parse_skill_value(skill_value, true);
    }
    if map.contains_key(key("human")) {
        return Err(LoadError::InvalidGraph(
            "`human:` is retired; write `interactive: <skill>`".to_string(),
        ));
    }
    if map.contains_key(key("id")) {
        return Err(LoadError::InvalidGraph(
            "occurrence metadata are valid only on skill nodes".to_string(),
        ));
    }
    if let Some(graph_value) = map.get(key("graph")) {
        return parse_named_target(graph_value, loader);
    }
    if let Some(command_value) = map.get(key("cmd")) {
        return parse_command_value(command_value);
    }
    if let Some(xor_value) = map.get(key("xor")) {
        return parse_xor_value(xor_value, loader);
    }
    Err(LoadError::InvalidGraph(
        "graph item mapping must include step, interactive, loop, cmd, graph, or xor".to_string(),
    ))
}

fn parse_command_value(value: &Value) -> Result<Step, LoadError> {
    let raw = value
        .as_str()
        .ok_or_else(|| LoadError::InvalidGraph("cmd value must be string".into()))?;
    let mut parts = raw.split_whitespace();
    let command = parts
        .next()
        .ok_or_else(|| LoadError::InvalidGraph("cmd value must include a command".into()))?
        .to_string();
    let args = parts.map(ToString::to_string).collect();

    Ok(Step::new(Target::Command(
        Command { command, args }.current(),
    )))
}

/// `loop: <target>` runs a deciding skill last, `step`, `interactive` or
/// `loop-or-next`, which advances, returns to the target or stops blocked. An
/// interactive decider is an interactive step: the person there decides.
fn parse_loop(map: &serde_yaml_ng::Mapping, target: &Value) -> Result<Step, LoadError> {
    let target = target
        .as_str()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .ok_or_else(|| LoadError::InvalidGraph("loop must name a preceding step".to_string()))?;
    if map
        .keys()
        .any(|field| *field != key("loop") && *field != key("step") && *field != key("interactive"))
    {
        return Err(LoadError::InvalidGraph(
            "a loop item takes only loop and step or interactive".to_string(),
        ));
    }
    let mut step = match (map.get(key("step")), map.get(key("interactive"))) {
        (Some(_), Some(_)) => {
            return Err(LoadError::InvalidGraph(
                "a loop decides through step or interactive, not both".to_string(),
            ))
        }
        (Some(decider), None) => parse_skill_value(decider, false)?,
        (None, Some(decider)) => parse_skill_value(decider, true)?,
        (None, None) => Step::new(Target::Skill(Skill::named(DEFAULT_DECIDER))),
    };
    step.returns = Some(target.to_string());
    Ok(step)
}

/// The authored `step:` or `interactive:` mapping flattens skill options and
/// the occurrence id.
#[derive(Debug, Deserialize)]
struct SkillMapping {
    #[serde(flatten)]
    skill: Skill,
    id: Option<String>,
}

fn parse_skill_value(value: &Value, interactive: bool) -> Result<Step, LoadError> {
    match value {
        Value::String(name) => Ok(Step {
            interactive,
            ..Step::new(Target::Skill(Skill::named(name)))
        }),
        Value::Mapping(map) => {
            if map.contains_key(key("repeat")) {
                return Err(LoadError::InvalidGraph(
                    "step repeat is retired; follow the work with `- loop: <target>`".to_string(),
                ));
            }
            if map.contains_key(key("human")) {
                return Err(LoadError::InvalidGraph(
                    "`human:` is retired; write the item as `interactive: <skill>`".to_string(),
                ));
            }
            if map.contains_key(key("interactive")) {
                return Err(LoadError::InvalidGraph(
                    "a step is interactive by its item; write it as `interactive: <skill>`"
                        .to_string(),
                ));
            }
            let mut step: SkillMapping =
                serde_yaml_ng::from_value(value.clone()).map_err(|error| {
                    LoadError::InvalidGraph(format!("invalid step mapping: {error}"))
                })?;
            validate_step_settings(step.id.as_deref())?;
            step.skill.content = None;
            Ok(Step {
                target: Target::Skill(step.skill),
                id: step.id,
                interactive,
                returns: None,
            })
        }
        _ => Err(LoadError::InvalidGraph(
            "step value must be string or mapping".to_string(),
        )),
    }
}

fn parse_xor_value(value: &Value, loader: &mut DefinitionLoader<'_>) -> Result<Step, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidGraph("xor must be mapping".to_string()))?;
    Ok(Step::new(Target::Xor(parse_xor_def(map, loader)?)))
}

fn parse_xor_def(
    map: &serde_yaml_ng::Mapping,
    loader: &mut DefinitionLoader<'_>,
) -> Result<XorDef, LoadError> {
    let paths_value = map
        .get(key("paths"))
        .ok_or_else(|| LoadError::InvalidGraph("xor must have paths".into()))?;
    let paths_map = paths_value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidGraph("xor paths must be mapping".into()))?;

    if paths_map.is_empty() {
        return Err(LoadError::InvalidGraph(
            "xor must have at least one path".into(),
        ));
    }

    let mut paths = HashMap::new();
    for (path_key, path_value) in paths_map {
        let key_str = path_key
            .as_str()
            .ok_or_else(|| LoadError::InvalidGraph("xor path key must be string".into()))?;
        let path_map = path_value.as_mapping().ok_or_else(|| {
            LoadError::InvalidGraph(format!("xor path '{key_str}' must be mapping"))
        })?;

        let graph = parse_optional_string(path_map, "graph");
        let skill = parse_optional_string(path_map, "skill");
        let skills = parse_xor_path_skills(path_map, key_str)?;

        let target_count = usize::from(graph.is_some())
            + usize::from(skill.is_some())
            + usize::from(!skills.is_empty());
        if target_count > 1 {
            return Err(LoadError::InvalidGraph(format!(
                "xor path '{key_str}' cannot have more than one of graph, skill, or steps"
            )));
        }

        let description = parse_optional_string(path_map, "description").ok_or_else(|| {
            LoadError::InvalidGraph(format!("xor path '{key_str}' must have description"))
        })?;

        let steps = if let Some(name) = graph {
            vec![Step::new(loader.resolve(&name, None)?)]
        } else if let Some(name) = skill {
            vec![Step::new(Target::Skill(Skill::named(&name)))]
        } else {
            skills
        };
        paths.insert(key_str.to_string(), XorPath { steps, description });
    }

    let router = parse_optional_string(map, "router");

    Ok(XorDef { router, paths })
}

pub fn build_xor_routing_suffix(xor_def: &ConcreteXor) -> String {
    let mut suffix = String::from(
        "## Routing\n\nAfter completing your analysis, choose one of these paths:\n\n",
    );
    let mut keys: Vec<&String> = xor_def.paths.keys().collect();
    keys.sort();
    for key in &keys {
        let path = &xor_def.paths[*key];
        suffix.push_str(&format!("- **{key}**: {}\n", path.description));
    }
    suffix.push_str("\nReturn the declared JSON object with `path` set to one listed key.\n");
    suffix
}

fn compile_branch(
    branch_def: &XorDef,
    catalog: &SkillCatalog,
    sources: &[String],
) -> Result<ResolvedGraphItem, LoadError> {
    let router = match &branch_def.router {
        Some(name) => catalog.load_skill(name)?,
        None => catalog.load_skill("xor-route")?,
    };
    let paths = branch_def
        .paths
        .iter()
        .map(|(name, path)| {
            Ok((
                name.clone(),
                ResolvedGraphPath {
                    description: path.description.clone(),
                    items: compile_steps(&path.steps, catalog, sources)?,
                },
            ))
        })
        .collect::<Result<_, LoadError>>()?;
    Ok(ResolvedGraphItem::Xor {
        router,
        paths,
        sources: sources.to_vec(),
    })
}

fn parse_xor_path_skills(
    map: &serde_yaml_ng::Mapping,
    path_name: &str,
) -> Result<Vec<Step>, LoadError> {
    let Some(value) = map.get(key("steps")) else {
        return Ok(Vec::new());
    };

    let Value::Sequence(items) = value else {
        return Err(LoadError::InvalidGraph(format!(
            "xor path '{path_name}' skills must be a list"
        )));
    };

    items
        .iter()
        .map(|item| match item {
            Value::String(_) => parse_skill_value(item, false),
            Value::Mapping(skill_map) => {
                if let Some(target) = skill_map.get(key("loop")) {
                    return parse_loop(skill_map, target);
                }
                if let Some(skill_value) = skill_map.get(key("step")) {
                    return parse_skill_value(skill_value, false);
                }
                if let Some(skill_value) = skill_map.get(key("interactive")) {
                    return parse_skill_value(skill_value, true);
                }
                parse_skill_value(item, false)
            }
            _ => Err(LoadError::InvalidGraph(format!(
                "xor path '{path_name}' skills must contain only skill items"
            ))),
        })
        .collect()
}

fn parse_named_target(value: &Value, loader: &mut DefinitionLoader<'_>) -> Result<Step, LoadError> {
    let name = value
        .as_str()
        .ok_or_else(|| LoadError::InvalidGraph("graph ref must be string".to_string()))?;
    Ok(Step::new(loader.resolve(name, None)?))
}

fn parse_optional_string(map: &serde_yaml_ng::Mapping, field: &str) -> Option<String> {
    map.get(key(field))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn resolve_skill_reference(skill: &Skill, catalog: &SkillCatalog) -> Result<Skill, LoadError> {
    if skill.content.is_some() {
        return Ok(skill.clone());
    }

    let mut resolved = catalog.load_skill(&skill.name)?;

    if let Some(agent) = &skill.agent {
        resolved.agent = Some(agent.clone());
    }
    if let Some(default_agent) = &skill.default_agent {
        resolved.default_agent = Some(default_agent.clone());
    }
    if let Some(action_style) = &skill.action_style {
        resolved.action_style = Some(action_style.clone());
    }
    Ok(resolved)
}

fn validate_graph_nesting(sources: &[String], name: &str) -> Result<(), LoadError> {
    if sources.iter().any(|parent| parent == name) {
        return Err(LoadError::InvalidGraph(format!(
            "graph cycle detected: {} -> {name}",
            sources.join(" ")
        )));
    }
    if sources.len() > 5 {
        return Err(LoadError::InvalidGraph(
            "graph nesting exceeds max depth 5".into(),
        ));
    }
    Ok(())
}

fn compile_with_sources(
    graph: &GraphDefinition,
    catalog: &SkillCatalog,
    sources: &[String],
) -> Result<Vec<ResolvedGraphItem>, LoadError> {
    validate_graph_nesting(sources, &graph.name)?;
    let mut sources = sources.to_vec();
    sources.push(graph.name.clone());
    compile_steps(&graph.items, catalog, &sources)
}

fn compile_steps(
    steps: &[Step],
    catalog: &SkillCatalog,
    sources: &[String],
) -> Result<Vec<ResolvedGraphItem>, LoadError> {
    let mut items = Vec::new();
    for step in steps {
        if !matches!(step.target, Target::Skill(_))
            && (step.id.is_some() || step.interactive || step.returns.is_some())
        {
            return Err(LoadError::InvalidGraph(
                "occurrence metadata are valid only on skill nodes".into(),
            ));
        }
        match &step.target {
            Target::Command(command) => items.push(ResolvedGraphItem::Command(ConcreteCommand {
                item: command.clone(),
                sources: sources.to_vec(),
            })),
            Target::Graph(graph) => {
                items.push(ResolvedGraphItem::Group {
                    name: graph.name.clone(),
                    items: compile_with_sources(graph, catalog, sources)?,
                });
            }
            Target::Skill(skill) => items.push(ResolvedGraphItem::Skill(ConcreteSkill {
                skill: resolve_skill_reference(skill, catalog)?,
                id: step.id.clone(),
                interactive: step.interactive,
                returns: step
                    .returns
                    .as_deref()
                    .map(|target| resolve_loop_target(&flatten_resolved(&items), target))
                    .transpose()?,
                sources: sources.to_vec(),
            })),
            Target::Xor(branch) => items.push(compile_branch(branch, catalog, sources)?),
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use serde_yaml_ng::Value;

    use super::{
        build_xor_routing_suffix, compile_branch, compile_graph, load_graph, load_skill,
        ConcreteStep, DefinitionLoader, GraphDefinition, Skill, Step, XorDef, XorPath,
    };
    use crate::definition::Target;
    use crate::error::LoadError;
    use tempfile::TempDir;

    fn parse_graph_items(value: &Value) -> Result<Vec<Step>, LoadError> {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".lf/graphs")).unwrap();
        for name in ["qa-fix", "deploy", "build"] {
            fs::write(
                tmp.path().join(format!(".lf/graphs/{name}.yaml")),
                "- step: implement\n",
            )
            .unwrap();
        }
        super::parse_graph_items(
            value,
            &mut DefinitionLoader::new(
                Some(tmp.path()),
                &super::SkillCatalog::discover(Some(tmp.path())).unwrap(),
            ),
        )
    }

    #[test]
    fn portable_graph_names_coexist_and_alias_cycles_fail() {
        let repo = TempDir::new().unwrap();
        let graphs = repo.path().join(".lf/graphs/team");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(graphs.join("ship.yaml"), "- implement\n").unwrap();
        for name in ["team/ship", "team-ship"] {
            assert_eq!(
                super::load_authored_graph(name, repo.path()).unwrap().name,
                "team/ship"
            );
            assert_eq!(
                super::customize(name, repo.path()).unwrap(),
                graphs.join("ship.yaml")
            );
        }
        fs::write(graphs.join("ship.yaml"), "- graph: team-ship\n").unwrap();
        assert!(super::load_authored_graph("team/ship", repo.path())
            .unwrap_err()
            .to_string()
            .contains("cycle detected"));
        fs::write(repo.path().join(".lf/graphs/team-ship.yml"), "- gate\n").unwrap();
        let graph = super::load_authored_graph("team/ship", repo.path()).unwrap();
        assert_eq!(graph.name, "team/ship");
        let steps = compile_graph(&graph, repo.path()).unwrap();
        assert!(matches!(&steps[..], [ConcreteStep::Skill(skill)] if skill.skill.name == "gate"));
        assert_eq!(
            super::load_authored_graph("team-ship", repo.path())
                .unwrap()
                .name,
            "team-ship"
        );
        assert_eq!(
            super::customize("team-ship", repo.path()).unwrap(),
            repo.path().join(".lf/graphs/team-ship.yml")
        );
    }

    #[test]
    fn portable_repository_graph_only_overrides_identical_builtin_name() {
        let repo = TempDir::new().unwrap();
        let path = repo.path().join(".lf/graphs/task/design.yaml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "- cmd: sync\n").unwrap();
        assert_eq!(
            super::load_authored_graph("task/design", repo.path())
                .unwrap()
                .name,
            "task/design"
        );
        assert_eq!(
            super::load_authored_graph("task-design", repo.path())
                .unwrap()
                .name,
            "task-design"
        );
        assert_eq!(
            super::find_graph_source_path("task-design", repo.path()).unwrap(),
            None
        );
        let dashed = repo.path().join(".lf/graphs/task-design.yaml");
        fs::write(&dashed, "- cmd: sync\n").unwrap();
        assert_eq!(
            super::customize("task-design", repo.path()).unwrap(),
            dashed
        );
    }

    #[test]
    fn portable_graph_fallback_reports_only_real_ambiguity() {
        let repo = TempDir::new().unwrap();
        for name in ["a/b-c", "a-b/c"] {
            let path = repo.path().join(format!(".lf/graphs/{name}.yaml"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "- implement\n").unwrap();
            assert_eq!(
                super::load_authored_graph(name, repo.path()).unwrap().name,
                name
            );
        }
        let error = super::load_authored_graph("a-b-c", repo.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("a/b-c") && error.contains("a-b/c"));
    }

    #[test]
    fn portable_untyped_lookup_prefers_exact_names_before_kind() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs/team")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills/team")).unwrap();
        let path = repo.path().join(".lf/graphs/team/work.yaml");
        fs::write(&path, "- implement\n").unwrap();
        fs::write(repo.path().join(".lf/skills/team-work.md"), "Dash skill").unwrap();
        assert_eq!(
            load_graph("team/work", repo.path()).unwrap().name,
            "team/work"
        );
        assert_eq!(
            load_skill("team/work", repo.path()).unwrap().name,
            "team-work"
        );
        assert!(matches!(
            super::resolve_definition(repo.path(), "team-work", None).unwrap(),
            super::Target::Skill(_)
        ));
        fs::write(repo.path().join(".lf/skills/team/work.md"), "Slash skill").unwrap();
        assert!(matches!(
            super::resolve_definition(repo.path(), "team/work", None).unwrap(),
            super::Target::Graph(_)
        ));
        fs::write(&path, "broken: [").unwrap();
        assert!(load_graph("team/work", repo.path()).is_err());
        assert!(matches!(
            super::resolve_definition(repo.path(), "team-work", None).unwrap(),
            super::Target::Skill(_)
        ));
    }

    #[test]
    fn portable_loop_targets_prefer_ids_then_literal_names_then_fallback() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills/team")).unwrap();
        for name in ["team/check", "team-check"] {
            fs::write(repo.path().join(format!(".lf/skills/{name}.md")), name).unwrap();
        }
        let path = repo.path().join(".lf/graphs/proof.yaml");
        for (body, target) in [
            ("- step: {name: team/check, id: team-check}\n- step: team-check\n- loop: team-check\n", 0),
            ("- step: team/check\n- step: team-check\n- loop: team-check\n", 1),
            ("- step: team/check\n- step: team-check\n- loop: team/check\n", 0),
            ("- step: team/check\n- loop: team-check\n", 0),
        ] {
            fs::write(&path, body).unwrap();
            let steps = compile_graph(&super::load_authored_graph("proof", repo.path()).unwrap(), repo.path()).unwrap();
            assert_eq!(super::return_target(&steps, steps.len() - 1), Some(target));
        }
    }

    #[test]
    fn load_skill_finds_repo_local_skill() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("myskill.md"), "# My Skill\nDo the thing.").unwrap();

        let skill = load_skill("myskill", tmp.path()).unwrap();
        assert_eq!(skill.name, "myskill");
        assert!(skill.content.unwrap().contains("Do the thing"));
    }

    #[test]
    fn interactive_items_are_steps_named_by_their_skill_or_id() {
        let tmp = TempDir::new().unwrap();
        let graphs = tmp.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(
            graphs.join("review.yaml"),
            "- interactive: explore\n- interactive: {name: demo, id: final-demo, agent: codex}\n",
        )
        .unwrap();
        let graph = load_graph("review", tmp.path()).unwrap();
        let Target::Skill(demo) = &graph.items[1].target else {
            panic!("demo is a skill step");
        };
        assert_eq!(demo.agent.as_deref(), Some("codex"));
        let items = compile_graph(&graph, tmp.path()).unwrap();
        let steps: Vec<_> = items
            .iter()
            .filter_map(|item| match item {
                ConcreteStep::Skill(skill) => skill.interactive_step().map(str::to_owned),
                _ => None,
            })
            .collect();
        assert_eq!(steps, ["explore", "final-demo"]);
        let ConcreteStep::Skill(demo) = &items[1] else {
            panic!("demo compiles to a skill step");
        };
        assert_eq!(demo.skill.agent.as_deref(), Some("codex"));

        fs::write(
            graphs.join("retired.yaml"),
            "- step:\n    name: explore\n    human: true\n",
        )
        .unwrap();
        let error = load_graph("retired", tmp.path()).unwrap_err();
        assert!(error.to_string().contains("retired"), "{error}");
        fs::write(graphs.join("item.yaml"), "- human: explore\n").unwrap();
        let error = load_graph("item", tmp.path()).unwrap_err();
        assert!(
            error.to_string().contains("interactive: <skill>"),
            "{error}"
        );
    }

    #[test]
    fn loops_resolve_one_preceding_occurrence() {
        let tmp = TempDir::new().unwrap();
        let graphs = tmp.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        let compile = |body: &str| {
            fs::write(graphs.join("loop-proof.yaml"), body).unwrap();
            load_graph("loop-proof", tmp.path()).and_then(|graph| compile_graph(&graph, tmp.path()))
        };
        let edges = |steps: &[ConcreteStep]| -> Vec<(String, usize)> {
            steps
                .iter()
                .filter_map(|step| match step {
                    ConcreteStep::Skill(skill) => Some((skill.skill.name.clone(), skill.returns?)),
                    _ => None,
                })
                .collect()
        };
        let pursue = compile_graph(&load_graph("pursue", tmp.path()).unwrap(), tmp.path()).unwrap();
        assert_eq!(
            edges(&pursue),
            [
                ("loop-or-next".to_string(), 4),
                ("loop-or-next".to_string(), 6)
            ]
        );

        // A unique skill name is enough; repeated skills need one occurrence name.
        let named = compile(
            "- implement\n- step: {name: implement, id: second}\n- loop: second\n- loop: second\n  step: gate\n",
        )
        .unwrap();
        assert_eq!(
            edges(&named),
            [("loop-or-next".to_string(), 1), ("gate".to_string(), 2)]
        );
        for (body, expected) in [
            ("- implement\n- loop: missing\n", "preceding step"),
            ("- loop: implement\n- implement\n", "preceding step"),
            (
                "- implement\n- implement\n- loop: implement\n",
                "ambiguous: steps 1, 2 of 2",
            ),
            (
                "- implement\n- loop: implement\n  id: decide\n",
                "only loop and step",
            ),
            (
                "- implement\n- loop: implement\n  step: {name: demo, id: demo, human: true}\n",
                "retired",
            ),
            (
                "- implement\n- loop: implement\n  step: demo\n  interactive: demo\n",
                "not both",
            ),
            (
                "- implement\n- step: {name: gate, repeat: {from: implement}}\n",
                "repeat is retired",
            ),
        ] {
            let error = compile(body).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn graph_steps_ignore_retired_skill_capabilities() {
        let tmp = TempDir::new().unwrap();
        let graphs = tmp.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(
            graphs.join("fake-implementation.yaml"),
            "- step:\n    name: design\n    capabilities: [task_implementation]\n",
        )
        .unwrap();

        let graph = load_graph("fake-implementation", tmp.path()).expect("load graph");
        let steps = compile_graph(&graph, tmp.path()).expect("compile graph");

        assert!(matches!(
            &steps[0],
            ConcreteStep::Skill(skill) if skill.skill.name == "design"
        ));
    }

    #[test]
    fn nested_expansion_preserves_stage_identity() {
        let tmp = TempDir::new().unwrap();
        let graphs = tmp.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(
            graphs.join("inner.yaml"),
            "- step:\n    id: revise\n    name: implement\n- interactive:\n    id: review_kickoff\n    name: explore\n",
        )
        .unwrap();
        fs::write(graphs.join("outer.yaml"), "- graph: inner\n").unwrap();

        let graph = load_graph("outer", tmp.path()).unwrap();
        let items = compile_graph(&graph, tmp.path()).unwrap();
        let ConcreteStep::Skill(step) = &items[1] else {
            panic!("nested review node must compile to a skill")
        };
        assert_eq!(step.id.as_deref(), Some("review_kickoff"));
        assert!(step.interactive);
        assert_eq!(step.sources, vec!["outer", "inner"]);
    }

    #[test]
    fn compiled_occurrence_ids_are_unique_and_skill_only() {
        let tmp = TempDir::new().unwrap();
        let graphs = tmp.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(
            graphs.join("inner.yaml"),
            "- interactive:\n    id: review_kickoff\n    name: explore\n",
        )
        .unwrap();
        fs::write(
            graphs.join("duplicate.yaml"),
            "- graph: inner\n- graph: inner\n",
        )
        .unwrap();
        let duplicate = load_graph("duplicate", tmp.path()).unwrap();
        assert!(compile_graph(&duplicate, tmp.path())
            .unwrap_err()
            .to_string()
            .contains("not unique after expansion"));

        fs::write(graphs.join("invalid.yaml"), "- cmd: sync\n  id: review\n").unwrap();
        assert!(load_graph("invalid", tmp.path())
            .unwrap_err()
            .to_string()
            .contains("valid only on skill nodes"));
    }

    #[test]
    fn load_skill_rejects_legacy_colon_form() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills/gstack");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("office-hours.md"),
            "---\ninteractive: false\n---\n# Office Hours\nDo the thing.\n",
        )
        .unwrap();

        // The colon form is no longer accepted; users must use `/`.
        let err = load_skill("gstack:office-hours", tmp.path()).unwrap_err();
        assert!(matches!(err, LoadError::SkillNotFound(_)));
    }

    #[test]
    fn load_skill_finds_namespaced_skill_with_slash() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills/gstack");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("office-hours.md"),
            "---\ninteractive: false\n---\n# Office Hours\nDo the thing.\n",
        )
        .unwrap();

        let skill = load_skill("gstack/office-hours", tmp.path()).unwrap();
        assert_eq!(skill.name, "gstack/office-hours");
        assert!(skill.content.unwrap().contains("Do the thing"));
    }

    #[test]
    fn load_skill_finds_builtin_skill() {
        // Builtin skills like "unbreak", "implement" should be available everywhere
        let tmp = TempDir::new().unwrap();

        let result = load_skill("unbreak", tmp.path());
        assert!(
            result.is_ok(),
            "builtin 'unbreak' skill should be found: {:?}",
            result.err()
        );
    }

    #[test]
    fn load_skill_finds_all_builtins() {
        let tmp = TempDir::new().unwrap();
        for name in crate::builtins::builtin_skill_names() {
            let result = load_skill(name, tmp.path());
            assert!(
                result.is_ok(),
                "builtin '{}' skill should be found: {:?}",
                name,
                result.err()
            );
        }
    }

    #[test]
    fn load_skill_parses_frontmatter_agent() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("fast.md"),
            r#"---
agent: claude:haiku
---
# Fast Skill
Do it quickly.
"#,
        )
        .unwrap();

        let skill = load_skill("fast", tmp.path()).unwrap();
        assert_eq!(skill.agent, Some("claude:haiku".to_string()));
    }

    #[test]
    fn load_skill_parses_frontmatter_default_agent() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("fast.md"),
            r#"---
default_agent: codex:o3
---
# Fast Skill
Do it quickly.
"#,
        )
        .unwrap();

        let skill = load_skill("fast", tmp.path()).unwrap();
        assert_eq!(skill.default_agent, Some("codex:o3".to_string()));
    }

    #[test]
    fn load_skill_ignores_retired_frontmatter_interactive() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("design.md"),
            r#"---
interactive: true
---
# Design Skill
Design the feature.
"#,
        )
        .unwrap();

        let skill = load_skill("design", tmp.path()).unwrap();
        assert_eq!(skill.name, "design");
        assert!(skill.content.unwrap().contains("Design the feature."));
    }

    #[test]
    fn load_skill_ignores_retired_capabilities_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("design.md"),
            r#"---
action_style: exploratory
capabilities: [task_implementation]
---
# Design Skill
Design the feature.
"#,
        )
        .unwrap();

        let skill = load_skill("design", tmp.path()).unwrap();
        assert_eq!(skill.action_style.as_deref(), Some("exploratory"));
        assert!(skill.content.unwrap().contains("Design the feature."));
    }

    #[test]
    fn load_skill_not_found_error_message() {
        let tmp = TempDir::new().unwrap();

        let result = load_skill("nonexistent", tmp.path());
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn load_graph_finds_builtin_graph() {
        let tmp = TempDir::new().unwrap();
        let result = load_graph("pursue", tmp.path());
        assert!(
            result.is_ok(),
            "builtin 'pursue' graph should be found: {:?}",
            result.err()
        );
    }

    #[test]
    fn load_skill_falls_back_to_agent_skills() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".agents/skills/my-tool");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-tool\n---\nDo the thing.",
        )
        .unwrap();

        let skill = load_skill("my-tool", tmp.path()).unwrap();
        assert_eq!(skill.name, "my-tool");
        assert!(skill.content.unwrap().contains("Do the thing."));
    }

    #[test]
    fn incident_repairs_investigates_and_plans_without_expanding_delivery() {
        let tmp = TempDir::new().unwrap();
        let steps =
            compile_graph(&load_graph("incident", tmp.path()).unwrap(), tmp.path()).unwrap();
        let names: Vec<_> = steps
            .iter()
            .map(|step| match step {
                ConcreteStep::Skill(step) => step.skill.name.as_str(),
                _ => panic!("incident should contain only repair, analysis and planning"),
            })
            .collect();
        assert_eq!(names, ["unbreak", "5whys", "launch-plan"]);
    }

    #[test]
    fn vsm_operation_captures_five_functions_without_a_loop() {
        let tmp = TempDir::new().unwrap();
        let steps =
            compile_graph(&load_graph("vsm-operate", tmp.path()).unwrap(), tmp.path()).unwrap();
        let names: Vec<_> = steps
            .iter()
            .map(|step| {
                let ConcreteStep::Skill(step) = step else {
                    panic!("VSM should contain only its five skills");
                };
                assert_eq!(step.sources, ["vsm-operate"]);
                assert!(!step.interactive);
                assert!(step.returns.is_none());
                let content = step.skill.content.as_ref().unwrap();
                assert!(content.contains("lf wave list --current --json"));
                assert!(content.contains("LF_GRAPH_ID"));
                step.skill.name.as_str()
            })
            .collect();
        assert_eq!(names, ["s1", "s2", "s3", "s4", "s5"]);
    }

    #[test]
    fn load_graph_compiles_all_builtin_graphs() {
        let tmp = TempDir::new().unwrap();
        for name in crate::builtins::builtin_graph_names() {
            let graph = load_graph(name, tmp.path());
            assert!(
                graph.is_ok(),
                "builtin graph '{}' should load: {:?}",
                name,
                graph.err()
            );
            let graph = graph.unwrap();
            let compiled = compile_graph(&graph, tmp.path());
            assert!(
                compiled.is_ok(),
                "builtin graph '{}' should compile: {:?}",
                name,
                compiled.err()
            );
            assert_captured(&compiled.unwrap());
        }

        fn assert_captured(items: &[ConcreteStep]) {
            for item in items {
                match item {
                    ConcreteStep::Skill(skill) => assert!(
                        skill.skill.content.is_some(),
                        "missing captured skill {}",
                        skill.skill.name,
                    ),
                    ConcreteStep::Command(op) => assert!(!op.item.command.is_empty()),
                    ConcreteStep::Xor(branch) => {
                        assert!(branch.router.content.is_some());
                        for path in branch.paths.values() {
                            assert_captured(&path.steps);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn retired_and_step_is_rejected() {
        let yaml = r#"
- and:
    branches:
      - step: implement
      - step: review
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let error = parse_graph_items(&value).expect_err("and steps are retired");
        assert!(error.to_string().contains(
            "graph item mapping must include step, interactive, loop, cmd, graph, or xor"
        ));
    }

    #[test]
    fn unimplemented_or_step_is_rejected() {
        let yaml = r#"
- or:
    paths:
      fix:
        step: implement
        description: "Fix it"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let error = parse_graph_items(&value).expect_err("or steps are not supported");
        assert!(error.to_string().contains(
            "graph item mapping must include step, interactive, loop, cmd, graph, or xor"
        ));
    }

    #[test]
    fn generic_loop_step_is_rejected() {
        let yaml = r#"
- loop:
    steps: [implement]
    exit:
      paths:
        done:
          description: "Done"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let error = parse_graph_items(&value).expect_err("generic loops are retired");
        assert!(error
            .to_string()
            .contains("loop must name a preceding step"));
    }

    #[test]
    fn parse_command_mapping_accepts_command_and_args() {
        let yaml = r#"
- cmd: pr land
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_graph_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step {
                target: Target::Command(item),
                ..
            } => {
                assert_eq!(item.command, "pr");
                assert_eq!(item.args, vec!["land"]);
            }
            other => panic!("expected command item, got {other:?}"),
        }
    }

    #[test]
    fn parse_xor_with_graph_paths() {
        let yaml = r#"
- qa
- triage
- xor:
    paths:
      fix:
        graph: qa-fix
        description: "Blocking issues found, fix before deploy"
      deploy:
        graph: deploy
        description: "Clean enough to ship"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_graph_items(&value).unwrap();
        assert_eq!(items.len(), 3);

        match &items[2] {
            Step {
                target: Target::Xor(branch),
                ..
            } => {
                assert_eq!(branch.paths.len(), 2);
                let fix = &branch.paths["fix"];
                assert!(
                    matches!(&fix.steps[0].target, Target::Graph(graph) if graph.name == "qa-fix")
                );
                assert_eq!(fix.description, "Blocking issues found, fix before deploy");
                let deploy = &branch.paths["deploy"];
                assert!(
                    matches!(&deploy.steps[0].target, Target::Graph(graph) if graph.name == "deploy")
                );
                assert_eq!(deploy.description, "Clean enough to ship");
            }
            other => panic!("expected Xor, got {other:?}"),
        }
    }

    #[test]
    fn parse_xor_with_skill_path() {
        let yaml = r#"
- xor:
    paths:
      skip:
        skill: gate
        description: "Just run gate"
      full:
        graph: build
        description: "Full build"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_graph_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step {
                target: Target::Xor(branch),
                ..
            } => {
                let skip = &branch.paths["skip"];
                assert_eq!(
                    skip.steps,
                    vec![Step::new(Target::Skill(Skill::named("gate")))]
                );
                let full = &branch.paths["full"];
                assert!(
                    matches!(&full.steps[0].target, Target::Graph(graph) if graph.name == "build")
                );
            }
            other => panic!("expected Xor, got {other:?}"),
        }
    }

    #[test]
    fn parse_xor_rejects_both_graph_and_skill() {
        let yaml = r#"
- xor:
    paths:
      bad:
        graph: build
        skill: gate
        description: "invalid"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let result = parse_graph_items(&value);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("graph") && err.contains("skill"),
            "expected error about both graph and skill, got: {err}"
        );
    }

    #[test]
    fn parse_xor_allows_silence_path() {
        let yaml = r#"
- xor:
    paths:
      silence:
        description: "no action needed"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let result = parse_graph_items(&value);
        assert!(
            result.is_ok(),
            "path with only description should be valid (silence path)"
        );
    }

    #[test]
    fn parse_xor_rejects_missing_description() {
        let yaml = r#"
- xor:
    paths:
      bad:
        graph: build
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let result = parse_graph_items(&value);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("description"),
            "expected error about missing description, got: {err}"
        );
    }

    #[test]
    fn parse_xor_accepts_inline_skills() {
        let yaml = r#"
- xor:
    paths:
      tune:
        description: "Adjust the chord"
        steps:
          - implement
          - review
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_graph_items(&value).unwrap();

        let Step {
            target: Target::Xor(xor_def),
            ..
        } = &items[0]
        else {
            panic!("expected xor item");
        };
        let tune = &xor_def.paths["tune"];
        assert_eq!(tune.steps.len(), 2);
        assert_eq!(
            tune.steps[0],
            Step::new(Target::Skill(Skill::named("implement")))
        );
        assert_eq!(
            tune.steps[1],
            Step::new(Target::Skill(Skill::named("review")))
        );
    }

    #[test]
    fn parse_xor_rejects_multiple_targets() {
        let yaml = r#"
- xor:
    paths:
      bad:
        description: "invalid"
        skill: implement
        steps:
          - review
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let err = parse_graph_items(&value).unwrap_err().to_string();
        assert!(err.contains("graph, skill, or steps"));
    }

    #[test]
    fn loaded_names_and_inline_targets_capture_the_same_branch_behavior() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".lf/graphs")).unwrap();
        fs::create_dir_all(tmp.path().join(".lf/skills")).unwrap();
        fs::write(tmp.path().join(".lf/skills/work.md"), "Captured work").unwrap();
        fs::write(
            tmp.path().join(".lf/graphs/inner.yaml"),
            "- interactive:\n    name: work\n    id: review\n- cmd: pr land --local\n",
        )
        .unwrap();
        fs::write(tmp.path().join(".lf/graphs/outer.yaml"),
            "- xor:\n    paths:\n      proceed:\n        graph: inner\n        description: Review and deliver\n").unwrap();
        let mut outer = load_graph("outer", tmp.path()).unwrap();
        let named = compile_graph(&outer, tmp.path()).unwrap();
        let inner = load_graph("inner", tmp.path()).unwrap();
        fs::remove_dir_all(tmp.path().join(".lf/graphs")).unwrap();
        assert_eq!(compile_graph(&outer, tmp.path()).unwrap(), named);
        let Step {
            target: Target::Xor(branch),
            ..
        } = &mut outer.items[0]
        else {
            panic!("YAML xor must compose a Target")
        };
        branch.paths.get_mut("proceed").unwrap().steps = vec![Step::new(Target::Graph(inner))];
        assert_eq!(compile_graph(&outer, tmp.path()).unwrap(), named);
        let ConcreteStep::Xor(branch) = &named[0] else {
            panic!("captured branch")
        };
        let ConcreteStep::Skill(review) = &branch.paths["proceed"].steps[0] else {
            panic!("review")
        };
        assert_eq!(review.interactive_step(), Some("review"));
        assert_eq!(review.skill.content.as_deref(), Some("Captured work"));
        assert_eq!(review.sources, ["outer", "inner"]);
        let ConcreteStep::Command(command) = &branch.paths["proceed"].steps[1] else {
            panic!("command")
        };
        assert_eq!(command.item.argv(), ["lf", "pr", "land", "--local"]);
    }

    #[test]
    fn compile_xor_keeps_concrete_xor() {
        let tmp = TempDir::new().unwrap();
        let graph = GraphDefinition {
            name: "test-or".to_string(),
            items: vec![
                Step::new(Target::Skill(Skill::named("gate"))),
                Step::new(Target::Xor(XorDef {
                    router: None,
                    paths: {
                        let mut m = HashMap::new();
                        m.insert(
                            "fix".to_string(),
                            XorPath {
                                steps: vec![Step::new(Target::Graph(
                                    load_graph("pursue", tmp.path()).unwrap(),
                                ))],
                                description: "Fix it".to_string(),
                            },
                        );
                        m
                    },
                })),
            ],
        };

        let items = compile_graph(&graph, tmp.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], ConcreteStep::Skill(_)));
        assert!(matches!(&items[1], ConcreteStep::Xor(_)));

        if let ConcreteStep::Xor(branch) = &items[1] {
            assert_eq!(branch.paths.len(), 1);
            assert_eq!(branch.paths["fix"].description, "Fix it");
        }
    }

    #[test]
    fn build_xor_routing_suffix_sorts_paths() {
        let tmp = TempDir::new().unwrap();
        let branch = compile_branch(
            &XorDef {
                router: None,
                paths: HashMap::from([
                    (
                        "zeta".to_string(),
                        XorPath {
                            steps: Vec::new(),
                            description: "Last".to_string(),
                        },
                    ),
                    (
                        "alpha".to_string(),
                        XorPath {
                            steps: Vec::new(),
                            description: "First".to_string(),
                        },
                    ),
                ]),
            },
            &super::SkillCatalog::discover(Some(tmp.path())).unwrap(),
            &[],
        )
        .unwrap();
        let ConcreteStep::Xor(branch) = super::flatten_resolved(&[branch]).remove(0) else {
            panic!("expected XOR");
        };
        let suffix = build_xor_routing_suffix(&branch);
        assert!(suffix.find("**alpha**").unwrap() < suffix.find("**zeta**").unwrap());
        assert!(suffix.contains("declared JSON object"));
        assert!(branch
            .router
            .content
            .unwrap()
            .contains("declared JSON object"));
        assert!(branch.paths.values().all(|path| path.steps.is_empty()));
    }

    #[test]
    fn xor_snapshot_preserves_all_routes_and_skills_after_source_deletion() {
        let tmp = TempDir::new().unwrap();
        let graphs = tmp.path().join(".lf/graphs");
        let skills = tmp.path().join(".lf/skills");
        fs::create_dir_all(&graphs).unwrap();
        fs::create_dir_all(&skills).unwrap();
        for (name, content) in [
            ("pin-router", "---\nagent: codex\n---\nOriginal router"),
            ("pin-chosen", "Original chosen"),
            ("pin-other", "Original other"),
        ] {
            fs::write(skills.join(format!("{name}.md")), content).unwrap();
        }
        fs::write(
            graphs.join("pin-root.yaml"),
            r#"
- xor:
    router: pin-router
    paths:
      chosen:
        description: Chosen description
        skill: pin-chosen
      unchosen:
        description: Unchosen description
        graph: pin-inner
"#,
        )
        .unwrap();
        fs::write(
            graphs.join("pin-inner.yaml"),
            r#"
- xor:
    router: pin-router
    paths:
      nested:
        description: Nested description
        steps:
          - step:
              name: pin-other
              id: nested-work
              agent: claude
          - loop: nested-work
            step: pin-other
          - interactive:
              name: pin-other
              id: nested-stage
      quiet:
        description: Nothing to do
"#,
        )
        .unwrap();
        let graph = load_graph("pin-root", tmp.path()).unwrap();
        let pinned = compile_graph(&graph, tmp.path()).unwrap();
        let saved = serde_json::to_string(&pinned).unwrap();

        fs::write(skills.join("pin-router.md"), "Changed router").unwrap();
        fs::write(skills.join("pin-other.md"), "Changed other").unwrap();
        let changed = compile_graph(&graph, tmp.path()).unwrap();
        assert_ne!(changed, pinned);
        fs::remove_dir_all(tmp.path().join(".lf")).unwrap();
        let restored: Vec<ConcreteStep> = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored, pinned);
        let ConcreteStep::Xor(root) = &restored[0] else {
            panic!("root router")
        };
        assert_eq!(root.router.content.as_deref(), Some("Original router"));
        assert_eq!(root.router.agent.as_deref(), Some("codex"));
        let ConcreteStep::Skill(chosen) = &root.paths["chosen"].steps[0] else {
            panic!("chosen skill")
        };
        assert_eq!(chosen.skill.content.as_deref(), Some("Original chosen"));
        let ConcreteStep::Xor(inner) = &root.paths["unchosen"].steps[0] else {
            panic!("unchosen router")
        };
        assert_eq!(inner.router, root.router);
        assert!(inner.paths["quiet"].steps.is_empty());
        let ConcreteStep::Skill(other) = &inner.paths["nested"].steps[0] else {
            panic!("nested skill")
        };
        assert_eq!(other.skill.content.as_deref(), Some("Original other"));
        assert_eq!(other.skill.agent.as_deref(), Some("claude"));
        assert_eq!(other.sources, ["pin-root", "pin-inner"]);
        assert!(build_xor_routing_suffix(inner).contains("Nested description"));
    }

    #[test]
    fn xor_expansion_rejects_cycles_and_excessive_depth() {
        let tmp = TempDir::new().unwrap();
        let graphs = tmp.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(graphs.join("pin-cycle.yaml"), "- xor:\n    paths:\n      branch:\n        description: Recursive\n        graph: pin-child\n").unwrap();
        for child in ["- graph: pin-cycle\n", "- pin-cycle\n", "- xor:\n    paths:\n      again:\n        description: Recursive\n        graph: pin-cycle\n"] {
            fs::write(graphs.join("pin-child.yaml"), child).unwrap();
            let error = load_graph("pin-cycle", tmp.path()).unwrap_err().to_string();
            assert!(error.contains("cycle detected"), "{error}");
        }
        for index in 0..7 {
            let next = index + 1;
            fs::write(graphs.join(format!("deep-{index}.yaml")), format!("- xor:\n    paths:\n      deeper:\n        description: Deeper\n        graph: deep-{next}\n")).unwrap();
        }
        fs::write(graphs.join("deep-7.yaml"), "- implement\n").unwrap();
        let deepest = load_graph("deep-2", tmp.path()).unwrap();
        compile_graph(&deepest, tmp.path()).unwrap();
        for (name, expected) in [("deep-1", "max depth"), ("deep-2", "cycle detected")] {
            let composed = GraphDefinition {
                name: name.into(),
                items: vec![Step::new(Target::Graph(deepest.clone()))],
            };
            let error = compile_graph(&composed, tmp.path())
                .unwrap_err()
                .to_string();
            assert!(error.contains(expected), "{error}");
        }
        let error = load_graph("deep-0", tmp.path()).unwrap_err().to_string();
        assert!(error.contains("max depth"), "{error}");
    }

    #[test]
    fn xor_expansion_validates_nested_edges_and_stage_ids() {
        let tmp = TempDir::new().unwrap();
        let graphs = tmp.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(graphs.join("pin-validation.yaml"), "- step:\n    name: implement\n    id: outside\n- xor:\n    paths:\n      nested:\n        description: Nested\n        graph: pin-invalid\n").unwrap();
        for (body, expected) in [
            ("- xor:\n    paths:\n      inline:\n        description: Invalid edge\n        steps:\n          - loop: outside\n", "preceding step"),
            ("- xor:\n    paths:\n      inline:\n        description: Invalid gate\n        steps:\n          - step:\n              name: demo\n              human: true\n", "retired"),
            ("- xor:\n    paths:\n      inline:\n        description: Duplicate gate\n        steps:\n          - interactive:\n              name: demo\n              id: outside\n", "not unique"),
        ] {
            fs::write(graphs.join("pin-invalid.yaml"), body).unwrap();
            let error = load_graph("pin-validation", tmp.path())
                .and_then(|graph| compile_graph(&graph, tmp.path())).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn xor_expansion_rejects_missing_content_in_unchosen_paths() {
        let tmp = TempDir::new().unwrap();
        for target in [
            "skill: pin-missing-skill-23952",
            "steps: [pin-missing-skill-23952]",
            "graph: pin-missing-graph-23952",
        ] {
            let value: Value = serde_yaml_ng::from_str(&format!("- xor:\n    paths:\n      empty:\n        description: Empty\n      missing:\n        description: Missing\n        {target}\n")).unwrap();
            let result = super::parse_graph_items(
                &value,
                &mut DefinitionLoader::new(
                    Some(tmp.path()),
                    &super::SkillCatalog::discover(Some(tmp.path())).unwrap(),
                ),
            )
            .and_then(|items| {
                compile_graph(
                    &GraphDefinition {
                        name: "pin-missing".into(),
                        items,
                    },
                    tmp.path(),
                )
            });
            assert!(result.unwrap_err().to_string().contains("not found"));
        }
    }

    #[test]
    fn deliver_graph_gates_then_lands_and_waits() {
        let tmp = TempDir::new().unwrap();
        let graph = load_graph("deliver", tmp.path()).unwrap();
        let items = compile_graph(&graph, tmp.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert!(
            matches!(&items[1], ConcreteStep::Command(command) if command.item.display_name() == "land --wait-and-fix")
        );
        assert!(load_graph("deploy", tmp.path()).is_err());
        assert!(load_graph("ship", tmp.path()).is_err());
    }
}
