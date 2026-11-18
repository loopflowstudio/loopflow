//! Opt-in process and SQLite volume receipts. No SQL, arguments or row values
//! leave the process. LF_PERF_OUTPUT names an existing output directory; it
//! selects observation only and provides no Machine or execution authority.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use loopflow_storage::instrumentation::{
    CONNECTIONS, ENABLED, ROWS, SQL_NANOS, SQL_PROFILES, STATEMENTS,
};

/// One receipt stream per actual CLI process, including nested CLI invocations.
/// Missing final records expose interruption rather than implying zero work.
#[derive(Debug)]
pub struct ProcessMeasurement {
    stop: Sender<()>,
    writer: Option<JoinHandle<()>>,
}

impl ProcessMeasurement {
    pub fn start() -> Option<Self> {
        let directory = std::env::var_os("LF_PERF_OUTPUT")?;
        let path = std::path::PathBuf::from(directory).join(format!(
            "lf-{}-{}.jsonl",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let mut file = match OpenOptions::new().create_new(true).write(true).open(path) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("Warning: cannot record CLI performance: {error}");
                return None;
            }
        };
        let started = Instant::now();
        if let Err(error) = record(&mut file, "start", started) {
            eprintln!("Warning: cannot record CLI performance: {error}");
            return None;
        }
        let (stop, receiver) = mpsc::channel();
        let writer = std::thread::Builder::new()
            .name("lf-perf".into())
            .spawn(move || loop {
                let event = match receiver.recv_timeout(Duration::from_secs(1)) {
                    Err(mpsc::RecvTimeoutError::Timeout) => "sample",
                    _ => "end",
                };
                if let Err(error) = record(&mut file, event, started) {
                    eprintln!("Warning: cannot record CLI performance: {error}");
                    break;
                }
                if event == "end" {
                    break;
                }
            });
        let writer = match writer {
            Ok(writer) => writer,
            Err(error) => {
                eprintln!("Warning: cannot start CLI performance writer: {error}");
                return None;
            }
        };
        ENABLED.store(true, Ordering::Relaxed);
        Some(Self {
            stop,
            writer: Some(writer),
        })
    }
}

impl Drop for ProcessMeasurement {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        ENABLED.store(false, Ordering::Relaxed);
    }
}

fn record(file: &mut File, event: &str, started: Instant) -> std::io::Result<()> {
    let value = serde_json::json!({
        "event": event,
        "pid": std::process::id(),
        "time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64(),
        "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
        "connections": CONNECTIONS.load(Ordering::Relaxed),
        "statements": STATEMENTS.load(Ordering::Relaxed),
        "rows": ROWS.load(Ordering::Relaxed),
        "interaction": std::env::var("LF_PERF_INTERACTION").ok(),
        "sql_profiles": SQL_PROFILES.load(Ordering::Relaxed),
        "sql_ms": SQL_NANOS.load(Ordering::Relaxed) as f64 / 1_000_000.0,
    });
    writeln!(file, "{value}")
}

#[cfg(any(test, feature = "test-support"))]
pub mod subscription_measurement;
