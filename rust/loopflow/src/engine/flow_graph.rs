//! Display projection of one captured Flow definition.
//!
//! Surfaces draw the Flow from this shape instead of parsing YAML or inferring
//! topology from skill names. Every node carries a structural key (`3`,
//! `4/fix/1`) that is unique inside the definition and identical to the key a
//! saved [`ExecutionCursor`] selects, so a repeated skill such as `loop-decide`
//! keeps one identity per occurrence.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use std::path::Path;

use crate::engine::execution::{node_key, ExecutionCursor, NestedCursor};
use crate::engine::flow::ConcreteStep;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGraph {
    pub name: String,
    pub steps: Vec<FlowNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowNode {
    pub key: String,
    /// Authored occurrence id, when the definition names one.
    pub id: Option<String>,
    /// Literal skill name, operation, or XOR router.
    pub label: String,
    pub kind: FlowNodeKind,
    pub human: bool,
    /// Key of the earlier node this deciding occurrence can return to.
    pub returns_to: Option<String>,
    /// Composed Flows this occurrence was expanded from, outermost first.
    pub parents: Vec<String>,
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
    pub decider: String,
    pub traversals: u32,
}

/// One selectable Flow and the topology it would pin if started now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCatalogEntry {
    pub name: String,
    /// `None` when the definition cannot be loaded or expanded.
    pub graph: Option<FlowGraph>,
    /// Why the definition is unusable; set exactly when `graph` is `None`.
    pub unavailable: Option<String>,
}

/// Every Flow available in `repo`, each expanded through the shared loader.
pub fn flow_catalog(repo: &Path) -> Vec<FlowCatalogEntry> {
    crate::engine::available_flow_names(repo)
        .into_iter()
        .map(|name| {
            let expanded = crate::engine::load_flow(&name, repo)
                .map_err(|error| error.to_string())
                .and_then(|flow| {
                    crate::engine::expand_flow(&flow, repo)
                        .map(|steps| FlowGraph::new(&flow.name, &steps))
                        .map_err(|error| error.to_string())
                });
            match expanded {
                Ok(graph) => FlowCatalogEntry {
                    name,
                    graph: Some(graph),
                    unavailable: None,
                },
                Err(reason) => FlowCatalogEntry {
                    name,
                    graph: None,
                    unavailable: Some(reason),
                },
            }
        })
        .collect()
}

impl FlowGraph {
    pub fn new(name: impl Into<String>, steps: &[ConcreteStep]) -> Self {
        Self {
            name: name.into(),
            steps: nodes(steps, ""),
        }
    }
}

fn nodes(steps: &[ConcreteStep], prefix: &str) -> Vec<FlowNode> {
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let key = node_key(prefix, index);
            match step {
                ConcreteStep::Skill(skill) => FlowNode {
                    returns_to: return_target(steps, index).map(|target| node_key(prefix, target)),
                    key,
                    id: skill.policy.id.clone(),
                    label: skill.skill.name.clone(),
                    kind: FlowNodeKind::Skill,
                    human: skill.policy.human,
                    parents: skill.flow_parents.clone(),
                    paths: Vec::new(),
                },
                ConcreteStep::Op(op) => FlowNode {
                    key,
                    id: None,
                    label: op.item.display_name(),
                    kind: FlowNodeKind::Op,
                    human: false,
                    returns_to: None,
                    parents: op.flow_parents.clone(),
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
                                steps: nodes(&path.steps, &format!("{key}/{name}/")),
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
                        parents: branch.flow_parents.clone(),
                        paths,
                    }
                }
            }
        })
        .collect()
}

/// Index of the earlier occurrence the deciding skill at `index` returns to.
fn return_target(steps: &[ConcreteStep], index: usize) -> Option<usize> {
    let ConcreteStep::Skill(skill) = &steps[index] else {
        return None;
    };
    let from = skill.policy.repeat.as_ref()?.from.as_str();
    steps[..index].iter().position(|target| {
        matches!(target, ConcreteStep::Skill(target) if target.policy.id.as_deref() == Some(from))
    })
}

