//! Display projection of one compiled Flow definition.
//!
//! Surfaces draw the Flow from this shape instead of parsing YAML or inferring
//! topology from skill names. Node keys are preorder IDs, including every
//! sorted XOR alternative. Authored names and runtime cursor paths remain
//! separate; repeated skills keep distinct IDs within their Flow.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::flow::{flatten_resolved, resolve_flow, return_target, ResolvedFlowItem};

use std::path::Path;

use crate::engine::execution::{ExecutionCursor, NestedCursor};
use crate::engine::flow::ConcreteStep;
use crate::engine::workflow::{self, WorkflowDefinition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGraph {
    pub name: String,
    pub steps: Vec<FlowNode>,
    pub interactions: Box<InteractionGraph>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowNode {
    pub key: u32,
    /// Authored occurrence id, when the definition names one.
    pub id: Option<String>,
    /// Literal skill name, operation, or XOR router.
    pub label: String,
    pub kind: FlowNodeKind,
    pub human: bool,
    /// Key of the earlier node this deciding occurrence can return to.
    pub returns_to: Option<u32>,
    /// Composed Flows this occurrence was compiled from, outermost first.
    pub sources: Vec<String>,
    /// XOR alternatives, sorted by name; empty for other kinds.
    pub paths: Vec<FlowGraphPath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowNodeKind {
    Skill,
    Op,
    Xor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGraphPath {
    pub name: String,
    pub description: String,
    pub steps: Vec<FlowNode>,
}

/// Execution evidence for one backward edge in the pinned invocation. The edge
/// itself is the deciding node's `returns_to`; this carries only its counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowReturn {
    /// Key of the deciding occurrence that owns the edge.
    pub decider: u32,
    pub traversals: u32,
}

/// Disclosure structure from the same resolution that supplies the execution graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowTemplate {
    /// Digest of resolved content and composition; unrelated to invocation identity.
    pub revision: String,
    pub items: Vec<FlowTemplateItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlowTemplateItem {
    Node {
        key: u32,
        paths: BTreeMap<String, Vec<FlowTemplateItem>>,
    },
    Group {
        id: String,
        name: String,
        items: Vec<FlowTemplateItem>,
    },
}

fn template_items(
    items: &[ResolvedFlowItem],
    next: &mut u32,
    group: &mut usize,
) -> Vec<FlowTemplateItem> {
    items
        .iter()
        .map(|item| {
            if let ResolvedFlowItem::Group { name, items } = item {
                let id = format!("group-{}", *group);
                *group += 1;
                return FlowTemplateItem::Group {
                    id,
                    name: name.clone(),
                    items: template_items(items, next, group),
                };
            }
            let key = *next;
            *next += 1;
            let paths = if let ResolvedFlowItem::Xor { paths, .. } = item {
                paths
                    .iter()
                    .map(|(name, path)| (name.clone(), template_items(&path.items, next, group)))
                    .collect()
            } else {
                BTreeMap::new()
            };
            FlowTemplateItem::Node { key, paths }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogKind {
    Flow,
    Workflow,
}

/// One Flow or workflow a Task can run, as it would be captured if started now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCatalogEntry {
    pub name: String,
    pub kind: CatalogKind,
    /// The repository file that defines it; `None` for a builtin.
    pub source: Option<String>,
    /// A Flow's topology; `None` for a workflow or an unusable definition.
    pub graph: Option<FlowGraph>,
    pub template: Option<FlowTemplate>,
    /// A workflow's nodes and edges; `None` for a Flow or an unusable definition.
    pub workflow: Option<WorkflowDefinition>,
    /// Why the definition is unusable; set exactly when it has no topology.
    pub unavailable: Option<String>,
}

/// Every Flow and workflow available in `repo`, each read through the shared
/// loader. A file that does not load stays listed with the reason.
pub fn flow_catalog(repo: &Path) -> Vec<FlowCatalogEntry> {
    let source = |path: Option<std::path::PathBuf>| {
        let path = path?;
        let relative = path.strip_prefix(repo).unwrap_or(&path);
        Some(relative.to_string_lossy().into_owned())
    };
    let flows = crate::engine::available_flow_names(repo)
        .into_iter()
        .map(|name| {
            let compiled = crate::engine::load_flow(&name, repo)
                .map_err(|error| error.to_string())
                .and_then(|flow| {
                    let resolved = resolve_flow(&flow, repo).map_err(|error| error.to_string())?;
                    let bytes = serde_json::to_vec(&(&flow.name, &resolved))
                        .map_err(|error| error.to_string())?;
                    let template = FlowTemplate {
                        revision: hex::encode(Sha256::digest(bytes)),
                        items: template_items(&resolved, &mut 0, &mut 0),
                    };
                    Ok((
                        FlowGraph::new(&flow.name, &flatten_resolved(&resolved)),
                        template,
                    ))
                });
            let (topology, unavailable) = match compiled {
                Ok(topology) => (Some(topology), None),
                Err(reason) => (None, Some(reason)),
            };
            let (graph, template) = topology.unzip();
            FlowCatalogEntry {
                source: source(crate::engine::flow::find_flow_source_path(&name, repo)),
                name,
                kind: CatalogKind::Flow,
                graph,
                template,
                workflow: None,
                unavailable,
            }
        });
    let workflows = workflow::available_workflow_names(repo)
        .into_iter()
        .map(|name| {
            let (workflow, unavailable) = match workflow::load_workflow(&name, repo) {
                Ok(workflow) => (workflow, None),
                Err(error) => (None, Some(error.to_string())),
            };
            FlowCatalogEntry {
                source: source(workflow::workflow_path(&name, repo)),
                name,
                kind: CatalogKind::Workflow,
                graph: None,
                template: None,
                workflow,
                unavailable,
            }
        });
    flows.chain(workflows).collect()
}

/// Participation stages and bounded references to the captured automated routes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionGraph {
    pub stages: Vec<String>,
    pub transitions: Vec<InteractionTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionTransition {
    /// Structural human occurrence or `@start` / `@end` anchor.
    pub from: String,
    pub to: String,
    pub nodes: Vec<String>,
    pub routes: Vec<InteractionRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionRoute {
    pub from: String,
    pub to: String,
    /// Authored XOR path name, or the deciding occurrence's repeat/advance route.
    pub condition: Option<String>,
}

pub fn project_interactions(graph: &FlowGraph) -> InteractionGraph {
    project_interaction_nodes(&graph.steps)
}

fn project_interaction_nodes(steps: &[FlowNode]) -> InteractionGraph {
    use std::collections::BTreeSet;
    fn connect(
        steps: &[FlowNode],
        end: &str,
        routes: &mut Vec<InteractionRoute>,
        stages: &mut Vec<String>,
    ) {
        for (index, node) in steps.iter().enumerate() {
            let next = steps
                .get(index + 1)
                .map_or_else(|| end.to_string(), |node| node.key.to_string());
            if node.human {
                stages.push(node.key.to_string());
            }
            if node.kind == FlowNodeKind::Xor {
                for path in &node.paths {
                    routes.push(InteractionRoute {
                        from: node.key.to_string(),
                        to: path
                            .steps
                            .first()
                            .map_or_else(|| next.clone(), |node| node.key.to_string()),
                        condition: Some(path.name.clone()),
                    });
                    connect(&path.steps, &next, routes, stages);
                }
            } else {
                routes.push(InteractionRoute {
                    from: node.key.to_string(),
                    to: next,
                    condition: node.returns_to.as_ref().map(|_| "advance".into()),
                });
            }
            if let Some(target) = &node.returns_to {
                routes.push(InteractionRoute {
                    from: node.key.to_string(),
                    to: target.to_string(),
                    condition: Some("repeat".into()),
                });
            }
        }
    }
    let mut routes = vec![InteractionRoute {
        from: "@start".into(),
        to: steps
            .first()
            .map_or_else(|| "@end".to_string(), |node| node.key.to_string()),
        condition: None,
    }];
    let mut stages = Vec::new();
    connect(steps, "@end", &mut routes, &mut stages);
    let boundaries: BTreeSet<_> = stages.iter().map(String::as_str).chain(["@end"]).collect();
    let mut transitions = Vec::new();
    for source in std::iter::once("@start").chain(stages.iter().map(String::as_str)) {
        let mut visited = BTreeSet::new();
        let mut pending = vec![source];
        let mut reachable_routes = Vec::new();
        let mut targets = BTreeSet::new();
        while let Some(current) = pending.pop() {
            if !visited.insert(current) {
                continue;
            }
            for route in routes.iter().filter(|route| route.from == current) {
                reachable_routes.push(route);
                if boundaries.contains(route.to.as_str()) {
                    targets.insert(route.to.as_str());
                } else {
                    pending.push(route.to.as_str());
                }
            }
        }
        for target in targets {
            // Stop at every other stage before walking backward. In particular,
            // a repeat back to the source cannot leak work into its exit edge.
            let candidates: Vec<_> = reachable_routes
                .iter()
                .copied()
                .filter(|route| route.to == target || !boundaries.contains(route.to.as_str()))
                .collect();
            let mut ancestors = BTreeSet::from([target]);
            loop {
                let before = ancestors.len();
                for route in &candidates {
                    if ancestors.contains(route.to.as_str()) {
                        ancestors.insert(route.from.as_str());
                    }
                }
                if ancestors.len() == before {
                    break;
                }
            }
            let selected: Vec<_> = candidates
                .into_iter()
                .filter(|route| ancestors.contains(route.to.as_str()))
                .cloned()
                .collect();
            let nodes = selected
                .iter()
                .flat_map(|route| [&route.from, &route.to])
                .filter(|key| !key.starts_with('@') && !boundaries.contains(key.as_str()))
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            transitions.push(InteractionTransition {
                from: source.into(),
                to: target.into(),
                nodes,
                routes: selected,
            });
        }
    }
    InteractionGraph {
        stages,
        transitions,
    }
}

impl FlowGraph {
    pub fn new(name: impl Into<String>, steps: &[ConcreteStep]) -> Self {
        let steps = nodes(steps, &mut 0);
        let interactions = Box::new(project_interaction_nodes(&steps));
        Self {
            name: name.into(),
            steps,
            interactions,
        }
    }

    /// Look up a captured node, including nodes inside unselected alternatives.
    pub(crate) fn node_at(&self, id: u32) -> Option<&FlowNode> {
        fn find(nodes: &[FlowNode], id: u32) -> Option<&FlowNode> {
            nodes.iter().find_map(|node| {
                if node.key == id {
                    Some(node)
                } else {
                    node.paths.iter().find_map(|path| find(&path.steps, id))
                }
            })
        }
        find(&self.steps, id)
    }
}

fn nodes(steps: &[ConcreteStep], next: &mut u32) -> Vec<FlowNode> {
    let mut result: Vec<FlowNode> = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        let key = *next;
        *next = next
            .checked_add(1)
            .expect("captured Flow fits u32 node IDs");
        let node = match step {
            ConcreteStep::Skill(skill) => FlowNode {
                returns_to: return_target(steps, index).map(|target| result[target].key),
                key,
                id: skill.id.clone(),
                label: skill.skill.name.clone(),
                kind: FlowNodeKind::Skill,
                human: skill.human,
                sources: skill.sources.clone(),
                paths: Vec::new(),
            },
            ConcreteStep::Command(op) => FlowNode {
                key,
                id: None,
                label: op.item.display_name(),
                kind: FlowNodeKind::Op,
                human: false,
                returns_to: None,
                sources: op.sources.clone(),
                paths: Vec::new(),
            },
            ConcreteStep::Xor(branch) => {
                let mut names: Vec<_> = branch.paths.keys().collect();
                names.sort();
                let paths = names
                    .into_iter()
                    .map(|name| {
                        let path = &branch.paths[name];
                        FlowGraphPath {
                            name: name.clone(),
                            description: path.description.clone(),
                            steps: nodes(&path.steps, next),
                        }
                    })
                    .collect();
                FlowNode {
                    key,
                    id: None,
                    label: branch.router.name.clone(),
                    kind: FlowNodeKind::Xor,
                    human: false,
                    returns_to: None,
                    sources: branch.sources.clone(),
                    paths,
                }
            }
        };
        result.push(node);
    }
    result
}

/// Exact occurrence in this compiled graph: its preorder node key, counting
/// every XOR alternative, and the returns taken at each nesting level.
pub(crate) fn location(
    steps: &[ConcreteStep],
    cursor: &ExecutionCursor,
) -> anyhow::Result<(u32, Vec<Vec<u32>>)> {
    fn count(steps: &[ConcreteStep]) -> usize {
        steps
            .iter()
            .map(|step| {
                1 + match step {
                    ConcreteStep::Xor(branch) => {
                        branch.paths.values().map(|path| count(&path.steps)).sum()
                    }
                    _ => 0,
                }
            })
            .sum()
    }
    fn locate(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> anyhow::Result<usize> {
        let step = steps
            .get(cursor.index)
            .ok_or_else(|| anyhow::anyhow!("cursor has no captured node"))?;
        let mut index = count(&steps[..cursor.index]);
        if let Some(child) = cursor.child.as_deref() {
            let (ConcreteStep::Xor(branch), NestedCursor::Xor { selected, cursor }) = (step, child)
            else {
                anyhow::bail!("cursor child does not belong to a captured XOR");
            };
            let mut paths: Vec<_> = branch.paths.iter().collect();
            paths.sort_by_key(|(name, _)| *name);
            index += 1;
            for (name, path) in paths {
                if name == selected {
                    return Ok(index + locate(&path.steps, cursor)?);
                }
                index += count(&path.steps);
            }
            anyhow::bail!("cursor selects an uncaptured XOR alternative");
        }
        Ok(index)
    }
    Ok((
        u32::try_from(locate(steps, cursor)?)?,
        flow_iterations(steps, cursor),
    ))
}

/// Ordered backward-edge traversal counts at each active nesting level, outermost
/// first. Empty levels are retained so nested counts never masquerade as root
/// counts. Visit tokens used for human-boundary identity are deliberately absent.
pub fn flow_iterations(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> Vec<Vec<u32>> {
    let counts = (0..steps.len())
        .filter(|index| return_target(steps, *index).is_some())
        .map(|index| {
            let traversals = cursor.progress.repeats.get(&index.to_string());
            traversals.copied().unwrap_or(0)
        })
        .collect();
    let mut levels = vec![counts];
    if let (
        Some(ConcreteStep::Xor(branch)),
        Some(NestedCursor::Xor {
            selected,
            cursor: child,
        }),
    ) = (steps.get(cursor.index), cursor.child.as_deref())
    {
        if let Some(path) = branch.paths.get(selected) {
            levels.extend(flow_iterations(&path.steps, child));
        }
    }
    levels
}

/// Recorded return counts (see `flow_iterations`) in words: the pass each
/// returned loop is on, loops numbered in authored order. `None` on a first pass.
pub fn loop_passes(iterations: &[Vec<u32>]) -> Option<String> {
    let counts: Vec<u32> = iterations.iter().flatten().copied().collect();
    let passes: Vec<String> = counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(index, count)| {
            let pass = u64::from(*count) + 1;
            if counts.len() == 1 {
                format!("pass {pass}")
            } else {
                format!("loop {} pass {pass}", index + 1)
            }
        })
        .collect();
    (!passes.is_empty()).then(|| passes.join(", "))
}

/// Where a recorded step stands inside its Flow's graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionProjection {
    /// Key of the current occurrence; `None` once the Flow finished.
    pub current: Option<u32>,
    /// Occurrences already finished in the current pass, including the router of
    /// a selected XOR. Earlier passes' completions are not carried forward.
    pub completed: Vec<u32>,
    pub returns: Vec<FlowReturn>,
}

/// Project the step at `key`, which recorded `iterations` (see
/// `flow_iterations`), onto `graph`. Deciders outside the step's own path
/// report no traversals.
pub fn project_position(
    graph: &FlowGraph,
    key: u32,
    iterations: &[Vec<u32>],
    finished: bool,
) -> PositionProjection {
    fn contains(node: &FlowNode, key: u32) -> bool {
        node.key == key
            || node
                .paths
                .iter()
                .any(|path| path.steps.iter().any(|node| contains(node, key)))
    }
    fn walk(
        nodes: &[FlowNode],
        key: u32,
        levels: &[Vec<u32>],
        active: bool,
        projection: &mut PositionProjection,
    ) {
        let counts = levels.first().filter(|_| active);
        let on_path = nodes.iter().position(|node| active && contains(node, key));
        let mut decider = 0;
        for (index, node) in nodes.iter().enumerate() {
            if on_path.is_some_and(|current| index < current) {
                projection.completed.push(node.key);
            }
            if node.returns_to.is_some() {
                projection.returns.push(FlowReturn {
                    decider: node.key,
                    traversals: counts
                        .and_then(|counts| counts.get(decider))
                        .copied()
                        .unwrap_or(0),
                });
                decider += 1;
            }
            let here = on_path == Some(index);
            if here && node.key == key {
                projection.current = Some(key);
            } else if here {
                projection.completed.push(node.key);
            }
            for path in &node.paths {
                let selected = here && path.steps.iter().any(|node| contains(node, key));
                walk(
                    &path.steps,
                    key,
                    levels.get(1..).unwrap_or_default(),
                    selected,
                    projection,
                );
            }
        }
    }
    let mut projection = PositionProjection {
        current: None,
        completed: Vec::new(),
        returns: Vec::new(),
    };
    walk(&graph.steps, key, iterations, true, &mut projection);
    if finished {
        projection.current = None;
        projection.completed = graph.steps.iter().map(|node| node.key).collect();
    }
    projection
}

#[cfg(test)]
mod tests {
    #[test]
    fn template_resolution_preserves_composition_and_execution() {
        use super::{flow_catalog, FlowGraph, FlowTemplateItem};
        use crate::engine::flow::{compile_flow, load_flow};
        let repo = tempfile::tempdir().unwrap();
        let flows = repo.path().join(".lf/flows");
        let skills = repo.path().join(".lf/skills");
        std::fs::create_dir_all(&flows).unwrap();
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("sample.md"), "First content").unwrap();
        std::fs::write(flows.join("piece.yaml"), "- sample\n").unwrap();
        std::fs::write(flows.join("empty.yaml"), "[]\n").unwrap();
        std::fs::write(
            flows.join("study.yaml"),
            r#"
- flow: piece
- flow: piece
- flow: empty
- xor:
    paths:
      fix:
        description: Fix it
        flow: piece
      pass:
        description: Continue
        steps: []
- step: {name: sample, id: start}
- loop: start
- demo
- loop: start
"#,
        )
        .unwrap();
        let entry = flow_catalog(repo.path())
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert!(entry.unavailable.is_none(), "{:?}", entry.unavailable);
        let template = entry.template.unwrap();
        let expanded =
            compile_flow(&load_flow("study", repo.path()).unwrap(), repo.path()).unwrap();
        let graph = entry.graph.unwrap();
        assert_eq!(graph, FlowGraph::new("study", &expanded));
        assert_eq!(
            graph
                .steps
                .iter()
                .filter(|n| n.returns_to.is_some())
                .count(),
            2
        );
        let [FlowTemplateItem::Group { id: first, .. }, FlowTemplateItem::Group { id: second, .. }, FlowTemplateItem::Group { items: empty, .. }, FlowTemplateItem::Node { paths, .. }, ..] =
            template.items.as_slice()
        else {
            panic!("expected distinct composition uses and XOR");
        };
        assert_ne!(first, second);
        assert!(empty.is_empty());
        assert_eq!(paths.len(), 2);
        let FlowTemplateItem::Group { items, .. } = &paths["fix"][0] else {
            panic!("expected nested group");
        };
        assert!(matches!(&items[0], FlowTemplateItem::Node { key, .. } if *key == 3));
        assert!(paths["pass"].is_empty());
        let reread = flow_catalog(repo.path())
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert_eq!(
            reread.template.as_ref().unwrap().revision,
            template.revision
        );
        std::fs::write(skills.join("sample.md"), "Changed content").unwrap();
        let changed = flow_catalog(repo.path())
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert_ne!(changed.template.unwrap().revision, template.revision);
        std::fs::write(flows.join("piece.yaml"), "- flow: study\n").unwrap();
        let cyclic = flow_catalog(repo.path())
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert!(cyclic.graph.is_none() && cyclic.template.is_none());
        assert!(cyclic.unavailable.unwrap().contains("cycle"));
        std::fs::remove_file(flows.join("piece.yaml")).unwrap();
        let missing = flow_catalog(repo.path())
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert!(missing.graph.is_none() && missing.template.is_none());
        assert!(missing.unavailable.is_some());
    }

    use std::collections::{BTreeMap, HashMap};

    use crate::engine::execution::{ExecutionCursor, NestedCursor};
    use crate::engine::flow::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, Skill};
    use crate::engine::flow_graph::{
        flow_iterations, project_position, FlowGraph, FlowNodeKind, PositionProjection,
    };
    use crate::engine::{compile_flow, load_flow};

    /// The projection of the step `cursor` selects, as its driver records it.
    fn project(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> PositionProjection {
        let (key, iterations) = super::location(steps, cursor).unwrap();
        project_position(&FlowGraph::new("", steps), key, &iterations, false)
    }

    fn skill(name: &str, id: Option<&str>, human: bool, returns: Option<usize>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(name),
            id: id.map(str::to_string),
            human,
            returns,
            sources: Vec::new(),
        })
    }

    #[test]
    fn participation_preserves_loops_repeated_skills_and_background_only_flows() {
        let steps = vec![
            skill("review", Some("start"), true, None),
            skill("implement", Some("build"), false, None),
            skill("decide", None, false, Some(1)),
            skill("review", Some("second"), true, None),
            skill("decide", None, false, Some(4)),
        ];
        let graph = FlowGraph::new("example", &steps);
        let projection = super::project_interactions(&graph);
        assert_eq!(projection.stages, ["0", "3"]);
        let edge = projection
            .transitions
            .iter()
            .find(|e| e.from == "0" && e.to == "3")
            .unwrap();
        assert_eq!(edge.nodes, ["1", "2"]);
        assert!(edge
            .routes
            .iter()
            .any(|r| r.from == "2" && r.to == "1" && r.condition.as_deref() == Some("repeat")));
        assert!(projection
            .transitions
            .iter()
            .any(|e| e.from == "3" && e.to == "0"));
        let background = FlowGraph::new("background", &steps[1..3]);
        assert!(background.interactions.stages.is_empty());
        assert_eq!(background.interactions.transitions.len(), 1);
        assert_eq!(background.interactions.transitions[0].nodes, ["0", "1"]);
    }

    #[test]
    fn participation_exit_excludes_work_that_requires_another_review() {
        let graph = FlowGraph::new(
            "review-loop",
            &[
                skill("build", Some("build"), false, None),
                skill("review", None, true, None),
                skill("decide", None, false, Some(2)),
            ],
        );
        let exit = graph
            .interactions
            .transitions
            .iter()
            .find(|edge| edge.from == "1" && edge.to == "@end")
            .unwrap();
        assert_eq!(exit.nodes, ["2"]);
        assert_eq!(exit.routes.len(), 2);
        assert!(exit.routes.iter().all(|route| route.to != "0"));
        let repeat = graph
            .interactions
            .transitions
            .iter()
            .find(|edge| edge.from == "1" && edge.to == "1")
            .unwrap();
        assert_eq!(repeat.nodes, ["0", "2"]);
    }

    #[test]
    fn participation_fixtures_preserve_exact_branch_alternatives() {
        let entries: Vec<super::FlowCatalogEntry> = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/flow_catalog.json"
        ))
        .unwrap();
        for entry in entries {
            if let Some(graph) = entry.graph {
                assert_eq!(*graph.interactions, super::project_interactions(&graph));
            }
        }
        let graph = FlowGraph::new(
            "branch",
            &[
                skill("review", None, true, None),
                ConcreteStep::Xor(ConcreteXor {
                    router: Skill::named("route"),
                    paths: HashMap::from([
                        (
                            "build".into(),
                            ConcretePath {
                                description: "build it".into(),
                                steps: vec![
                                    skill("implement", None, false, None),
                                    skill("review", None, true, None),
                                ],
                            },
                        ),
                        (
                            "skip".into(),
                            ConcretePath {
                                description: "skip it".into(),
                                steps: vec![],
                            },
                        ),
                    ]),
                    sources: vec![],
                }),
                skill("finish", None, true, None),
            ],
        );
        assert_eq!(graph.interactions.stages, ["0", "3", "4"]);
        let edges: Vec<_> = graph
            .interactions
            .transitions
            .iter()
            .filter(|edge| edge.from == "0")
            .collect();
        assert_eq!(edges.len(), 2);
        let skipped = edges.iter().find(|edge| edge.to == "4").unwrap();
        assert!(skipped
            .routes
            .iter()
            .any(|r| r.condition.as_deref() == Some("skip")));
        assert!(!skipped.nodes.contains(&"2".into()));
    }

    #[test]
    fn builtin_flows_keep_their_delivery_boundaries_without_review_steps() {
        let repo = tempfile::tempdir().unwrap();
        let cases: &[(&str, &[&str], &[usize], usize)] = &[
            ("queue", &["compress", "sync", "realign", "gate"], &[], 0),
            ("refresh", &["sync", "realign"], &[], 0),
            ("task-design", &["kickoff"], &[], 0),
            ("incident", &["unbreak", "5whys", "launch-plan"], &[], 0),
            (
                "pursue",
                &[
                    "implement",
                    "compress",
                    "sync",
                    "realign",
                    "loop-or-next",
                    "pr-publish",
                ],
                &[],
                1,
            ),
            ("deploy", &["gate", "pr land"], &[], 0),
            ("ship", &["gate", "pr land -c"], &[], 0),
            ("vsm-operate", &["s1", "s2", "s3", "s4", "s5"], &[], 0),
        ];
        for (name, labels, humans, returns) in cases {
            let flow = load_flow(name, repo.path()).unwrap();
            let graph = FlowGraph::new(*name, &compile_flow(&flow, repo.path()).unwrap());
            assert_eq!(
                graph
                    .steps
                    .iter()
                    .map(|s| s.label.as_str())
                    .collect::<Vec<_>>(),
                *labels,
                "{name}"
            );
            assert_eq!(
                graph
                    .steps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| s.human.then_some(i))
                    .collect::<Vec<_>>(),
                *humans,
                "{name}"
            );
            assert_eq!(
                graph
                    .steps
                    .iter()
                    .filter(|s| s.returns_to.is_some())
                    .count(),
                *returns,
                "{name}"
            );
        }
    }

    #[test]
    fn refresh_integrates_upstream_before_realigning() {
        let repo = tempfile::tempdir().unwrap();
        let flow = load_flow("refresh", repo.path()).unwrap();
        let graph = FlowGraph::new(&flow.name, &compile_flow(&flow, repo.path()).unwrap());
        let labels: Vec<_> = graph.steps.iter().map(|node| node.label.as_str()).collect();
        assert_eq!(labels, ["sync", "realign"]);
        assert_eq!(graph.steps[0].kind, FlowNodeKind::Op);
        assert_eq!(graph.steps[1].kind, FlowNodeKind::Skill);
    }

    #[test]
    fn pursue_loops_to_a_published_pr() {
        let repo = tempfile::tempdir().unwrap();
        let flow = load_flow("pursue", repo.path()).unwrap();
        let graph = FlowGraph::new(&flow.name, &compile_flow(&flow, repo.path()).unwrap());
        let labels: Vec<_> = graph.steps.iter().map(|node| node.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "implement",
                "compress",
                "sync",
                "realign",
                "loop-or-next",
                "pr-publish",
            ]
        );
        let implement = graph.steps[0].key;
        let returns: Vec<_> = graph
            .steps
            .iter()
            .filter_map(|node| node.returns_to.as_ref().map(|to| (node.key, to)))
            .collect();
        assert_eq!(returns, [(graph.steps[4].key, &implement)]);
        // Review and landing are the conversation's to launch, not this Flow's.
        assert!(graph.steps.iter().all(|node| !node.human));
        assert!(graph
            .steps
            .iter()
            .all(|node| node.kind != FlowNodeKind::Op || node.label == "sync"));
        assert_eq!(graph.steps[3].sources, ["pursue", "refresh"]);
    }

    #[test]
    fn repeated_decisions_keep_independent_counts_and_a_pass_scoped_completion() {
        let steps = vec![
            skill("design", Some("design"), false, None),
            skill("implement", Some("implement"), false, None),
            skill("loop-or-next", None, false, Some(1)),
            skill("demo", Some("demo"), true, None),
            skill("loop-or-next", None, false, Some(3)),
            skill("land", None, false, None),
        ];
        let cursor = ExecutionCursor {
            index: 1,
            iteration: 3,
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([("2".into(), 2), ("4".into(), 1)]),
                ..Default::default()
            },
            ..Default::default()
        };
        let projection = project(&steps, &cursor);
        assert_eq!(projection.current, Some(1));
        // Only the opening step is complete; earlier passes' decisions are not.
        assert_eq!(projection.completed, [0]);
        let counts: Vec<_> = projection
            .returns
            .iter()
            .map(|edge| (edge.decider, edge.traversals))
            .collect();
        assert_eq!(counts, [(2, 2), (4, 1)]);
        assert_eq!(flow_iterations(&steps, &cursor), [vec![2, 1]]);
        // Moving beyond one span does not collapse its independent count.
        let later = ExecutionCursor {
            index: 3,
            iteration: 99,
            ..cursor
        };
        assert_eq!(flow_iterations(&steps, &later), [vec![2, 1]]);
    }

    #[test]
    fn a_selected_xor_path_is_drawn_honestly_with_captured_ids() {
        let branch = ConcreteXor {
            router: Skill::named("xor-route"),
            paths: HashMap::from([
                (
                    "fix".to_string(),
                    ConcretePath {
                        description: "Fix it".into(),
                        steps: vec![
                            skill("patch", Some("patch"), false, None),
                            skill("check", Some("check"), false, Some(1)),
                        ],
                    },
                ),
                (
                    "skip".to_string(),
                    ConcretePath {
                        description: "Nothing to do".into(),
                        steps: Vec::new(),
                    },
                ),
            ]),
            sources: Vec::new(),
        };
        let steps = vec![
            skill("kickoff", None, false, None),
            ConcreteStep::Xor(branch),
        ];
        let graph = FlowGraph::new("routed", &steps);
        let xor = &graph.steps[1];
        assert_eq!(xor.kind, FlowNodeKind::Xor);
        assert_eq!(
            xor.paths
                .iter()
                .map(|path| path.name.as_str())
                .collect::<Vec<_>>(),
            ["fix", "skip"]
        );
        assert_eq!(xor.paths[0].steps[1].key, 3);
        assert_eq!(xor.paths[0].steps[1].returns_to, Some(2));

        let cursor = ExecutionCursor {
            index: 1,
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([("xor:1:fix/1".into(), 4)]),
                ..Default::default()
            },
            child: Some(Box::new(NestedCursor::Xor {
                selected: "fix".into(),
                cursor: ExecutionCursor {
                    index: 1,
                    iteration: 2,
                    progress: crate::engine::transitions::FlowProgress {
                        repeats: BTreeMap::from([("1".into(), 2)]),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            })),
            ..Default::default()
        };
        let projection = project(&steps, &cursor);
        assert_eq!(projection.current, Some(3));
        assert_eq!(projection.completed, [0, 1, 2]);
        // The active child's own count wins over an older settled visit.
        assert_eq!(projection.returns[0].traversals, 2);
        assert_eq!(flow_iterations(&steps, &cursor), [vec![], vec![2]]);
        assert_eq!(
            super::loop_passes(&[vec![], vec![2]]).as_deref(),
            Some("pass 3")
        );
        assert_eq!(
            super::loop_passes(&[vec![2, 0], vec![1]]).as_deref(),
            Some("loop 1 pass 3, loop 3 pass 2")
        );
        assert_eq!(super::loop_passes(&[vec![0, 0]]), None);
    }

    #[test]
    fn numeric_wire_matches_captured_ids_across_nested_alternatives() {
        use crate::ops::task_flow::{TaskFlowRecord, TaskFlowSnapshot};

        fn branch(paths: Vec<(&str, Vec<ConcreteStep>)>) -> ConcreteStep {
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("route"),
                paths: paths
                    .into_iter()
                    .map(|(name, steps)| {
                        (
                            name.into(),
                            ConcretePath {
                                description: name.into(),
                                steps,
                            },
                        )
                    })
                    .collect(),
                sources: Vec::new(),
            })
        }
        fn begin() -> ConcreteStep {
            skill("patch", Some("begin"), false, None)
        }
        fn check(returns: usize) -> ConcreteStep {
            skill("check", Some("check"), false, Some(returns))
        }
        let steps = vec![
            begin(),
            branch(vec![
                ("zeta", vec![begin(), check(1)]),
                (
                    "alpha",
                    vec![
                        begin(),
                        branch(vec![("fix", vec![begin(), check(1)])]),
                        check(2),
                    ],
                ),
            ]),
            check(2),
        ];
        let fixture: TaskFlowSnapshot = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/flow_numeric_nested.json"
        ))
        .unwrap();
        let TaskFlowRecord::Latest(pinned) = fixture.record else {
            panic!("launched Flow fixture")
        };
        let graph = FlowGraph::new("nested", &steps);
        assert_eq!(graph, pinned.graph);
        // Root 0, XOR 1, alpha 2/3/(fix 4/5)/6, zeta 7/8, root 9.
        // Compare every cursor with the existing storage identity algorithm.
        fn cursors(steps: &[ConcreteStep]) -> Vec<ExecutionCursor> {
            let mut result = Vec::new();
            for (index, step) in steps.iter().enumerate() {
                result.push(ExecutionCursor {
                    index,
                    ..Default::default()
                });
                if let ConcreteStep::Xor(branch) = step {
                    for (selected, path) in &branch.paths {
                        for child in cursors(&path.steps) {
                            result.push(ExecutionCursor {
                                index,
                                child: Some(Box::new(NestedCursor::Xor {
                                    selected: selected.clone(),
                                    cursor: child,
                                })),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
            result
        }
        let mut ids = Vec::new();
        for cursor in cursors(&steps) {
            let id = super::location(&steps, &cursor).unwrap().0;
            ids.push(id);
            assert_eq!(project(&steps, &cursor).current, Some(id));
            assert_eq!(graph.node_at(id).unwrap().key, id);
        }
        ids.sort();
        assert_eq!(ids, (0..10).collect::<Vec<_>>());
        let cursor = ExecutionCursor {
            index: 1,
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([("2".into(), 3)]),
                ..Default::default()
            },
            child: Some(Box::new(NestedCursor::Xor {
                selected: "alpha".into(),
                cursor: ExecutionCursor {
                    index: 1,
                    child: Some(Box::new(NestedCursor::Xor {
                        selected: "fix".into(),
                        cursor: ExecutionCursor {
                            index: 1,
                            progress: crate::engine::transitions::FlowProgress {
                                repeats: BTreeMap::from([("1".into(), 2)]),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    })),
                    ..Default::default()
                },
            })),
            ..Default::default()
        };
        let projection = project(&steps, &cursor);
        assert_eq!(projection.current, pinned.current);
        assert_eq!(projection.completed, pinned.completed);
        assert_eq!(projection.returns, pinned.returns);
        assert_eq!(flow_iterations(&steps, &cursor), pinned.iterations);
        // A step outside the alternatives reports their deciders untraversed.
        let last = ExecutionCursor {
            index: 2,
            ..Default::default()
        };
        let projection = project(&steps, &last);
        assert_eq!(projection.current, Some(9));
        assert_eq!(projection.completed, [0, 1]);
        assert_eq!(
            projection
                .returns
                .iter()
                .map(|r| (r.decider, r.traversals))
                .collect::<Vec<_>>(),
            [(5, 0), (6, 0), (8, 0), (9, 0)]
        );
        let finished = project_position(&graph, 9, &[vec![0]], true);
        assert_eq!(finished.current, None);
        assert_eq!(finished.completed, [0, 1, 9]);
    }
}
