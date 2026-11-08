//! Wall milestones of each native launch, always kept in a small rotated log so
//! a slow launch can be explained afterwards; never readiness or authority.
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const FILE_NAME: &str = "launch-timing.jsonl";
/// The live file rolls to `.1` past this size, so both stay near 2 MiB.
const ROLL_BYTES: u64 = 1024 * 1024;

fn load_average() -> Option<f64> {
    let mut loads = [0f64; 3];
    // SAFETY: getloadavg writes at most `nelem` doubles into the buffer.
    let filled = unsafe { libc::getloadavg(loads.as_mut_ptr(), 3) };
    (filled >= 1).then_some(loads[0])
}

pub(crate) fn record(phase: &str, subject: &str) {
    let Ok(wall) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return;
    };
    let event = serde_json::json!({
        "phase": phase,
        "session": subject,
        "wall": wall.as_secs_f64(),
        "pid": std::process::id(),
        "version": env!("CARGO_PKG_VERSION"),
        "load1": load_average(),
    });
    // One small append, with no arguments, prompt, environment or terminal text.
    // Missing/unwritable diagnostic output must not change a native launch.
    let path = match std::env::var_os("LOOPFLOW_AGENT_INPUT_TRACE") {
        // Benchmarks redirect the log, unrotated.
        Some(path) => PathBuf::from(path),
        None => {
            let path = crate::store::lf_home_dir().join("logs").join(FILE_NAME);
            // The log never creates a home that previews and reads leave untouched.
            if !path.parent().is_some_and(Path::is_dir) {
                return;
            }
            if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > ROLL_BYTES) {
                let _ = std::fs::rename(&path, path.with_extension("jsonl.1"));
            }
            path
        }
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let line = format!("{event}\n");
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    fn with_env<T>(name: &str, value: Option<&OsStr>, run: impl FnOnce() -> T) -> T {
        let previous = std::env::var_os(name);
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
        let result = run();
        match previous {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
        result
    }

    #[test]
    fn records_wall_milestones_without_terminal_content() {
        let _environment = loopflow_test_support::env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("events.ndjson");
        with_env("LOOPFLOW_AGENT_INPUT_TRACE", Some(path.as_os_str()), || {
            super::record("launch_lock_acquired", "session-fixture")
        });
        let event: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(event["phase"], "launch_lock_acquired");
        assert_eq!(event["session"], "session-fixture");
        assert!(event["wall"].as_f64().unwrap() > 0.0);
        assert!(event["load1"].as_f64().unwrap() >= 0.0);
        assert_eq!(event.as_object().unwrap().len(), 6);
    }

    #[test]
    fn always_logs_under_the_home_and_rolls_past_a_mebibyte() {
        let _environment = loopflow_test_support::env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let home = tempfile::tempdir().unwrap();
        let log = home.path().join("logs").join(super::FILE_NAME);
        with_env("LOOPFLOW_AGENT_INPUT_TRACE", None, || {
            with_env("LF_HOME", Some(home.path().as_os_str()), || {
                super::record("connect_begin", "no logs directory yet");
                assert!(!log.parent().unwrap().exists());
                std::fs::create_dir(log.parent().unwrap()).unwrap();
                super::record("connect_begin", "first");
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&log)
                    .unwrap()
                    .set_len(super::ROLL_BYTES + 1)
                    .unwrap();
                super::record("connect_begin", "second");
            })
        });
        assert!(log.with_extension("jsonl.1").exists());
        let live = std::fs::read_to_string(&log).unwrap();
        assert_eq!(live.lines().count(), 1);
        assert!(live.contains("\"second\""));
    }
}
