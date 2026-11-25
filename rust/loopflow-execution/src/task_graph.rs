//! A Task runs its captured workers and reviews their results at Graph breakpoints.

mod legacy;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::LoadError;
use crate::graph::runner::{ExecutionCursor, NestedCursor};
use crate::graph::{compile_graph, load_authored_graph, ConcreteSkill, ConcreteStep};
use loopflow_definitions::text::first_prose_line;
use loopflow_storage::sqlite::{Auto, AutoState};

pub use loopflow_storage::sqlite::task_work::{END, START};

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
        ConcreteStep::Breakpoint(stop) => (&stop.sources, stop.name.as_str(), None),
        ConcreteStep::Command(command) => (&command.sources, command.item.command.as_str(), None),
        ConcreteStep::Xor(branch) => (&branch.sources, branch.router.name.as_str(), None),
    };
    let interactive = match step {
        ConcreteStep::Skill(skill) => skill.interactive_name.as_deref(),
        _ => None,
    };
    interactive
        .into_iter()
        .chain(id)
        .chain(std::iter::once(own))
        .chain(sources.get(1).map(String::as_str))
        .collect()
}

/// The name a Task waits at before a step that merges: `lf land` or `lf pr
/// arm` however the graph spells it, or the `pr-land` skill.
pub fn landing_name(step: &ConcreteStep, merges: fn(&[String]) -> bool) -> Option<String> {
    match step {
        ConcreteStep::Command(command) => {
            merges(&command.item.argv()).then(|| command.item.display_name())
        }
        ConcreteStep::Skill(skill) if skill.skill.name == "pr-land" => {
            Some(names(step)[0].to_owned())
        }
        _ => None,
    }
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
            for (name, alternative) in &branch.paths {
                let mut path = path.to_vec();
                path.push((index, name.clone()));
                walk(&alternative.steps, &path, visit);
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
        interactive_steps(&self.steps)
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
            // A Graph records an operation under its display name.
            let recorded = matches!(step, ConcreteStep::Command(command) if command.item.display_name() == name);
            if found.is_none() && (recorded || names(step).contains(&name)) {
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

    /// The step that merges named `step`, where a Task someone turned auto off waits.
    pub fn landing_step(&self, step: &str, merges: fn(&[String]) -> bool) -> Option<Place> {
        let mut found = None;
        walk(&self.steps, &[], &mut |place, concrete| {
            if found.is_none() && landing_name(concrete, merges).as_deref() == Some(step) {
                found = Some(place);
            }
        });
        found
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
    pub fn next_breakpoint(&self, place: &Place) -> Option<&str> {
        fn scan(body: &[ConcreteStep], from: usize) -> Option<&str> {
            body[from..].iter().find_map(|step| match step {
                ConcreteStep::Breakpoint(stop) => Some(stop.name.as_str()),
                ConcreteStep::Xor(branch) => {
                    branch.paths.values().find_map(|path| scan(&path.steps, 0))
                }
                // A live released carrier still stops before its interactive worker.
                ConcreteStep::Skill(skill) => skill.interactive_step(),
                ConcreteStep::Command(_) => None,
            })
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

    /// What a person calls the step at `place`; an operation by the display
    /// name its Graph records it under.
    pub fn step_name(&self, place: &Place) -> String {
        step_name(self.step_at(place))
    }

    /// What a person calls the step `cursor` stands at, if the graph has it.
    pub fn step_name_of(&self, cursor: &ExecutionCursor) -> Option<String> {
        let (body, leaf) = cursor.current_body(&self.steps);
        body.get(leaf.index).map(step_name)
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
}

/// The interactive steps of `steps` in order, each XOR path's in name order.
fn interactive_steps(steps: &[ConcreteStep]) -> Vec<(Place, &ConcreteSkill)> {
    let mut out = Vec::new();
    walk(steps, &[], &mut |place, step| {
        if let ConcreteStep::Skill(skill) = step {
            if skill.interactive {
                out.push((place, skill));
            }
        }
    });
    out
}

fn interactive_summary(steps: &[ConcreteStep]) -> Vec<InteractiveStep> {
    interactive_steps(steps)
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

/// The top-level steps as a person reads them, consecutive repeats collapsed,
/// with the index in them of each top-level place. A composed step reads as
/// the graph it came from, and a step is interactive when a person takes part
/// anywhere inside it.
fn workflow(steps: &[ConcreteStep]) -> (Vec<WorkflowStep>, Vec<u32>) {
    let mut line: Vec<WorkflowStep> = Vec::new();
    let mut indices = Vec::with_capacity(steps.len());
    for step in steps {
        let sources = match step {
            ConcreteStep::Skill(skill) => &skill.sources,
            ConcreteStep::Breakpoint(stop) => &stop.sources,
            ConcreteStep::Command(command) => &command.sources,
            ConcreteStep::Xor(branch) => &branch.sources,
        };
        let name = sources.get(1).cloned().unwrap_or_else(|| step_name(step));
        let mut interactive = false;
        walk(std::slice::from_ref(step), &[], &mut |_, node| {
            interactive |= matches!(node, ConcreteStep::Breakpoint(_))
                || matches!(node, ConcreteStep::Skill(skill) if skill.interactive);
        });
        match line.last_mut() {
            Some(last) if last.name == name => last.interactive |= interactive,
            _ => line.push(WorkflowStep { name, interactive }),
        }
        indices.push(u32::try_from(line.len() - 1).expect("a graph has few steps"));
    }
    (line, indices)
}

/// What a person calls `step`; an operation by the display name its Graph
/// records it under.
pub fn step_name(step: &ConcreteStep) -> String {
    match step {
        ConcreteStep::Command(command) => command.item.display_name(),
        step => names(step)[0].to_owned(),
    }
}

/// A Graph as the workflow line shows it before any Task stands on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowOutline {
    /// The Graph's top-level steps in order, as `TaskGraph.steps` carries them.
    pub steps: Vec<WorkflowStep>,
    /// Every step where a person takes part, as `TaskGraph` carries them.
    pub interactive_steps: Vec<InteractiveStep>,
    /// "You take part at explore, ship-review and verify." or "Runs on its own."
    pub sentence: String,
}

impl WorkflowOutline {
    pub fn of(steps: &[ConcreteStep]) -> Self {
        let interactive_steps = interactive_summary(steps);
        let mut names: Vec<&str> = Vec::new();
        walk(steps, &[], &mut |_, step| {
            let name = match step {
                ConcreteStep::Breakpoint(stop) => Some(stop.name.as_str()),
                ConcreteStep::Skill(skill) => skill.interactive_step(),
                _ => None,
            };
            if let Some(name) = name.filter(|name| !names.contains(name)) {
                names.push(name);
            }
        });
        let sentence = match names.as_slice() {
            [] => "Runs on its own.".to_owned(),
            [only] => format!("You take part at {only}."),
            [rest @ .., last] => format!("You take part at {} and {last}.", rest.join(", ")),
        };
        Self {
            steps: workflow(steps).0,
            interactive_steps,
            sentence,
        }
    }
}

/// A step where a person takes part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InteractiveStep {
    pub name: String,
    pub skill: String,
    /// One line saying what the person does here: the skill's first line of prose.
    pub description: Option<String>,
}

/// One Task's Graph as `lf task status --json` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskGraph {
    pub name: String,
    pub interactive_steps: Vec<InteractiveStep>,
    pub position: TaskGraphPosition,
    /// The Graph's top-level steps in order, as the workflow line shows them.
    pub steps: Vec<WorkflowStep>,
    /// The index in `steps` of the step the Task stands in; null at `start`
    /// and at `end`.
    #[serde(deserialize_with = "Option::deserialize")]
    pub current_step: Option<u32>,
    /// Where the Task stands, in one sentence: "build is running implement.
    /// You are next needed at after-ship-review."
    pub sentence: String,
}

/// One top-level step of a Task's Graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowStep {
    pub name: String,
    /// A person takes part somewhere inside it, XOR paths included.
    pub interactive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskGraphPosition {
    /// The Task stands at `start`, at `end`, or at this interactive step,
    /// waiting on a person in the Task conversation.
    At { step: String },
    Breakpoint {
        step: String,
        arrival: loopflow_model::id::LfProcessId,
    },
    /// `lf task run` (`lf_process_id`) carries the Task on from `step`, and
    /// runs `at`. `running` is read from that Process: a run that ended
    /// without arriving stopped, and the next `lf task run` starts it again
    /// at `at`; the steps before it stay done.
    Run {
        step: String,
        at: String,
        lf_process_id: String,
        running: bool,
    },
}

impl TaskGraphPosition {
    /// Where the Task waits, or the step its run left.
    pub fn step(&self) -> &str {
        match self {
            Self::Breakpoint { step, .. } | Self::At { step } | Self::Run { step, .. } => step,
        }
    }
}

/// A Task's run as its Graph row holds it.
#[derive(Debug, Clone)]
pub struct TaskGraphRun<'a> {
    pub cursor: &'a ExecutionCursor,
    pub lf_process_id: String,
    pub running: bool,
}

impl TaskGraph {
    /// Where the Task stands on `graph`: waiting at `step`, or carried on from
    /// it by `run`. `auto` on names auto, not a person, as the next to take part.
    pub fn read(
        graph: &CapturedGraph,
        step: String,
        breakpoint: Option<&crate::store::task_work::BreakpointArrival>,
        run: Option<TaskGraphRun<'_>>,
        auto: Option<AutoState>,
    ) -> Self {
        let (steps, indices) = workflow(&graph.steps);
        let top = |place: &Place| {
            let index = place.path.first().map_or(place.index, |(index, _)| *index);
            indices[index]
        };
        let (position, current_step, sentence) = match (breakpoint, run) {
            (Some(stop), None) => {
                let place = graph.place_of(&stop.cursor);
                let mut sentence = format!("At breakpoint {step}.");
                if let Some(verdict) = &stop.cursor.leaf().progress.verdict {
                    if verdict.decision == crate::graph::transitions::GraphDecision::Blocked {
                        sentence.push_str(&format!(" Blocked: {}", verdict.summary));
                    }
                }
                (
                    TaskGraphPosition::Breakpoint {
                        step,
                        arrival: stop.id.clone(),
                    },
                    place.as_ref().map(top),
                    sentence,
                )
            }
            (None, None) => {
                let (place, sentence) = match step.as_str() {
                    START => (None, "Not started.".to_owned()),
                    END => (None, "At its end.".to_owned()),
                    step => match graph.interactive_step(step) {
                        Some(place) => (Some(place), format!("Waiting for you at {step}.")),
                        None => (graph.find(step), format!("Waiting at {step}.")),
                    },
                };
                (
                    TaskGraphPosition::At { step },
                    place.as_ref().map(top),
                    sentence,
                )
            }
            (_, Some(run)) => {
                let place = graph.place_of(run.cursor);
                let at = place
                    .as_ref()
                    .map_or_else(|| step.clone(), |place| graph.step_name(place));
                let current = place.as_ref().map(top);
                let label = current.map_or(at.as_str(), |index| &steps[index as usize].name);
                let sentence = if !run.running {
                    format!("Stopped at {label}. `lf task run` starts it again.")
                } else {
                    let running = if label == at {
                        format!("{label} is running.")
                    } else {
                        format!("{label} is running {at}.")
                    };
                    let next = place
                        .as_ref()
                        .and_then(|place| graph.after(place))
                        .and_then(|place| graph.next_breakpoint(&place));
                    match (next, auto) {
                        (None, _) => running,
                        (Some(next), Some(AutoState::On)) => {
                            format!("{running} Auto stands in next at {next}.")
                        }
                        (Some(next), _) => format!("{running} You are next needed at {next}."),
                    }
                };
                (
                    TaskGraphPosition::Run {
                        step,
                        at,
                        lf_process_id: run.lf_process_id,
                        running: run.running,
                    },
                    current,
                    sentence,
                )
            }
        };
        Self {
            name: graph.name.clone(),
            interactive_steps: interactive_summary(&graph.steps),
            position,
            steps,
            current_step,
            sentence,
        }
    }

    /// Where the Task waits, or the step its run left.
    pub fn step(&self) -> &str {
        self.position.step()
    }

    /// The interactive step a person takes part at now, such as `explore`.
    pub fn waiting_step(&self) -> Option<&InteractiveStep> {
        match &self.position {
            TaskGraphPosition::At { step } => {
                self.interactive_steps.iter().find(|s| &s.name == step)
            }
            TaskGraphPosition::Breakpoint { .. } | TaskGraphPosition::Run { .. } => None,
        }
    }

    /// Why the Task waits here and what lands it, when it stands at a step
    /// that is neither `start`, `end` nor interactive: it stopped before
    /// merging, which only an off does.
    pub fn waiting_to_land(&self, auto: Option<&Auto>, issue: &str) -> Option<String> {
        let off = auto.filter(|mark| mark.state == AutoState::Off)?;
        let TaskGraphPosition::At { step } = &self.position else {
            return None;
        };
        let ends = step == START || step == END;
        let interactive = self.interactive_steps.iter().any(|s| &s.name == step);
        (!ends && !interactive).then(|| crate::ops::task::auto::landing_wait(step, off, issue))
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
    use super::{CapturedGraph, TaskGraph, TaskGraphRun};
    use loopflow_storage::sqlite::{Auto, AutoState};

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
        let mut steps = Vec::new();
        super::walk(&feature.steps, &[], &mut |_, step| {
            if let crate::graph::ConcreteStep::Breakpoint(stop) = step {
                steps.push(stop.name.as_str());
            }
        });
        assert_eq!(
            steps,
            ["after-explore", "after-ship-review", "after-verify"]
        );
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
        let explore = graph.find("after-explore").unwrap();
        let next = graph.after(&explore).unwrap();
        assert_eq!(graph.next_breakpoint(&next), Some("after-ship-review"));
        // A composed graph's name finds its first step.
        let plan = graph.find("plan").unwrap();
        assert_eq!(plan.cursor().index, next.cursor().index);
        assert_eq!(
            graph.find("kickoff").unwrap().cursor().index,
            next.cursor().index
        );
        assert_eq!(
            graph.find("write-docs").unwrap().cursor().index,
            next.cursor().index + 2
        );
        assert_eq!(graph.find("start").unwrap().cursor().index, 0);
        assert!(graph.find("no-such-step").is_none());
        // After the last interactive step nothing remains.
        let verify = graph.find("after-verify").unwrap();
        assert!(graph
            .after(&verify)
            .is_none_or(|place| graph.next_breakpoint(&place).is_none()));
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
        assert_eq!(graph.next_breakpoint(&start), Some("after-demo"));
        let demo = graph.find("after-demo").unwrap();
        let after = graph.after(&demo).unwrap();
        assert_eq!(after.cursor().index, 2);
        assert_eq!(graph.next_breakpoint(&after), None);
    }

    #[test]
    fn only_an_off_makes_a_merging_step_a_landing_wait() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let at = |step: &str| TaskGraph::read(&graph, step.into(), None, None, None);
        let mark = |state| Auto {
            state,
            at: 0,
            by: crate::process::Starter::Person {
                name: Some("Maya".into()),
            },
        };
        let off = mark(AutoState::Off);
        assert_eq!(
            at("land").waiting_to_land(Some(&off), "INF-1").as_deref(),
            Some("Waiting at land: auto is off (Maya). `lf task auto on INF-1` lands it.")
        );
        assert_eq!(at("land").waiting_to_land(None, "INF-1"), None);
        assert_eq!(
            at("land").waiting_to_land(Some(&mark(AutoState::On)), "INF-1"),
            None
        );

        assert_eq!(at("end").waiting_to_land(Some(&off), "INF-1"), None);
    }

    #[test]
    fn the_workflow_line_names_top_level_steps_and_where_the_task_stands() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        let read = |step: &str, run: Option<(&str, bool)>, auto| {
            let cursor = run.map(|(at, _)| graph.find(at).unwrap().cursor());
            TaskGraph::read(
                &graph,
                step.into(),
                None,
                cursor
                    .as_ref()
                    .zip(run)
                    .map(|(cursor, (_, running))| TaskGraphRun {
                        cursor,
                        lf_process_id: "lfp_1".into(),
                        running,
                    }),
                auto,
            )
        };
        let start = read("start", None, None);
        let steps: Vec<_> = start
            .steps
            .iter()
            .map(|step| (step.name.as_str(), step.interactive))
            .collect();
        assert_eq!(
            steps,
            [
                ("plan", false),
                ("build", false),
                ("ship-review", false),
                ("after-ship-review", true),
                ("deliver", false),
                ("verify", false),
                ("after-verify", true)
            ]
        );
        assert_eq!(
            (start.current_step, start.sentence.as_str()),
            (None, "Not started.")
        );
        let end = read("end", None, None);
        assert_eq!(
            (end.current_step, end.sentence.as_str()),
            (None, "At its end.")
        );

        let implement = read("plan", Some(("implement", true)), None);
        assert_eq!(implement.current_step, Some(1));
        assert_eq!(
            implement.sentence,
            "build is running implement. You are next needed at after-ship-review."
        );
        let auto = read("plan", Some(("implement", true)), Some(AutoState::On));
        assert_eq!(
            auto.sentence,
            "build is running implement. Auto stands in next at after-ship-review."
        );
        let stopped = read("plan", Some(("implement", false)), None);
        assert_eq!(
            stopped.sentence,
            "Stopped at build. `lf task run` starts it again."
        );
        let review = read("ship-review", None, None);
        assert_eq!(review.current_step, Some(2));
        assert_eq!(review.sentence, "Waiting at ship-review.");
    }

    #[test]
    fn a_step_two_parts_share_is_placed_by_the_cursor_not_its_name() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        let mut seen = Vec::new();
        super::walk(&graph.steps, &[], &mut |place, _| {
            if graph.step_name(&place) == "concept-review" {
                seen.push(place.cursor());
            }
        });
        let labels: Vec<_> = seen
            .iter()
            .map(|cursor| {
                let line = TaskGraph::read(
                    &graph,
                    "plan".into(),
                    None,
                    Some(TaskGraphRun {
                        cursor,
                        lf_process_id: "lfp_1".into(),
                        running: true,
                    }),
                    None,
                );
                line.steps[line.current_step.unwrap() as usize].name.clone()
            })
            .collect();
        assert_eq!(labels, ["plan", "build"]);
    }
}
