pub mod activity;
pub mod ask;
pub mod auth;
pub mod chat;
pub mod ci;
pub mod desktop;
pub mod doctor;
#[cfg(test)]
pub(crate) mod fixtures;
pub mod flow;
pub mod home;
pub mod install;
pub mod list;
pub mod ops;
pub mod profile;
pub mod replay;
pub mod run;
pub mod runs;
pub mod screenshot;
pub mod ssh;
pub mod thread;
pub mod tokens;
pub mod top;
pub mod usage;
pub mod util;
pub mod wave_intent;
pub mod waves;
pub mod work;

/// One drill over the Wave → Project → Task Work hierarchy.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WorkFilter<'a> {
    pub wave: Option<&'a str>,
    pub project: Option<&'a str>,
    pub task: Option<&'a str>,
}

impl WorkFilter<'_> {
    pub(crate) fn matches(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
    ) -> bool {
        self.wave.is_none_or(|expected| wave == Some(expected))
            && self
                .project
                .is_none_or(|expected| project == Some(expected))
            && self.task.is_none_or(|expected| task == Some(expected))
    }
}
