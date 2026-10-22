//! A Task's Workflow is a graph: the compiled steps it took up, fixed from
//! then on. A step written `interactive: <skill>` is a stage, where a person
//! takes part in the Task conversation; `start` and `end` are implicit. The
//! routes between stages are projected from the steps, never authored: each
//! leaves a stage by the step that follows it, or returns to the step its
//! loop names, and runs up to the next stage. `lf task run` chooses a route
//! and runs that slice; a graph with no stages runs whole.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::LoadError;
use crate::flow::runner::{ExecutionCursor, NestedCursor};
use crate::flow::{compile_flow, load_authored_flow, parse_flow_source, ConcreteStep};
use crate::skills::first_prose_line;

pub const START: &str = "start";
pub const END: &str = "end";

/// The graph a Task took up, compiled as it was then.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedGraph {
    pub name: String,
    pub steps: Vec<ConcreteStep>,
}

/// A stage: a person takes part here, in the Task conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub name: String,
    pub skill: String,
    /// One line saying what the person does here: the skill's first line of prose.
    pub description: Option<String>,
}

/// A route between stages, named by what it runs first: the outermost graph
/// its first step was composed from, else that step's skill, else `end`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    pub name: String,
}

/// A graph as `lf task status --json` and `lf wave workflow list --json` carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub name: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

/// Where a route's run begins in the captured steps; `None` runs nothing and
/// enters `end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub start: Option<ExecutionCursor>,
}

impl WorkflowDefinition {
    /// Edges leaving `node`, with their index in route order.
    pub fn outgoing<'a>(
        &'a self,
        node: &'a str,
    ) -> impl Iterator<Item = (u32, &'a WorkflowEdge)> + 'a {
        self.edges
            .iter()
            .enumerate()
            .filter(move |(_, edge)| edge.from == node)
            .map(|(index, edge)| (index as u32, edge))
    }

    pub fn node(&self, name: &str) -> Option<&WorkflowNode> {
        self.nodes.iter().find(|node| node.name == name)
    }
}

/// A body of steps: the top level, or an XOR path descended into.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Place {
    /// Each XOR step descended through, with the path taken.
    path: Vec<(usize, String)>,
    index: usize,
}

impl Place {
    fn cursor(&self) -> ExecutionCursor {
        let mut cursor = ExecutionCursor {
            index: self.index,
            ..ExecutionCursor::default()
        };
        for (index, selected) in self.path.iter().rev() {
            cursor = ExecutionCursor {
                index: *index,
                child: Some(Box::new(NestedCursor::Xor {
                    selected: selected.clone(),
                    cursor,
                })),
                ..ExecutionCursor::default()
            };
        }
        cursor
    }
}

/// The steps of the body `path` descends to.
fn body<'a>(steps: &'a [ConcreteStep], path: &[(usize, String)]) -> &'a [ConcreteStep] {
    path.iter()
        .fold(steps, |body, (index, selected)| match &body[*index] {
            ConcreteStep::Xor(branch) => &branch.paths[selected].steps,
            _ => unreachable!("a place descends only through XOR steps"),
        })
}

fn step_name(step: &ConcreteStep) -> String {
    let (sources, own) = match step {
        ConcreteStep::Skill(skill) => (&skill.sources, skill.skill.name.clone()),
        ConcreteStep::Command(command) => (&command.sources, command.item.command.clone()),
        ConcreteStep::Xor(branch) => (&branch.sources, branch.router.name.clone()),
    };
    sources.get(1).cloned().unwrap_or(own)
}

/// The stage reached from `place` without a person: the first interactive step
/// along the straight run, looking into every XOR path in name order, else `end`.
fn next_stage(steps: &[ConcreteStep], place: &Place) -> String {
    fn scan(body: &[ConcreteStep], from: usize) -> Option<String> {
        for step in &body[from..] {
            match step {
                ConcreteStep::Skill(skill) => {
                    if let Some(stage) = skill.stage() {
                        return Some(stage.to_string());
                    }
                }
                ConcreteStep::Xor(branch) => {
                    let mut names: Vec<_> = branch.paths.keys().collect();
                    names.sort();
                    if let Some(stage) = names
                        .into_iter()
                        .find_map(|name| scan(&branch.paths[name].steps, 0))
                    {
                        return Some(stage);
                    }
                }
                ConcreteStep::Command(_) => {}
            }
        }
        None
    }
    let mut path = place.path.clone();
    let mut from = place.index;
    loop {
        if let Some(stage) = scan(body(steps, &path), from) {
            return stage;
        }
        match path.pop() {
            Some((index, _)) => from = index + 1,
            None => return END.to_string(),
        }
    }
}

