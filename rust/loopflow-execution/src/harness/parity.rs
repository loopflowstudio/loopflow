//! Behaviors people rely on across harnesses, and where each one is checked.
use loopflow_harness::HarnessKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behavior {
    WorkerStep,
    UsageRecording,
    StepContext,
    MidTurnSteer,
    HeadlessPermissions,
    MissingCli,
    UnansweredQuestion,
    Interrupt,
    RefusalFallthrough,
    IdleWorkSessionWake,
    ReopenWaitingSteers,
    InstalledSkills,
    ViewerHandoff,
}

impl Behavior {
    pub const ALL: [Self; 13] = [
        Self::WorkerStep,
        Self::UsageRecording,
        Self::StepContext,
        Self::MidTurnSteer,
        Self::HeadlessPermissions,
        Self::MissingCli,
        Self::UnansweredQuestion,
        Self::Interrupt,
        Self::RefusalFallthrough,
        Self::IdleWorkSessionWake,
        Self::ReopenWaitingSteers,
        Self::InstalledSkills,
        Self::ViewerHandoff,
    ];

    #[cfg(test)]
    fn description(self) -> &'static str {
        match self {
            Self::WorkerStep => "starts as a worker, settles, records its answer and native thread; the Graph advances",
            Self::UsageRecording => "records its usage in the Task's history",
            Self::StepContext => "receives the step's context",
            Self::MidTurnSteer => "hears a steer sent while it runs, after its next tool",
            Self::HeadlessPermissions => "never asks permission, whatever `yolo` says",
            Self::MissingCli => "is passed over when its CLI is not installed, or named once when it was the only choice",
            Self::UnansweredQuestion => "fails the turn, with the question as its reason, when it asks one and nobody is there; the run stops after that one attempt",
            Self::Interrupt => "stops on `lf task interrupt` with its turn recorded interrupted; the next `lf task run` starts the step again",
            Self::RefusalFallthrough => "moves to the next agent on a usage limit, a revoked login or a model the provider does not serve",
            Self::IdleWorkSessionWake => "as an idle work session, hears a steer",
            Self::ReopenWaitingSteers => "reopened work session takes waiting steers as a turn",
            Self::InstalledSkills => "lists synced skills and Graphs",
            Self::ViewerHandoff => "opening in another viewer keeps the conversation",
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    #[track_caller]
    pub fn assert_observed(self, harness: HarnessKind, observed: bool) {
        let declared = support(harness, self);
        assert_eq!(
            observed,
            declared == Support::Yes,
            "{harness:?} {self:?}: {declared:?}"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Yes,
    Gap {
        why: &'static str,
        owner: &'static str,
    },
}

pub fn support(harness: HarnessKind, behavior: Behavior) -> Support {
    use Behavior::{
        HeadlessPermissions, IdleWorkSessionWake, InstalledSkills, Interrupt, MidTurnSteer,
        MissingCli, RefusalFallthrough, ReopenWaitingSteers, StepContext, UnansweredQuestion,
        UsageRecording, ViewerHandoff, WorkerStep,
    };
    use HarnessKind::{Antigravity, Claude, Codex, OpenCode, Pi};
    match (harness, behavior) {
        (
            Claude | Codex | OpenCode | Pi | Antigravity,
            WorkerStep | UsageRecording | StepContext | MidTurnSteer | HeadlessPermissions
            | MissingCli | Interrupt | RefusalFallthrough | ReopenWaitingSteers | InstalledSkills,
        )
        | (OpenCode | Pi, UnansweredQuestion)
        | (Claude | Codex | OpenCode | Pi, IdleWorkSessionWake | ViewerHandoff) => Support::Yes,
        (Claude | Codex | Antigravity, UnansweredQuestion) => Support::Gap {
            why: "question is not read",
            owner: "Task 48edb9",
        },
        (Antigravity, IdleWorkSessionWake) => Support::Gap {
            why: "next turn only",
            owner: "LOO-1099",
        },
        (Antigravity, ViewerHandoff) => Support::Gap {
            why: "second viewer closes first",
            owner: "LOO-1097",
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verification {
    Automated(&'static str),
    RealProvider(&'static str),
}

pub fn verification(behavior: Behavior) -> Verification {
    use Verification::{Automated, RealProvider};
    match behavior {
        Behavior::WorkerStep | Behavior::UsageRecording | Behavior::StepContext | Behavior::MidTurnSteer =>
            Automated("task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness"),
        Behavior::HeadlessPermissions => Automated("harness::launch_matrix::every_harness_runs_all_four_launch_kinds"),
        Behavior::MissingCli => Automated("task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again"),
        Behavior::UnansweredQuestion => Automated("task_graph_launch_tests::a_question_with_nobody_there_ends_the_step"),
        Behavior::Interrupt => Automated("task_graph_launch_tests::a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again"),
        Behavior::RefusalFallthrough => Automated("harness::conformance_tests::each_harness_classifies_its_own_refusals"),
        Behavior::IdleWorkSessionWake => Automated("server::wake::tests::every_harness_wakes_its_idle_work_session"),
        Behavior::ReopenWaitingSteers => Automated("server::wake::tests::reopened_work_sessions_take_waiting_steers, session_launch_contract::a_reopened_antigravity_task_hears_repository_direction_once"),
        Behavior::InstalledSkills => Automated("harness::conformance_tests::every_harness_reads_a_synced_skill"),
        Behavior::ViewerHandoff => RealProvider("transport_tests covers held-terminal sharing. Check the provider TUI live"),
    }
}

#[cfg(test)]
mod tests {
    use super::{support, verification, Behavior, HarnessKind, Support, Verification};
    use std::fmt::Write;

    const README: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/harness/README.md");
    const START: &str = "<!-- parity:start -->";
    const END: &str = "<!-- parity:end -->";

    fn render(readme: &str) -> String {
        let (before, rest) = readme
            .split_once(START)
            .expect("README has a parity start marker");
        let (_, after) = rest
            .split_once(END)
            .expect("README has a parity end marker");
        let mut out = format!("{before}{START}\n| behavior |");
        for harness in HarnessKind::ALL {
            write!(out, " {harness:?} |").unwrap();
        }
        writeln!(out, " verification |").unwrap();
        writeln!(out, "|{}", "---|".repeat(HarnessKind::ALL.len() + 2)).unwrap();
        for behavior in Behavior::ALL {
            write!(out, "| {} |", behavior.description()).unwrap();
            for harness in HarnessKind::ALL {
                match support(harness, behavior) {
                    Support::Yes => out.push_str(" yes |"),
                    Support::Gap { why, owner } => write!(out, " no: {why} ({owner}) |").unwrap(),
                }
            }
            match verification(behavior) {
                Verification::Automated(tests) => {
                    let names = tests
                        .split(", ")
                        .map(|test| format!("`{test}`"))
                        .collect::<Vec<_>>();
                    writeln!(out, " automated ({}) |", names.join(", ")).unwrap();
                }
                Verification::RealProvider(check) => {
                    writeln!(out, " real provider ({check}) |").unwrap()
                }
            }
        }
        write!(out, "{END}{after}").unwrap();
        out
    }

    #[test]
    fn readme_table_matches_support() {
        let readme = include_str!("README.md");
        assert_eq!(
            readme,
            render(readme),
            "regenerate the parity table with regenerate_readme_table -- --ignored"
        );
    }

    #[test]
    #[ignore = "rewrites the harness README"]
    fn regenerate_readme_table() {
        let readme = std::fs::read_to_string(README).unwrap();
        std::fs::write(README, render(&readme)).unwrap();
    }
}
