//! A Task's Graph: the graph it took up, compiled then and fixed from then on,
//! and where the Task stands on it. A step written `interactive: <skill>` is
//! where a person takes part. `lf graph` runs that step in place; `lf task run`
//! stops before it, and the Task waits there for the Task conversation. The
//! next `lf task run` continues after it, or `--from <step>` starts anywhere.
//! Other Graphs run in the Task's checkout never move it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::LoadError;
use crate::graph::runner::{ExecutionCursor, NestedCursor};
use crate::graph::{compile_graph, load_authored_graph, ConcreteSkill, ConcreteStep};
use crate::skills::first_prose_line;

pub const START: &str = "start";
pub const END: &str = "end";

/// The graph a Task took up, compiled as it was then.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedGraph {
    pub name: String,
    pub steps: Vec<ConcreteStep>,
}

/// A place in the captured steps: the top level, or inside XOR paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Place {
    /// Each XOR step descended through, with the path taken.
    path: Vec<(usize, String)>,
    index: usize,
}

impl Place {
    pub fn cursor(&self) -> ExecutionCursor {
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

/// Every name a step answers to for `--from`: its id, its skill or command,
/// and the graph it was composed from.
fn names(step: &ConcreteStep) -> Vec<&str> {
    let (sources, own, id) = match step {
        ConcreteStep::Skill(skill) => (
            &skill.sources,
            skill.skill.name.as_str(),
            skill.id.as_deref(),
        ),
        ConcreteStep::Command(command) => (&command.sources, command.item.command.as_str(), None),
        ConcreteStep::Xor(branch) => (&branch.sources, branch.router.name.as_str(), None),
    };
    id.into_iter()
        .chain(std::iter::once(own))
        .chain(sources.get(1).map(String::as_str))
        .collect()
}

/// Visit every place in order, descending into XOR paths by name.
fn walk<'a>(
    steps: &'a [ConcreteStep],
    path: &[(usize, String)],
    visit: &mut impl FnMut(Place, &'a ConcreteStep),
) {
    for (index, step) in steps.iter().enumerate() {
        visit(
            Place {
                path: path.to_vec(),
                index,
            },
            step,
        );
        if let ConcreteStep::Xor(branch) = step {
            let mut names: Vec<_> = branch.paths.keys().collect();
            names.sort();
            for name in names {
                let mut path = path.to_vec();
                path.push((index, name.clone()));
                walk(&branch.paths[name].steps, &path, visit);
            }
        }
    }
}

impl CapturedGraph {
    /// Compile the graph named `name` as the repository defines it now.
    pub fn load(name: &str, repo: &Path) -> Result<Option<Self>, LoadError> {
        let graph = match load_authored_graph(name, repo) {
            Ok(graph) => graph,
            Err(LoadError::GraphNotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let steps = compile_graph(&graph, repo)?;
        Ok(Some(Self {
            name: graph.name,
            steps,
        }))
    }

    /// Its interactive steps in order, each XOR path's in name order.
    pub fn interactive_steps(&self) -> Vec<(Place, &ConcreteSkill)> {
        let mut out = Vec::new();
        walk(&self.steps, &[], &mut |place, step| {
            if let ConcreteStep::Skill(skill) = step {
                if skill.interactive {
                    out.push((place, skill));
                }
            }
        });
        out
    }

    /// The first step answering to `name`; `start` is the first step.
    pub fn find(&self, name: &str) -> Option<Place> {
        if name == START {
            return (!self.steps.is_empty()).then(|| Place {
                path: Vec::new(),
                index: 0,
            });
        }
        let mut found = None;
        walk(&self.steps, &[], &mut |place, step| {
            if found.is_none() && names(step).contains(&name) {
                found = Some(place);
            }
        });
        found
    }

    /// The interactive step named `step`.
    pub fn interactive_step(&self, step: &str) -> Option<Place> {
        self.interactive_steps()
            .into_iter()
            .find(|(_, skill)| skill.interactive_step() == Some(step))
            .map(|(place, _)| place)
    }

    /// The place after `place` in its body, or in the body that XOR belongs
    /// to; `None` after the last step.
    pub fn after(&self, place: &Place) -> Option<Place> {
        let mut path = place.path.clone();
        let mut index = place.index + 1;
        loop {
            if index < body(&self.steps, &path).len() {
                return Some(Place { path, index });
            }
            let (xor, _) = path.pop()?;
            index = xor + 1;
        }
    }

    /// The first interactive step a run from `place` reaches, looking into
    /// every XOR path in name order; `None` when it runs to the end.
    pub fn next_interactive(&self, place: &Place) -> Option<&str> {
        fn scan(body: &[ConcreteStep], from: usize) -> Option<&str> {
            for step in &body[from..] {
                match step {
                    ConcreteStep::Skill(skill) => {
                        if let Some(step) = skill.interactive_step() {
                            return Some(step);
                        }
                    }
                    ConcreteStep::Xor(branch) => {
                        let mut names: Vec<_> = branch.paths.keys().collect();
                        names.sort();
                        if let Some(step) = names
                            .into_iter()
                            .find_map(|name| scan(&branch.paths[name].steps, 0))
                        {
                            return Some(step);
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
            if let Some(step) = scan(body(&self.steps, &path), from) {
                return Some(step);
            }
            let (index, _) = path.pop()?;
            from = index + 1;
        }
    }

    /// The place `cursor` stands at, if the graph has it.
    pub fn place_of(&self, cursor: &ExecutionCursor) -> Option<Place> {
        let mut path = Vec::new();
        let mut cursor = cursor;
        while let Some(NestedCursor::Xor {
            selected,
            cursor: child,
        }) = cursor.child.as_deref()
        {
            path.push((cursor.index, selected.clone()));
            cursor = child;
        }
        let place = Place {
            path,
            index: cursor.index,
        };
        let mut body: &[ConcreteStep] = &self.steps;
        for (index, selected) in &place.path {
            match body.get(*index) {
                Some(ConcreteStep::Xor(branch)) => body = &branch.paths.get(selected)?.steps,
                _ => return None,
            }
        }
        (place.index < body.len()).then_some(place)
    }

    fn step_at(&self, place: &Place) -> &ConcreteStep {
        &body(&self.steps, &place.path)[place.index]
    }

    /// The interactive step at `place`, if it is one.
    pub fn interactive_at(&self, place: &Place) -> Option<&str> {
        match self.step_at(place) {
            ConcreteStep::Skill(skill) => skill.interactive_step(),
            _ => None,
        }
    }

    /// What a person calls the step at `place`.
    pub fn step_name(&self, place: &Place) -> &str {
        names(self.step_at(place))[0]
    }

    /// Steps a person would name for `--from`, for messages.
    pub fn describe_steps(&self) -> String {
        let mut seen: Vec<&str> = Vec::new();
        walk(&self.steps, &[], &mut |_, step| {
            let name = names(step)[0];
            if !seen.contains(&name) {
                seen.push(name);
            }
        });
        if seen.is_empty() {
            "none".into()
        } else {
            seen.join(", ")
        }
    }

    pub fn interactive_summary(&self) -> Vec<InteractiveStep> {
        self.interactive_steps()
            .into_iter()
            .map(|(_, skill)| InteractiveStep {
                name: skill
                    .interactive_step()
                    .expect("interactive steps have a name")
                    .into(),
                skill: skill.skill.name.clone(),
                description: skill.skill.content.as_deref().and_then(first_prose_line),
            })
            .collect()
    }
}

/// A step where a person takes part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveStep {
    pub name: String,
    pub skill: String,
    /// One line saying what the person does here: the skill's first line of prose.
    pub description: Option<String>,
}

/// One Task's Graph as `lf task status --json` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraph {
    pub name: String,
    pub interactive_steps: Vec<InteractiveStep>,
    pub position: TaskGraphPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskGraphPosition {
    /// The Task stands at `start`, at `end`, or at this interactive step,
    /// waiting on a person in the Task conversation.
    At { step: String },
    /// `lf task run` (`lf_process_id`) carries the Task on from `step`.
    /// `running` is read from that Process: a run that ended without
    /// arriving stopped, and the next `lf task run` starts it again.
    Run {
        step: String,
        lf_process_id: String,
        running: bool,
    },
}

impl TaskGraph {
    /// Where the Task waits, or the step its run left.
    pub fn step(&self) -> &str {
        match &self.position {
            TaskGraphPosition::At { step } | TaskGraphPosition::Run { step, .. } => step,
        }
    }

    /// The interactive step a person takes part at now, such as `explore`.
    pub fn waiting_step(&self) -> Option<&InteractiveStep> {
        match &self.position {
            TaskGraphPosition::At { step } => {
                self.interactive_steps.iter().find(|s| &s.name == step)
            }
            TaskGraphPosition::Run { .. } => None,
        }
    }

    /// One line for text status.
    pub fn summary(&self) -> String {
        let name = &self.name;
        match &self.position {
            TaskGraphPosition::At { step } => format!("Graph {name}: at {step}"),
            TaskGraphPosition::Run { step, running, .. } => format!(
                "Graph {name}: {} from {step}",
                if *running { "running" } else { "stopped" }
            ),
        }
    }

    /// What the Task conversation needs: the step it works and how to go on.
    pub fn guidance(&self, issue: &str) -> String {
        let summary = self.summary();
        let mut lines = vec![match (&self.position, self.waiting_step()) {
            (TaskGraphPosition::Run { running: true, .. }, _) => {
                return format!("{summary}. A person takes part again when that run stops.");
            }
            (TaskGraphPosition::Run { .. }, _) => format!(
                "{summary}. `lf task run {issue}` starts that run again; read its log in `lf task status {issue}` first."
            ),
            (_, Some(step)) => format!(
                "{summary}. The Task conversation works this step with the {} skill; when the person is done, `lf task run {issue}` continues after it.",
                step.skill
            ),
            (TaskGraphPosition::At { step }, None) if step == END => {
                format!("{summary}.")
            }
            (TaskGraphPosition::At { .. }, None) => {
                format!("{summary}. `lf task run {issue}` starts it.")
            }
        }];
        lines.push(format!(
            "`lf task run {issue} --from <step>` starts anywhere, to go back or skip ahead; `lf -b task run {issue}` runs straight through, interactive steps headless; `lf graph <graph>` in the Task worktree runs a Graph without moving the Task."
        ));
        lines.join("\n")
    }
}

/// Name the committed file a teammate's graph selection needs.
pub fn missing_graph(name: &str) -> String {
    format!(
        "graph {name:?} is unavailable: add .lf/graphs/{name}.yaml \
         (a teammate's graph arrives when its commit is merged or pulled)"
    )
}

pub(crate) fn validate_graph_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.chars().any(char::is_whitespace) || name.contains([':', '\\']) {
        return Err("invalid graph name".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CapturedGraph, TaskGraph, TaskGraphPosition};

    fn write(repo: &std::path::Path, path: &str, content: &str) {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn builtin_graphs_name_their_interactive_steps() {
        let repo = tempfile::tempdir().unwrap();
        let feature = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let steps: Vec<_> = feature
            .interactive_summary()
            .into_iter()
            .map(|step| step.name)
            .collect();
        assert_eq!(steps, ["explore", "demo", "verify"]);
        let pursue = CapturedGraph::load("pursue", repo.path()).unwrap().unwrap();
        assert!(pursue.interactive_steps().is_empty());
        assert!(CapturedGraph::load("no-such-graph", repo.path())
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_run_continues_after_the_step_it_stopped_at_or_from_any_named_step() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let explore = graph.interactive_step("explore").unwrap();
        let next = graph.after(&explore).unwrap();
        assert_eq!(graph.next_interactive(&next), Some("demo"));
        // A composed graph's name finds its first step.
        let pursue = graph.find("pursue").unwrap();
        assert_eq!(pursue.cursor().index, next.cursor().index);
        assert_eq!(graph.find("start").unwrap().cursor().index, 0);
        assert!(graph.find("no-such-step").is_none());
        // After the last interactive step nothing remains.
        let verify = graph.interactive_step("verify").unwrap();
        assert!(graph
            .after(&verify)
            .is_none_or(|place| graph.next_interactive(&place).is_none()));
    }

    #[test]
    fn an_interactive_step_inside_an_xor_path_is_found_there() {
        let repo = tempfile::tempdir().unwrap();
        write(
            repo.path(),
            ".lf/graphs/conditional.yaml",
            "- implement\n- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - interactive: demo\n      skip:\n        description: Nothing to review\n- gate\n",
        );
        let graph = CapturedGraph::load("conditional", repo.path())
            .unwrap()
            .unwrap();
        let start = graph.find("start").unwrap();
        assert_eq!(graph.next_interactive(&start), Some("demo"));
        let demo = graph.interactive_step("demo").unwrap();
        let after = graph.after(&demo).unwrap();
        assert_eq!(after.cursor().index, 2);
        assert_eq!(graph.next_interactive(&after), None);
    }

    #[test]
    fn guidance_names_the_waiting_step_and_how_to_go_on() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let at = |position| TaskGraph {
            name: graph.name.clone(),
            interactive_steps: graph.interactive_summary(),
            position,
        };
        let waiting = at(TaskGraphPosition::At {
            step: "demo".into(),
        });
        let guidance = waiting.guidance("INF-1");
        assert!(guidance.contains("with the demo skill"), "{guidance}");
        assert!(guidance.contains("`lf task run INF-1` continues after it"));
        assert!(guidance.contains("--from <step>"));
        let running = at(TaskGraphPosition::Run {
            step: "explore".into(),
            lf_process_id: "process".into(),
            running: true,
        });
        assert!(!running.guidance("INF-1").contains("task run"));
    }
}
