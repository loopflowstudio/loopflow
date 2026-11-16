//! Opt-in counters for the server subscription experiment. No installed code.
use std::collections::BTreeMap;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;

pub use loopflow_storage::instrumentation::SUBSCRIPTION_ENABLED as ENABLED;
static STAGES: Mutex<BTreeMap<&'static str, Count>> = Mutex::new(BTreeMap::new());

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Count {
    pub count: u64,
    pub wall_ms: f64,
    pub caller_cpu_ms: f64,
}

fn thread_cpu() -> f64 {
    // SAFETY: clock_gettime writes to a valid timespec; this clock is per thread.
    unsafe {
        let mut time = std::mem::zeroed::<libc::timespec>();
        assert_eq!(
            libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time),
            0
        );
        time.tv_sec as f64 * 1000.0 + time.tv_nsec as f64 / 1_000_000.0
    }
}

pub struct Stage(Option<(&'static str, Instant, f64)>);
impl Stage {
    pub fn new(name: &'static str) -> Self {
        Self(
            ENABLED
                .load(Relaxed)
                .then(|| (name, Instant::now(), thread_cpu())),
        )
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if let Some((name, began, cpu)) = self.0 {
            let mut stages = STAGES.lock().unwrap();
            let value = stages.entry(name).or_default();
            value.count += 1;
            value.wall_ms += began.elapsed().as_secs_f64() * 1000.0;
            value.caller_cpu_ms += thread_cpu() - cpu;
        }
    }
}

/// Same builder as Runtime::new. Hooks count actual worker and blocking threads.
pub struct Runtime(Option<tokio::runtime::Runtime>);
impl Runtime {
    pub fn build(builder: &mut tokio::runtime::Builder) -> std::io::Result<Self> {
        let _stage = Stage::new("runtime_build");
        builder
            .on_thread_start(|| drop(Stage::new("runtime_thread_start")))
            .on_thread_stop(|| drop(Stage::new("runtime_thread_stop")))
            .build()
            .map(|runtime| Self(Some(runtime)))
    }
}
impl std::ops::Deref for Runtime {
    type Target = tokio::runtime::Runtime;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _stage = Stage::new("runtime_drop");
        drop(self.0.take());
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub user_ms: f64,
    pub system_ms: f64,
    pub child_user_ms: f64,
    pub child_system_ms: f64,
    pub statements: u64,
    pub rows: u64,
    pub vm_steps: u64,
    pub opens: u64,
    pub closes: u64,
    pub stages: BTreeMap<&'static str, Count>,
}

fn usage(who: i32) -> (f64, f64) {
    // SAFETY: getrusage initializes a writable rusage of the required size.
    unsafe {
        let mut u = std::mem::zeroed::<libc::rusage>();
        assert_eq!(libc::getrusage(who, &mut u), 0);
        (
            u.ru_utime.tv_sec as f64 * 1000.0 + u.ru_utime.tv_usec as f64 / 1000.0,
            u.ru_stime.tv_sec as f64 * 1000.0 + u.ru_stime.tv_usec as f64 / 1000.0,
        )
    }
}

pub fn snapshot() -> Snapshot {
    let (user_ms, system_ms) = usage(libc::RUSAGE_SELF);
    let (child_user_ms, child_system_ms) = usage(libc::RUSAGE_CHILDREN);
    let sql = loopflow_storage::instrumentation::subscription_snapshot();
    Snapshot {
        user_ms,
        system_ms,
        child_user_ms,
        child_system_ms,
        statements: sql.statements,
        rows: sql.rows,
        vm_steps: sql.vm_steps,
        opens: sql.opens,
        closes: sql.closes,
        stages: STAGES.lock().unwrap().clone(),
    }
}
impl Snapshot {
    pub fn since(mut self, before: Self) -> Self {
        self.user_ms -= before.user_ms;
        self.system_ms -= before.system_ms;
        self.child_user_ms -= before.child_user_ms;
        self.child_system_ms -= before.child_system_ms;
        self.statements -= before.statements;
        self.rows -= before.rows;
        self.vm_steps -= before.vm_steps;
        self.opens -= before.opens;
        self.closes -= before.closes;
        for (name, old) in before.stages {
            let value = self.stages.entry(name).or_default();
            value.count -= old.count;
            value.wall_ms -= old.wall_ms;
            value.caller_cpu_ms -= old.caller_cpu_ms;
        }
        self
    }
}
