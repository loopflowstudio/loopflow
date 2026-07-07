pub mod oracle;
pub mod pass;
pub mod project;
pub mod run;
pub mod task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Wave,
    Project,
    Task,
}

impl Tier {
    pub fn pass_flow(self) -> &'static str {
        match self {
            Self::Wave => "wave-pass",
            Self::Project => "project-pass",
            Self::Task => "task-pass",
        }
    }
}
