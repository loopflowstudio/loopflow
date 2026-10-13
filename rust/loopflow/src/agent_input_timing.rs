//! Opt-in wall milestones for native input profiling; never readiness or authority.
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn record(phase: &str, session: &str) {
    let Some(path) = std::env::var_os("LOOPFLOW_AGENT_INPUT_TRACE") else {
        return;
    };
    let Ok(wall) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return;
    };
    let event = serde_json::json!({
        "phase": phase,
        "session": session,
        "wall": wall.as_secs_f64(),
        "pid": std::process::id(),
        "version": env!("CARGO_PKG_VERSION"),
    });
    // One small append, with no arguments, prompt, environment or terminal text.
    // Missing/unwritable diagnostic output must not change a native launch.
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let line = format!("{event}\n");
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn records_wall_milestones_without_terminal_content() {
        let _environment = loopflow_test_support::env_lock();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("events.ndjson");
        let previous = std::env::var_os("LOOPFLOW_AGENT_INPUT_TRACE");
        std::env::set_var("LOOPFLOW_AGENT_INPUT_TRACE", &path);
        super::record("launch_lock_acquired", "session-fixture");
        match previous {
            Some(value) => std::env::set_var("LOOPFLOW_AGENT_INPUT_TRACE", value),
            None => std::env::remove_var("LOOPFLOW_AGENT_INPUT_TRACE"),
        }
        let event: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(event["phase"], "launch_lock_acquired");
        assert_eq!(event["session"], "session-fixture");
        assert!(event["wall"].as_f64().unwrap() > 0.0);
        assert_eq!(event.as_object().unwrap().len(), 5);
    }
}
