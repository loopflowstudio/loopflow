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
use loopflow_definitions::text::first_prose_line;
use loopflow_storage::sqlite::task_work::PauseStage;
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
pub fn landing_name(
    step: &ConcreteStep,
    mut merges: impl FnMut(&[String]) -> bool,
) -> Option<String> {
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
                        if let Some(step) =
                            branch.paths.values().find_map(|path| scan(&path.steps, 0))
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

    fn has_landing(steps: &[ConcreteStep], merges: &mut dyn FnMut(&[String]) -> bool) -> bool {
        steps.iter().any(|step| match step {
            ConcreteStep::Xor(branch) => branch
                .paths
                .values()
                .any(|path| Self::has_landing(&path.steps, merges)),
            _ => landing_name(step, &mut *merges).is_some(),
        })
    }

    /// Could any remaining path need a person before or at its next landing?
    fn needs_review(
        &self,
        place: &Place,
        stop_at_landing: bool,
        merges: &mut dyn FnMut(&[String]) -> bool,
    ) -> bool {
        fn scan(
            steps: &[ConcreteStep],
            after: bool,
            stop_at_landing: bool,
            merges: &mut dyn FnMut(&[String]) -> bool,
        ) -> bool {
            steps.iter().rev().fold(after, |later, step| match step {
                ConcreteStep::Skill(skill) if skill.interactive_step().is_some() => true,
                ConcreteStep::Xor(branch) => branch
                    .paths
                    .values()
                    .any(|path| scan(&path.steps, later, stop_at_landing, merges)),
                _ if landing_name(step, &mut *merges).is_some() => stop_at_landing,
                _ => later,
            })
        }
        let mut steps = self.steps.as_slice();
        let mut later = false;
        for (index, selected) in &place.path {
            later = scan(&steps[index + 1..], later, stop_at_landing, merges);
            let ConcreteStep::Xor(branch) = &steps[*index] else {
                unreachable!("validated XOR path")
            };
            steps = &branch.paths[selected].steps;
        }
        scan(&steps[place.index..], later, stop_at_landing, merges)
    }

    /// Whether no human step remains before landing from the current cursor or stop.
    pub fn ships_before_review(
        &self,
        step: &str,
        cursor: Option<&ExecutionCursor>,
        auto: Option<AutoState>,
        mut merges: impl FnMut(&[String]) -> bool,
    ) -> bool {
        if !Self::has_landing(&self.steps, &mut merges) {
            return false;
        }
        let place = if let Some(cursor) = cursor {
            if cursor.index == self.steps.len() {
                return true;
            }
            self.place_of(cursor)
        } else if step == END {
            return true;
        } else {
            self.interactive_step(step).or_else(|| self.find(step))
        };
        let stop_at_landing = auto == Some(AutoState::Off);
        place.is_some_and(|place| !self.needs_review(&place, stop_at_landing, &mut merges))
    }

    /// A run restarts at its saved cursor unless `auto` now parks at that step.
    /// A Task already waiting for a person stays there.
    pub(crate) fn resumes_on_unpause(
        &self,
        step: &str,
        cursor: Option<&ExecutionCursor>,
        auto: Option<AutoState>,
        merges: impl FnMut(&[String]) -> bool,
    ) -> bool {
        match cursor {
            Some(cursor) => self
                .place_of(cursor)
                .is_some_and(|place| self.stop_at(&place, auto, merges).is_none()),
            None if step == START => true,
            None => {
                step != END && auto != Some(AutoState::Off) && self.interactive_step(step).is_none()
            }
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

    /// The landing step at `place`, if it is one.
    pub fn landing_at(
        &self,
        place: &Place,
        merges: impl FnMut(&[String]) -> bool,
    ) -> Option<String> {
        landing_name(self.step_at(place), merges)
    }

    /// The natural stop `place` parks at under `auto`: an interactive step
    /// without auto on, or a landing step with auto off.
    pub fn stop_at(
        &self,
        place: &Place,
        auto: Option<AutoState>,
        merges: impl FnMut(&[String]) -> bool,
    ) -> Option<String> {
        if auto != Some(AutoState::On) {
            if let Some(step) = self.interactive_at(place) {
                return Some(step.to_owned());
            }
        }
        if auto == Some(AutoState::Off) {
            if let Some(step) = self.landing_at(place, merges) {
                return Some(step);
            }
        }
        None
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
            ConcreteStep::Command(command) => &command.sources,
            ConcreteStep::Xor(branch) => &branch.sources,
        };
        let name = sources.get(1).cloned().unwrap_or_else(|| step_name(step));
        let interactive = !interactive_steps(std::slice::from_ref(step)).is_empty();
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

/// The workflow track's primary action and current-step chip label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowControl {
    pub action: WorkflowAction,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAction {
    Run,
    Pause,
    Pausing,
    Unpause,
    Resume,
    Wait,
    Done,
}

fn chip_step(top: Option<&str>, at: &str) -> String {
    match top {
        Some(top) if top != at => format!("{top} · {at}"),
        _ => at.to_owned(),
    }
}

impl WorkflowControl {
    fn run(step: &str) -> Self {
        Self {
            action: WorkflowAction::Run,
            label: format!("Run: {step}"),
        }
    }

    fn paused(step: &str, resume: bool, stopping: bool) -> Self {
        let (action, prefix) = match (stopping, resume) {
            (true, _) => (WorkflowAction::Pausing, "Pausing"),
            (false, true) => (WorkflowAction::Resume, "Paused"),
            (false, false) => (WorkflowAction::Unpause, "Paused"),
        };
        Self {
            action,
            label: format!("{prefix}: {step}"),
        }
    }

    fn start(steps: &[WorkflowStep]) -> Self {
        Self::run(steps.first().map_or(END, |step| step.name.as_str()))
    }
}

/// A Graph as the workflow line shows it before any Task stands on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowOutline {
    pub start: WorkflowControl,
    /// The Graph's top-level steps in order, as `TaskGraph.steps` carries them.
    pub steps: Vec<WorkflowStep>,
    /// Every step where a person takes part, as `TaskGraph` carries them.
    pub interactive_steps: Vec<InteractiveStep>,
    /// "You take part at explore and ship-review." or "Runs on its own."
    pub sentence: String,
}

impl WorkflowOutline {
    pub fn of(steps: &[ConcreteStep]) -> Self {
        let interactive_steps = interactive_summary(steps);
        let mut names: Vec<&str> = Vec::new();
        for step in &interactive_steps {
            if !names.contains(&step.name.as_str()) {
                names.push(&step.name);
            }
        }
        let sentence = match names.as_slice() {
            [] => "Runs on its own.".to_owned(),
            [only] => format!("You take part at {only}."),
            [rest @ .., last] => format!("You take part at {} and {last}.", rest.join(", ")),
        };
        let steps = workflow(steps).0;
        Self {
            start: WorkflowControl::start(&steps),
            steps,
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
    pub control: WorkflowControl,
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
    /// You are next needed at ship-review."
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
            Self::At { step } | Self::Run { step, .. } => step,
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
        run: Option<TaskGraphRun<'_>>,
        auto: Option<AutoState>,
        pause: Option<PauseStage>,
        merges: impl FnMut(&[String]) -> bool,
    ) -> Self {
        let (steps, indices) = workflow(&graph.steps);
        let top = |place: &Place| {
            let index = place.path.first().map_or(place.index, |(index, _)| *index);
            indices[index]
        };
        let top_name = |place: &Place| steps[top(place) as usize].name.as_str();
        let resumes = pause.is_some()
            && graph.resumes_on_unpause(&step, run.as_ref().map(|run| run.cursor), auto, merges);
        let pausing = pause == Some(PauseStage::Stopping)
            || (pause.is_some() && run.as_ref().is_some_and(|run| run.running));
        let unpause = if resumes {
            "`lf task unpause --run` resumes it."
        } else {
            "`lf task unpause` lifts the hold."
        };
        let (position, current_step, sentence, control, chip) = match run {
            None => {
                let (place, sentence, control, chip) = match step.as_str() {
                    START => {
                        let first = steps.first().map_or(END, |step| step.name.as_str());
                        (
                            None,
                            "Not started.".to_owned(),
                            WorkflowControl::run(first),
                            first.to_owned(),
                        )
                    }
                    END => (
                        None,
                        "At its end.".to_owned(),
                        WorkflowControl {
                            action: WorkflowAction::Done,
                            label: "Done".into(),
                        },
                        END.to_owned(),
                    ),
                    step => match graph.interactive_step(step) {
                        Some(place) => {
                            let next = graph.after(&place).map(|next| {
                                if top(&place) == top(&next) {
                                    graph.step_name(&next)
                                } else {
                                    top_name(&next).to_owned()
                                }
                            });
                            let chip = chip_step(Some(top_name(&place)), step);
                            (
                                Some(place),
                                format!("Waiting for you at {step}."),
                                WorkflowControl {
                                    action: WorkflowAction::Run,
                                    label: format!(
                                        "{step} · next: {}",
                                        next.as_deref().unwrap_or(END)
                                    ),
                                },
                                chip,
                            )
                        }
                        None => {
                            let place = graph.find(step);
                            let chip = chip_step(place.as_ref().map(top_name), step);
                            let (sentence, control) = if auto == Some(AutoState::Off) {
                                (
                                    format!("Waiting at {step}: auto is off."),
                                    WorkflowControl {
                                        action: WorkflowAction::Wait,
                                        label: format!("Waiting: {chip}"),
                                    },
                                )
                            } else {
                                (format!("Waiting at {step}."), WorkflowControl::run(&chip))
                            };
                            (place, sentence, control, chip)
                        }
                    },
                };
                let sentence = match pause {
                    Some(PauseStage::Stopping) => format!("Pausing at {step}."),
                    Some(PauseStage::Held) => {
                        format!("Paused at {step}. {unpause}")
                    }
                    None => sentence,
                };
                (
                    TaskGraphPosition::At { step },
                    place.as_ref().map(top),
                    sentence,
                    control,
                    chip,
                )
            }
            Some(run) => {
                let place = graph.place_of(run.cursor);
                let at = place
                    .as_ref()
                    .map_or_else(|| step.clone(), |place| graph.step_name(place));
                let current = place.as_ref().map(top);
                let top_name = place.as_ref().map(top_name);
                let label = top_name.unwrap_or(at.as_str());
                let chip = chip_step(top_name, &at);
                let location = || {
                    if label == at {
                        format!("at {at}")
                    } else {
                        format!("in {label} at {at}")
                    }
                };
                let sentence = if pause.is_some() {
                    if pausing {
                        let verb = if run.running
                            && place.as_ref().is_some_and(|place| {
                                matches!(graph.step_at(place), ConcreteStep::Command(_))
                            }) {
                            "finishing"
                        } else {
                            "stopping"
                        };
                        format!("Pausing at {label} ({at} is {verb}).")
                    } else {
                        format!("Paused {}. {unpause}", location())
                    }
                } else if !run.running {
                    format!("Stopped {}. `lf task run` starts it again.", location())
                } else {
                    let running = if label == at {
                        format!("{label} is running.")
                    } else {
                        format!("{label} is running {at}.")
                    };
                    let next = place
                        .as_ref()
                        .and_then(|place| graph.after(place))
                        .and_then(|place| graph.next_interactive(&place));
                    match (next, auto) {
                        (None, _) => running,
                        (Some(next), Some(AutoState::On)) => {
                            format!("{running} Auto stands in next at {next}.")
                        }
                        (Some(next), _) => format!("{running} You are next needed at {next}."),
                    }
                };
                let control = if run.running {
                    WorkflowControl {
                        action: WorkflowAction::Pause,
                        label: chip.clone(),
                    }
                } else {
                    WorkflowControl::run(&chip)
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
                    control,
                    chip,
                )
            }
        };
        let control = if pause.is_some() {
            WorkflowControl::paused(&chip, resumes, pausing)
        } else {
            control
        };
        Self {
            control,
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
            TaskGraphPosition::Run { .. } => None,
        }
    }

    /// The merging step the Task waits at and the off that stopped it there,
    /// when it stands at a step that is neither `start`, `end` nor
    /// interactive: it stopped before merging, which only an off does.
    pub fn landing_stop<'a>(&'a self, auto: Option<&'a Auto>) -> Option<(&'a str, &'a Auto)> {
        let off = auto.filter(|mark| mark.state == AutoState::Off)?;
        let TaskGraphPosition::At { step } = &self.position else {
            return None;
        };
        let ends = step == START || step == END;
        let interactive = self.interactive_steps.iter().any(|s| &s.name == step);
        (!ends && !interactive).then_some((step.as_str(), off))
    }
}

/// Tasks in a list often share a Graph. Parse each command's argv once per reading.
pub(crate) fn cached_merges(merges: fn(&[String]) -> bool) -> impl FnMut(&[String]) -> bool {
    let mut commands = std::collections::HashMap::<Vec<String>, bool>::new();
    move |argv| {
        if let Some(&result) = commands.get(argv) {
            return result;
        }
        let result = merges(argv);
        commands.insert(argv.to_vec(), result);
        result
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
pub(crate) mod tests {
    pub(crate) fn test_merges(args: &[String]) -> bool {
        args.get(1).is_some_and(|arg| arg == "land")
    }

    use super::{interactive_summary, CapturedGraph, TaskGraph, TaskGraphRun};
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
        let steps: Vec<_> = interactive_summary(&feature.steps)
            .into_iter()
            .map(|step| step.name)
            .collect();
        assert_eq!(steps, ["explore", "ship-review"]);
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
        assert_eq!(graph.next_interactive(&next), Some("ship-review"));
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
        // Delivery and verification run after review.
        let review = graph.interactive_step("ship-review").unwrap();
        assert!(graph
            .after(&review)
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
    fn only_an_off_makes_a_merging_step_a_landing_wait() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let at = |step: &str| TaskGraph::read(&graph, step.into(), None, None, None, test_merges);
        let mark = |state| Auto {
            state,
            at: 0,
            by: crate::process::Starter::Person {
                name: Some("Maya".into()),
            },
        };
        let off = mark(AutoState::Off);
        let on = mark(AutoState::On);
        assert_eq!(at("land").landing_stop(Some(&off)), Some(("land", &off)));
        assert_eq!(at("land").landing_stop(None), None);
        assert_eq!(at("land").landing_stop(Some(&on)), None);
        assert_eq!(at("ship-review").landing_stop(Some(&off)), None);
        assert_eq!(at("end").landing_stop(Some(&off)), None);
    }

    #[test]
    fn workflow_control_resumes_interrupted_work_and_releases_human_stops() {
        use super::PauseStage;
        use super::WorkflowAction::{Done, Pause, Pausing, Resume, Run, Unpause, Wait};
        let repo = tempfile::tempdir().unwrap();
        write(repo.path(), ".lf/graphs/test.yaml",
            "- implement\n- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - interactive: demo\n      skip:\n        description: Skip it\n- gate\n- interactive: verify\n");
        let graph = CapturedGraph::load("test", repo.path()).unwrap().unwrap();
        for (step, run, held, action, label) in [
            ("start", None, false, Run, "Run: implement"),
            ("start", None, true, Resume, "Paused: implement"),
            ("start", Some(true), false, Pause, "implement"),
            ("start", Some(false), false, Run, "Run: implement"),
            ("start", Some(false), true, Resume, "Paused: implement"),
            ("demo", None, true, Unpause, "Paused: xor-route · demo"),
            ("demo", None, false, Run, "demo · next: gate"),
            ("verify", None, false, Run, "verify · next: end"),
            ("verify", None, true, Unpause, "Paused: verify"),
            ("end", None, true, Unpause, "Paused: end"),
            ("end", None, false, Done, "Done"),
        ] {
            let cursor = graph.find("implement").unwrap().cursor();
            let run = run.map(|running| TaskGraphRun {
                cursor: &cursor,
                lf_process_id: "worker".into(),
                running,
            });
            let control = TaskGraph::read(
                &graph,
                step.into(),
                run,
                None,
                held.then_some(PauseStage::Held),
                test_merges,
            )
            .control;
            assert_eq!((control.action, control.label.as_str()), (action, label));
        }

        // Composed top-level steps stay in every non-running chip label.
        let code = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        let implement = code.find("implement").unwrap().cursor();
        let verify = code.find("verify").unwrap().cursor();
        let land = code.find("land --wait-and-fix").unwrap().cursor();
        for (step, cursor, running, auto, pause, action, label) in [
            (
                "start",
                None,
                false,
                None,
                Some(PauseStage::Held),
                Resume,
                "Paused: plan",
            ),
            (
                "plan",
                Some(&implement),
                false,
                None,
                None,
                Run,
                "Run: build · implement",
            ),
            (
                "plan",
                Some(&implement),
                false,
                None,
                Some(PauseStage::Held),
                Resume,
                "Paused: build · implement",
            ),
            ("plan", Some(&verify), false, None, None, Run, "Run: verify"),
            (
                "plan",
                Some(&verify),
                true,
                None,
                Some(PauseStage::Stopping),
                Pausing,
                "Pausing: verify",
            ),
            (
                "plan",
                Some(&land),
                true,
                None,
                Some(PauseStage::Held),
                Pausing,
                "Pausing: deliver · land --wait-and-fix",
            ),
            (
                "land --wait-and-fix",
                None,
                false,
                Some(AutoState::Off),
                None,
                Wait,
                "Waiting: deliver · land --wait-and-fix",
            ),
            (
                "land --wait-and-fix",
                None,
                false,
                Some(AutoState::Off),
                Some(PauseStage::Held),
                Unpause,
                "Paused: deliver · land --wait-and-fix",
            ),
            (
                "ship-review",
                None,
                false,
                None,
                Some(PauseStage::Held),
                Unpause,
                "Paused: ship-review",
            ),
        ] {
            let run = cursor.map(|cursor| TaskGraphRun {
                cursor,
                lf_process_id: "worker".into(),
                running,
            });
            let control =
                TaskGraph::read(&code, step.into(), run, auto, pause, test_merges).control;
            assert_eq!((control.action, control.label.as_str()), (action, label));
        }
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
                cursor
                    .as_ref()
                    .zip(run)
                    .map(|(cursor, (_, running))| TaskGraphRun {
                        cursor,
                        lf_process_id: "lfp_1".into(),
                        running,
                    }),
                auto,
                None,
                test_merges,
            )
        };
        let start = read("start", None, None);
        assert_eq!(start.control.label, "Run: plan");
        assert_eq!(
            read("ship-review", None, None).control.label,
            "ship-review · next: deliver"
        );
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
                ("ship-review", true),
                ("deliver", false),
                ("verify", false)
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
            "build is running implement. You are next needed at ship-review."
        );
        let auto = read("plan", Some(("implement", true)), Some(AutoState::On));
        assert_eq!(
            auto.sentence,
            "build is running implement. Auto stands in next at ship-review."
        );
        let stopped = read("plan", Some(("implement", false)), None);
        assert_eq!(
            stopped.sentence,
            "Stopped in build at implement. `lf task run` starts it again."
        );
        let review = read("ship-review", None, None);
        assert_eq!(review.current_step, Some(2));
        assert_eq!(review.sentence, "Waiting for you at ship-review.");
    }

    #[test]
    fn a_paused_workflow_names_the_stopping_step_until_cleanup_finishes() {
        use super::WorkflowAction::{Pausing, Resume};
        use loopflow_storage::sqlite::task_work::PauseStage;
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        for (step, pause, running, sentence, action, label) in [
            (
                "implement",
                PauseStage::Stopping,
                true,
                "Pausing at build (implement is stopping).",
                Pausing,
                "Pausing: build · implement",
            ),
            (
                "implement",
                PauseStage::Stopping,
                false,
                "Pausing at build (implement is stopping).",
                Pausing,
                "Pausing: build · implement",
            ),
            (
                "implement",
                PauseStage::Held,
                false,
                "Paused in build at implement. `lf task unpause --run` resumes it.",
                Resume,
                "Paused: build · implement",
            ),
            (
                "land --wait-and-fix",
                PauseStage::Held,
                true,
                "Pausing at deliver (land --wait-and-fix is finishing).",
                Pausing,
                "Pausing: deliver · land --wait-and-fix",
            ),
        ] {
            let cursor = graph.find(step).unwrap().cursor();

            let run = TaskGraphRun {
                cursor: &cursor,
                lf_process_id: "carrier".into(),
                running,
            };
            let reading = TaskGraph::read(
                &graph,
                "start".into(),
                Some(run),
                None,
                Some(pause),
                test_merges,
            );
            assert_eq!(reading.sentence, sentence);
            assert_eq!(
                (reading.control.action, reading.control.label.as_str()),
                (action, label)
            );
        }
        assert_eq!(
            TaskGraph::read(
                &graph,
                "ship-review".into(),
                None,
                Some(AutoState::On),
                Some(PauseStage::Held),
                test_merges,
            )
            .sentence,
            "Paused at ship-review. `lf task unpause` lifts the hold."
        );
    }

    #[test]
    fn paused_stop_cursors_honour_auto_and_do_not_promise_resume() {
        use super::WorkflowAction::{Resume, Unpause};
        use loopflow_storage::sqlite::task_work::PauseStage;

        let repo = tempfile::tempdir().unwrap();
        let code = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        let review = code.find("ship-review").unwrap().cursor();
        let land = code.find("land --wait-and-fix").unwrap().cursor();
        let implement = code.find("implement").unwrap().cursor();
        let read = |cursor: &crate::graph::runner::ExecutionCursor, auto| {
            TaskGraph::read(
                &code,
                "start".into(),
                Some(TaskGraphRun {
                    cursor,
                    lf_process_id: "carrier".into(),
                    running: false,
                }),
                auto,
                Some(PauseStage::Held),
                test_merges,
            )
        };

        // Turning auto off while paused on ship-review or land switches Resume -> Unpause.
        for (cursor, auto, action, sentence) in [
            (
                &review,
                Some(AutoState::On),
                Resume,
                "Paused at ship-review. `lf task unpause --run` resumes it.",
            ),
            (
                &review,
                Some(AutoState::Off),
                Unpause,
                "Paused at ship-review. `lf task unpause` lifts the hold.",
            ),
            (
                &review,
                None,
                Unpause,
                "Paused at ship-review. `lf task unpause` lifts the hold.",
            ),
            (
                &land,
                Some(AutoState::On),
                Resume,
                "Paused in deliver at land --wait-and-fix. `lf task unpause --run` resumes it.",
            ),
            (
                &land,
                None,
                Resume,
                "Paused in deliver at land --wait-and-fix. `lf task unpause --run` resumes it.",
            ),
            (
                &land,
                Some(AutoState::Off),
                Unpause,
                "Paused in deliver at land --wait-and-fix. `lf task unpause` lifts the hold.",
            ),
            (
                &implement,
                Some(AutoState::Off),
                Resume,
                "Paused in build at implement. `lf task unpause --run` resumes it.",
            ),
        ] {
            let reading = read(cursor, auto);
            assert_eq!(reading.control.action, action, "{sentence}");
            assert_eq!(reading.sentence, sentence);
        }

        // Arriving at an interactive step under a hold stays At { step } with Unpause wording.
        write(
            repo.path(),
            ".lf/graphs/demo.yaml",
            "- cmd: help graph\n- interactive: demo\n",
        );
        let demo = CapturedGraph::load("demo", repo.path()).unwrap().unwrap();
        let parked = TaskGraph::read(
            &demo,
            "demo".into(),
            None,
            None,
            Some(PauseStage::Held),
            test_merges,
        );
        assert_eq!(parked.control.action, Unpause);
        assert_eq!(parked.control.label, "Paused: demo");
        assert_eq!(
            parked.sentence,
            "Paused at demo. `lf task unpause` lifts the hold."
        );
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
                    Some(TaskGraphRun {
                        cursor,
                        lf_process_id: "lfp_1".into(),
                        running: true,
                    }),
                    None,
                    None,
                    test_merges,
                );
                line.steps[line.current_step.unwrap() as usize].name.clone()
            })
            .collect();
        assert_eq!(labels, ["plan", "build"]);
    }
}
