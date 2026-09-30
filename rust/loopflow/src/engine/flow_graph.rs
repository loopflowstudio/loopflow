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
    pub interactions: Box<InteractionGraph>,
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
            let next = steps.get(index + 1).map_or(end, |node| node.key.as_str());
            if node.human {
                stages.push(node.key.clone());
            }
            if node.kind == FlowNodeKind::Xor {
                for path in &node.paths {
                    routes.push(InteractionRoute {
                        from: node.key.clone(),
                        to: path
                            .steps
                            .first()
                            .map_or(next, |node| node.key.as_str())
                            .into(),
                        condition: Some(path.name.clone()),
                    });
                    connect(&path.steps, next, routes, stages);
                }
            } else {
                routes.push(InteractionRoute {
                    from: node.key.clone(),
                    to: next.into(),
                    condition: node.returns_to.as_ref().map(|_| "advance".into()),
                });
            }
            if let Some(target) = &node.returns_to {
                routes.push(InteractionRoute {
                    from: node.key.clone(),
                    to: target.clone(),
                    condition: Some("repeat".into()),
                });
            }
        }
    }
    let mut routes = vec![InteractionRoute {
        from: "@start".into(),
        to: steps
            .first()
            .map_or("@end", |node| node.key.as_str())
            .into(),
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
        let steps = nodes(steps, "");
        let interactions = Box::new(project_interaction_nodes(&steps));
        Self {
            name: name.into(),
            steps,
            interactions,
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
                    id: skill.id.clone(),
                    label: skill.skill.name.clone(),
                    kind: FlowNodeKind::Skill,
                    human: skill.human,
                    parents: skill.flow_parents.clone(),
                    paths: Vec::new(),
                },
                ConcreteStep::Command(op) => FlowNode {
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
    let from = skill.repeat.as_ref()?.from.as_str();
    steps[..index].iter().position(|target| {
        matches!(target, ConcreteStep::Skill(target) if target.id.as_deref() == Some(from))
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
            let id = skill.id.as_ref()?;
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
                let (Some(id), Some(_target)) = (&skill.id, return_target(steps, index)) else {
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
            ConcreteStep::Command(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use crate::engine::execution::{ExecutionCursor, NestedCursor};
    use crate::engine::flow::{
        ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, RepeatPolicy, Skill,
    };
    use crate::engine::flow_graph::{flow_iterations, project_cursor, FlowGraph, FlowNodeKind};
    use crate::engine::{expand_flow, load_flow};

    fn skill(name: &str, id: Option<&str>, human: bool, from: Option<&str>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(name),
            id: id.map(str::to_string),
            human,
            repeat: from.map(|from| RepeatPolicy {
                from: from.to_string(),
            }),
            flow_parents: Vec::new(),
        })
    }

    #[test]
    fn participation_preserves_loops_repeated_skills_and_background_only_flows() {
        let steps = vec![
            skill("review", Some("start"), true, None),
            skill("implement", Some("build"), false, None),
            skill("decide", None, false, Some("build")),
            skill("review", Some("second"), true, None),
            skill("decide", None, false, Some("start")),
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
                skill("decide", None, false, Some("build")),
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
                    flow_parents: vec![],
                }),
                skill("finish", None, true, None),
            ],
        );
        assert_eq!(graph.interactions.stages, ["0", "1/build/1", "2"]);
        let edges: Vec<_> = graph
            .interactions
            .transitions
            .iter()
            .filter(|edge| edge.from == "0")
            .collect();
        assert_eq!(edges.len(), 2);
        let skipped = edges.iter().find(|edge| edge.to == "2").unwrap();
        assert!(skipped
            .routes
            .iter()
            .any(|r| r.condition.as_deref() == Some("skip")));
        assert!(!skipped.nodes.contains(&"1/build/0".into()));
    }

    #[test]
    fn retained_flows_keep_their_delivery_and_review_boundaries() {
        let repo = tempfile::tempdir().unwrap();
        let cases: &[(&str, &[&str], &[usize], usize)] = &[
            ("code", &["implement", "compress"], &[], 0),
            ("queue", &["compress", "rebase", "realign", "gate"], &[], 0),
            ("refresh", &["rebase", "realign"], &[], 0),
            ("task-design", &["kickoff", "review-design"], &[1], 0),
            ("incident", &["unbreak", "5whys", "launch-plan"], &[], 0),
            (
                "pursue",
                &[
                    "implement",
                    "compress",
                    "rebase",
                    "realign",
                    "loop-decide",
                    "pr-publish",
                    "demo",
                    "loop-decide",
                ],
                &[6],
                2,
            ),
            ("deploy", &["gate", "pr land"], &[], 0),
            ("ship", &["gate", "pr land -c"], &[], 0),
            ("ship-demo", &["gate", "demo", "pr land -c"], &[1], 0),
            ("vsm-operate", &["s1", "s2", "s3", "s4", "s5"], &[], 0),
        ];
        for (name, labels, humans, returns) in cases {
            let flow = load_flow(name, repo.path()).unwrap();
            let graph = FlowGraph::new(*name, &expand_flow(&flow, repo.path()).unwrap());
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
        let graph = FlowGraph::new(&flow.name, &expand_flow(&flow, repo.path()).unwrap());
        let labels: Vec<_> = graph.steps.iter().map(|node| node.label.as_str()).collect();
        assert_eq!(labels, ["rebase", "realign"]);
        assert_eq!(graph.steps[0].kind, FlowNodeKind::Op);
        assert_eq!(graph.steps[1].kind, FlowNodeKind::Skill);
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
                "rebase",
                "realign",
                "loop-decide",
                "pr-publish",
                "demo",
                "loop-decide",
                "compress",
                "rebase",
                "realign",
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
        assert!(graph.steps[1].human && graph.steps[8].human);
        assert_eq!(graph.steps[14].kind, FlowNodeKind::Op);
        assert_eq!(graph.steps[5].parents, ["feature", "pursue", "refresh"]);
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
