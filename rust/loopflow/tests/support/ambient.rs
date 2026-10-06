use std::ffi::OsString;

/// Execution authority and capture context inherited by tests launched inside lf.
pub const AMBIENT_TASK_ENV: &[&str] = &[
    "LF_CAPTURE_KEY",
    "LF_AGENT_CALLER",
    "LF_AS",
    "LF_RUN_ID",
    "LF_RUN_DIR",
    "LF_FLOW_STEP",
    "LF_WORK_ADVANCE_CLAIM",
    "LF_WAVE_ID",
    "LF_ACCOUNT_LEASE",
    "LF_ACCOUNT_SELECTION",
    "LF_ACCOUNT_ISOLATION",
    "LF_HUMAN_SESSION",
];

/// Call while holding the suite's environment lock; restore on drop.
#[derive(Debug)]
pub struct EnvGuard(Vec<(&'static str, Option<OsString>)>);

impl EnvGuard {
    pub fn new() -> Self {
        Self::clear(AMBIENT_TASK_ENV)
    }

    pub fn clear(names: &[&'static str]) -> Self {
        Self(
            names
                .iter()
                .map(|&name| {
                    let value = std::env::var_os(name);
                    std::env::remove_var(name);
                    (name, value)
                })
                .collect(),
        )
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.0 {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}