/// Ordered backward-edge traversal counts at each active nesting level, outermost
/// first. Empty levels are retained so nested counts never masquerade as root
/// counts. Visit tokens used for human-boundary identity are deliberately absent.
pub fn flow_iterations(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> Vec<Vec<u32>> {
    let counts = steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
            return_target(steps, index)?;
            let ConcreteStep::Skill(skill) = step else {
                return None;
            };
            let id = skill.policy.id.as_ref()?;
            Some(cursor.progress.repeats.get(id).copied().unwrap_or(0))
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

/// Where a saved cursor stands inside its captured definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorProjection {
    /// Key of the current occurrence; `None` when the cursor selects no node.
    pub current: Option<String>,
    /// Occurrences already finished in the current pass, including the router of
    /// a selected XOR. Earlier passes' completions are not carried forward.
    pub completed: Vec<String>,
    pub returns: Vec<FlowReturn>,
}

pub fn project_cursor(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> CursorProjection {
    let mut projection = CursorProjection {
        current: None,
        completed: Vec::new(),
        returns: Vec::new(),
    };
    walk_cursor(steps, "", Some(cursor), &mut projection);
    collect_returns(
        steps,
        "",
        Some(cursor),
        &cursor.progress.repeats,
        &mut projection.returns,
    );
    projection
}

fn walk_cursor(
    steps: &[ConcreteStep],
    prefix: &str,
    cursor: Option<&ExecutionCursor>,
    projection: &mut CursorProjection,
) {
    let Some(cursor) = cursor else { return };
    let index = cursor.index.min(steps.len());
    projection
        .completed
        .extend((0..index).map(|index| node_key(prefix, index)));
    let key = node_key(prefix, cursor.index);
    match (steps.get(cursor.index), cursor.child.as_deref()) {
        (Some(ConcreteStep::Xor(branch)), Some(NestedCursor::Xor { selected, cursor })) => {
            match branch.paths.get(selected) {
                Some(path) if cursor.index < path.steps.len() => {
                    projection.completed.push(key.clone());
                    walk_cursor(
                        &path.steps,
                        &format!("{key}/{selected}/"),
                        Some(cursor),
                        projection,
                    );
                }
                // A completed or unrecorded child still waits on its XOR.
                _ => projection.current = Some(key),
            }
        }
        (Some(_), _) => projection.current = Some(key),
        (None, _) => {}
    }
}

fn collect_returns(
    steps: &[ConcreteStep],
    prefix: &str,
    cursor: Option<&ExecutionCursor>,
    repeats: &BTreeMap<String, u32>,
    out: &mut Vec<FlowReturn>,
) {
    for (index, step) in steps.iter().enumerate() {
        let key = node_key(prefix, index);
        match step {
            ConcreteStep::Skill(skill) => {
                let (Some(id), Some(_target)) = (&skill.policy.id, return_target(steps, index))
                else {
                    continue;
                };
                out.push(FlowReturn {
                    decider: key,
                    traversals: repeats.get(id).copied().unwrap_or(0),
                });
            }
            ConcreteStep::Xor(branch) => {
                let mut names: Vec<_> = branch.paths.keys().collect();
                names.sort();
                for name in names {
                    let child = cursor
                        .filter(|cursor| cursor.index == index)
                        .and_then(|cursor| match cursor.child.as_deref() {
                            Some(NestedCursor::Xor { selected, cursor }) if selected == name => {
                                Some(cursor)
                            }
                            _ => None,
                        });
                    // The active child keeps its own counts; settled visits are
                    // folded into the parent under the path's prefix.
                    let settled_prefix = format!("xor:{index}:{name}/");
                    let settled: BTreeMap<String, u32> = repeats
                        .iter()
                        .filter_map(|(key, count)| {
                            key.strip_prefix(&settled_prefix)
                                .map(|key| (key.to_owned(), *count))
                        })
                        .collect();
                    collect_returns(
                        &branch.paths[name].steps,
                        &format!("{key}/{name}/"),
                        child,
                        child.map_or(&settled, |child| &child.progress.repeats),
                        out,
                    );
                }
            }
            ConcreteStep::Op(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use crate::engine::execution::{ExecutionCursor, NestedCursor};
    use crate::engine::flow::{
        ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, OccurrencePolicy, RepeatPolicy,
        Skill,
    };
    use crate::engine::flow_graph::{flow_iterations, project_cursor, FlowGraph, FlowNodeKind};
    use crate::engine::{expand_flow, load_flow};

    fn skill(name: &str, id: Option<&str>, human: bool, from: Option<&str>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(name),
            policy: OccurrencePolicy {
                id: id.map(str::to_string),
                human,
                repeat: from.map(|from| RepeatPolicy {
                    from: from.to_string(),
                }),
            },
            flow_parents: Vec::new(),
        })
    }

    #[test]
    fn feature_draws_both_returns_to_implement_with_forward_delivery() {
        let repo = tempfile::tempdir().unwrap();
        let flow = load_flow("feature", repo.path()).unwrap();
        let graph = FlowGraph::new(&flow.name, &expand_flow(&flow, repo.path()).unwrap());
        let labels: Vec<_> = graph.steps.iter().map(|node| node.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "kickoff",
                "review-design",
                "implement",
                "compress",
                "review-slice",
                "concept-review",
                "loop-decide",
                "demo",
                "loop-decide",
                "compress",
                "update-wave",
                "gate",
                "pr land -c"
            ]
        );
        let implement = graph.steps[2].key.clone();
        let returns: Vec<_> = graph
            .steps
            .iter()
            .filter_map(|node| node.returns_to.as_ref().map(|to| (node.id.clone(), to)))
            .collect();
        assert_eq!(
            returns,
            [
                (Some("decide".to_string()), &implement),
                (Some("decide_delivery".to_string()), &implement)
            ]
        );
        assert!(graph.steps[1].human && graph.steps[7].human);
        assert_eq!(graph.steps[12].kind, FlowNodeKind::Op);
        assert_eq!(graph.steps[6].parents, ["feature", "pursue"]);
    }

    #[test]
    fn repeated_decisions_keep_independent_counts_and_a_pass_scoped_completion() {
        let steps = vec![
            skill("design", Some("design"), false, None),
            skill("implement", Some("implement"), false, None),
            skill("loop-decide", Some("decide"), false, Some("implement")),
            skill("demo", Some("demo"), true, None),
            skill(
                "loop-decide",
                Some("decide_delivery"),
                false,
                Some("implement"),
            ),
            skill("land", None, false, None),
        ];
        let cursor = ExecutionCursor {
            index: 1,
            iteration: 3,
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([("decide".into(), 2), ("decide_delivery".into(), 1)]),
                ..Default::default()
            },
            ..Default::default()
        };
        let projection = project_cursor(&steps, &cursor);
        assert_eq!(projection.current.as_deref(), Some("1"));
        // Only the opening step is complete; earlier passes' decisions are not.
        assert_eq!(projection.completed, ["0"]);
        let counts: Vec<_> = projection
            .returns
            .iter()
            .map(|edge| (edge.decider.as_str(), edge.traversals))
            .collect();
        assert_eq!(counts, [("2", 2), ("4", 1)]);
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
    fn a_selected_xor_path_is_drawn_honestly_with_nested_keys() {
        let branch = ConcreteXor {
            router: Skill::named("xor-route"),
            paths: HashMap::from([
                (
                    "fix".to_string(),
                    ConcretePath {
                        description: "Fix it".into(),
                        steps: vec![
                            skill("patch", Some("patch"), false, None),
                            skill("check", Some("check"), false, Some("patch")),
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
            flow_parents: Vec::new(),
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
        assert_eq!(xor.paths[0].steps[1].key, "1/fix/1");
        assert_eq!(xor.paths[0].steps[1].returns_to.as_deref(), Some("1/fix/0"));

        let cursor = ExecutionCursor {
            index: 1,
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([("xor:1:fix/check".into(), 4)]),
                ..Default::default()
            },
            child: Some(Box::new(NestedCursor::Xor {
                selected: "fix".into(),
                cursor: ExecutionCursor {
                    index: 1,
                    iteration: 2,
                    progress: crate::engine::transitions::FlowProgress {
                        repeats: BTreeMap::from([("check".into(), 2)]),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            })),
            ..Default::default()
        };
        let projection = project_cursor(&steps, &cursor);
        assert_eq!(projection.current.as_deref(), Some("1/fix/1"));
        assert_eq!(projection.completed, ["0", "1", "1/fix/0"]);
        // The active child's own count wins over an older settled visit.
        assert_eq!(projection.returns[0].traversals, 2);
        assert_eq!(flow_iterations(&steps, &cursor), [vec![], vec![2]]);
    }
}