/// The place after `place` in its body, or in the body that XOR belongs to.
fn after(steps: &[ConcreteStep], place: &Place) -> Option<Place> {
    let mut path = place.path.clone();
    let mut index = place.index + 1;
    loop {
        if index < body(steps, &path).len() {
            return Some(Place { path, index });
        }
        let (xor, _) = path.pop()?;
        index = xor + 1;
    }
}

impl CapturedGraph {
    /// Compile the graph named `name` as the repository defines it now.
    pub fn load(name: &str, repo: &Path) -> Result<Option<Self>, LoadError> {
        let flow = match load_authored_flow(name, repo) {
            Ok(flow) => flow,
            Err(LoadError::FlowNotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let steps = compile_flow(&flow, repo)?;
        Ok(Some(Self {
            name: flow.name,
            steps,
        }))
    }

    /// Compile authored text kept outside the repository, such as a Wave's stored graph.
    pub fn from_source(name: &str, content: &str, repo: &Path) -> Result<Self, LoadError> {
        let flow = parse_flow_source(name, content, repo)?;
        let steps = compile_flow(&flow, repo)?;
        Ok(Self {
            name: flow.name,
            steps,
        })
    }

    /// The graph of a Task that reached `end` without taking one up: nothing
    /// between `start` and `end`.
    pub fn unplanned() -> Self {
        Self {
            name: "unplanned".to_string(),
            steps: Vec::new(),
        }
    }

    fn stages(&self) -> Vec<(Place, &crate::flow::ConcreteSkill)> {
        fn collect<'a>(
            steps: &'a [ConcreteStep],
            path: &[(usize, String)],
            out: &mut Vec<(Place, &'a crate::flow::ConcreteSkill)>,
        ) {
            for (index, step) in steps.iter().enumerate() {
                match step {
                    ConcreteStep::Skill(skill) if skill.interactive => out.push((
                        Place {
                            path: path.to_vec(),
                            index,
                        },
                        skill,
                    )),
                    ConcreteStep::Xor(branch) => {
                        let mut names: Vec<_> = branch.paths.keys().collect();
                        names.sort();
                        for name in names {
                            let mut path = path.to_vec();
                            path.push((index, name.clone()));
                            collect(&branch.paths[name].steps, &path, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        collect(&self.steps, &[], &mut out);
        out
    }

    /// Its stages and the routes between them, with where each route's run begins.
    pub fn project(&self) -> Result<(WorkflowDefinition, Vec<Route>), String> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut routes = Vec::new();
        let mut route = |from: &str, place: Option<Place>| {
            let (to, name, start) = match place {
                Some(place) => (
                    next_stage(&self.steps, &place),
                    step_name(&body(&self.steps, &place.path)[place.index]),
                    Some(place.cursor()),
                ),
                None => (END.to_string(), END.to_string(), None),
            };
            edges.push(WorkflowEdge {
                from: from.to_string(),
                to,
                name,
            });
            routes.push(Route { start });
        };
        route(
            START,
            (!self.steps.is_empty()).then(|| Place {
                path: Vec::new(),
                index: 0,
            }),
        );
        for (place, skill) in self.stages() {
            let stage = skill.stage().expect("collected stages are interactive");
            nodes.push(WorkflowNode {
                name: stage.to_string(),
                skill: skill.skill.name.clone(),
                description: skill.skill.content.as_deref().and_then(first_prose_line),
            });
            if let Some(returns) = skill.returns {
                route(
                    stage,
                    Some(Place {
                        path: place.path.clone(),
                        index: place.index - returns,
                    }),
                );
            }
            route(stage, after(&self.steps, &place));
        }
        let definition = WorkflowDefinition {
            name: self.name.clone(),
            nodes,
            edges,
        };
        for node in std::iter::once(START).chain(definition.nodes.iter().map(|n| n.name.as_str())) {
            let mut names: Vec<_> = definition
                .outgoing(node)
                .map(|(_, edge)| edge.name.as_str())
                .collect();
            names.sort_unstable();
            if let Some(duplicate) = names.windows(2).find(|pair| pair[0] == pair[1]) {
                return Err(format!(
                    "two routes leave {node} as {}; compose one of them as a graph of its own",
                    duplicate[0]
                ));
            }
        }
        Ok((definition, routes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCatalogEntry {
    pub name: String,
    pub source: Option<String>,
    pub workflow: Option<WorkflowDefinition>,
    pub unavailable: Option<String>,
}

/// Every graph available in the repository, projected as a Workflow; a graph
/// that does not compile or route stays listed with the reason.
pub fn workflow_catalog(repo: &Path) -> Result<Vec<WorkflowCatalogEntry>, LoadError> {
    crate::flow::available_flow_names(repo)?
        .into_iter()
        .map(|name| {
            let (workflow, unavailable) = match load_workflow(&name, repo) {
                Ok(Some(definition)) => (Some(definition), None),
                Ok(None) => (None, Some(format!("graph {name} is not defined"))),
                Err(error) => (None, Some(error.to_string())),
            };
            Ok(WorkflowCatalogEntry {
                source: crate::flow::find_flow_source_path(&name, repo)?.map(|path| {
                    path.strip_prefix(repo)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned()
                }),
                name,
                workflow,
                unavailable,
            })
        })
        .collect()
}

/// The graph named `name`, projected; `None` when the repository defines none.
pub fn load_workflow(name: &str, repo: &Path) -> Result<Option<WorkflowDefinition>, LoadError> {
    let Some(graph) = CapturedGraph::load(name, repo)? else {
        return Ok(None);
    };
    graph
        .project()
        .map(|(definition, _)| Some(definition))
        .map_err(|error| LoadError::InvalidFlow(format!("graph {name}: {error}")))
}

/// The authored text of graph `name`: the repository's file, else the builtin.
pub fn read_workflow_source(name: &str, repo: &Path) -> Result<Option<String>, LoadError> {
    if let Some(path) = crate::flow::find_flow_source_path(name, repo)? {
        return Ok(Some(std::fs::read_to_string(path)?));
    }
    Ok(crate::builtins::resolve_builtin_flow(name)
        .and_then(crate::builtins::get_builtin_flow)
        .map(str::to_owned))
}

pub(crate) fn validate_workflow_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.chars().any(char::is_whitespace) || name.contains([':', '\\']) {
        return Err("invalid graph name".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_workflow, workflow_catalog, CapturedGraph, END, START};
    use crate::flow::runner::ExecutionCursor;

    fn write(repo: &std::path::Path, path: &str, content: &str) {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn routes(definition: &super::WorkflowDefinition, node: &str) -> Vec<(String, String)> {
        definition
            .outgoing(node)
            .map(|(_, edge)| (edge.name.clone(), edge.to.clone()))
            .collect()
    }

    #[test]
    fn builtin_graphs_project_their_stages_and_the_routes_between_them() {
        let repo = tempfile::tempdir().unwrap();
        let feature = load_workflow("feature", repo.path()).unwrap().unwrap();
        let stages: Vec<_> = feature.nodes.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(stages, ["review-design", "demo"]);
        assert_eq!(feature.nodes[0].skill, "review-design");
        assert!(feature.nodes[0].description.is_some());
        assert_eq!(
            routes(&feature, START),
            [("kickoff".to_string(), "review-design".to_string())]
        );
        // From a stage: back around its loop, or on; routes are named by what
        // they run first, the composed graph before the bare skill.
        assert_eq!(
            routes(&feature, "review-design"),
            [
                ("kickoff".to_string(), "review-design".to_string()),
                ("pursue".to_string(), "demo".to_string())
            ]
        );
        assert_eq!(
            routes(&feature, "demo"),
            [
                ("pursue".to_string(), "demo".to_string()),
                ("deliver".to_string(), END.to_string())
            ]
        );
        // A Task with no PR: its last route runs nothing.
        let research = load_workflow("research", repo.path()).unwrap().unwrap();
        assert_eq!(
            routes(&research, "research"),
            [
                ("research".to_string(), "research".to_string()),
                (END.to_string(), END.to_string())
            ]
        );
        assert!(load_workflow("code", repo.path()).unwrap().is_some());
        // A graph without stages is one route from start to end.
        let pursue = load_workflow("pursue", repo.path()).unwrap().unwrap();
        assert!(pursue.nodes.is_empty());
        assert_eq!(
            routes(&pursue, START),
            [("implement".to_string(), END.to_string())]
        );
        assert!(load_workflow("no-such-graph", repo.path())
            .unwrap()
            .is_none());
    }

    #[test]
    fn routes_begin_where_their_slice_runs() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let (definition, routes) = graph.project().unwrap();
        let start = |node: &str, name: &str| {
            let (index, _) = definition
                .outgoing(node)
                .find(|(_, edge)| edge.name == name)
                .unwrap();
            routes[index as usize].start.clone().map(|c| c.index)
        };
        assert_eq!(start(START, "kickoff"), Some(0));
        // Repeating review-design returns to kickoff; advancing runs pursue's first step.
        assert_eq!(start("review-design", "kickoff"), Some(0));
        let pursue = start("review-design", "pursue").unwrap();
        assert!(
            matches!(&graph.steps[pursue], crate::flow::ConcreteStep::Skill(s) if s.sources.get(1).map(String::as_str) == Some("pursue"))
        );
        assert_eq!(start("demo", "pursue"), Some(pursue));
        let deliver = start("demo", "deliver").unwrap();
        assert!(
            matches!(&graph.steps[deliver], crate::flow::ConcreteStep::Skill(s) if s.skill.name == "gate")
        );
        let research = CapturedGraph::load("research", repo.path())
            .unwrap()
            .unwrap();
        let (definition, routes) = research.project().unwrap();
        let (end, _) = definition
            .outgoing("research")
            .find(|(_, edge)| edge.to == END)
            .unwrap();
        assert_eq!(routes[end as usize].start, None);
        assert_eq!(
            CapturedGraph::unplanned().project().unwrap().1,
            [super::Route { start: None }]
        );
    }

    #[test]
    fn a_stage_inside_an_xor_path_is_a_conditional_stage() {
        let repo = tempfile::tempdir().unwrap();
        write(
            repo.path(),
            ".lf/flows/conditional.yaml",
            "- implement\n- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - interactive: demo\n      skip:\n        description: Nothing to review\n- gate\n",
        );
        let graph = CapturedGraph::load("conditional", repo.path())
            .unwrap()
            .unwrap();
        let (definition, starts) = graph.project().unwrap();
        assert_eq!(
            routes(&definition, START),
            [("implement".to_string(), "demo".to_string())]
        );
        assert_eq!(
            routes(&definition, "demo"),
            [("gate".to_string(), END.to_string())]
        );
        assert_eq!(
            starts[1].start,
            Some(ExecutionCursor {
                index: 2,
                ..ExecutionCursor::default()
            })
        );
    }

    #[test]
    fn graphs_that_cannot_be_routed_or_compiled_are_listed_with_the_reason() {
        let repo = tempfile::tempdir().unwrap();
        write(
            repo.path(),
            ".lf/flows/twice.yaml",
            "- implement\n- interactive: demo\n- loop: implement\n  interactive: demo\n",
        );
        write(
            repo.path(),
            ".lf/flows/across.yaml",
            "- implement\n- interactive: demo\n- loop: implement\n",
        );
        write(
            repo.path(),
            ".lf/flows/same-route.yaml",
            "- implement\n- loop: implement\n  interactive: demo\n- implement\n",
        );
        let catalog = workflow_catalog(repo.path()).unwrap();
        let reason = |name: &str| {
            catalog
                .iter()
                .find(|entry| entry.name == name)
                .unwrap()
                .unavailable
                .clone()
                .unwrap()
        };
        assert!(
            reason("twice").contains("two stages are named \"demo\""),
            "{}",
            reason("twice")
        );
        assert!(
            reason("across").contains("returns across stage"),
            "{}",
            reason("across")
        );
        assert!(
            reason("same-route").contains("two routes leave demo as implement"),
            "{}",
            reason("same-route")
        );
        assert_eq!(
            catalog
                .iter()
                .find(|entry| entry.name == "feature")
                .unwrap()
                .source,
            None
        );
    }
}
