//! The generic flowloop driver: loop a named flow in a placed worktree until
//! the loop file says done, under caps.
//!
//! The driver knows nothing about what the flow does or when it is done —
//! that judgment lives in the flow's skills. The termination bit is ONE
//! GENERIC CONTRACT, identical for every flowloop, and the driver teaches it
//! itself: every pass's seed carries a standing instruction
//! ([`loop_instruction`]) explaining how to mark for termination, so ANY
//! flow is loopable without its skills knowing loop mechanics. Purpose-built
//! loop flows (task, project) additionally discuss WHEN to flip
//! the bit in their mutate skill, with tier context.
//!
//! The bit is one file, read and REMOVED by the driver at every pass
//! boundary (it speaks for one boundary):
//!
//! ```yaml
//! # scratch/loop.yaml
//! done: true          # terminate the loop at this boundary
//! # or
//! recheck: gh pr view --json state -q .state | grep -q MERGED
//! ```
//!
//! `done` ends the loop. `recheck` is an agent-authored predicate the driver
//! polls mechanically (free — no pass burned) until it exits 0, then runs
//! one more pass so the flow can do its close-out and write `done`. No file
//! → the next pass starts immediately. Caps (max passes, wall clock)
//! escalate via `lf chat --parent` and error out.

use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::flowloop::pass::{run_pass, PassOptions};
use crate::flowloop::run::FlowloopRun;
use crate::ops::{OpsError, OpsResult};
use crate::wave::wire::{
    DetachedLoopRequest, DetachedLoopResponse, RESIDENT_TOKEN_HEADER, SUBAGENT_TOKEN_HEADER,
};

const LOOP_FILE: &str = "scratch/loop.yaml";
const DEFAULT_MAX_PASSES: u32 = 8;
const DEFAULT_PASS_TIMEOUT_SECS: u64 = 60 * 30;
const DEFAULT_WALL_CLOCK_SECS: u64 = 60 * 60 * 2;
const DEFAULT_POLL_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopOptions {
    /// The flow each pass runs (`task`, `project`, any flow).
    pub flow: String,
    pub wave: Option<String>,
    pub max_passes: u32,
    pub pass_timeout: Duration,
    pub wall_clock: Duration,
    pub poll: Duration,
    pub max_turns: Option<u32>,
}

impl LoopOptions {
    pub fn new(flow: String, wave: Option<String>) -> Self {
        Self {
            flow,
            wave,
            max_passes: DEFAULT_MAX_PASSES,
            pass_timeout: Duration::from_secs(DEFAULT_PASS_TIMEOUT_SECS),
            wall_clock: Duration::from_secs(DEFAULT_WALL_CLOCK_SECS),
            poll: Duration::from_secs(DEFAULT_POLL_SECS),
            max_turns: None,
        }
    }
}

/// What the loop file said at a pass boundary.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct LoopFile {
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub recheck: Option<String>,
}

/// `lf loop <flow> "<seed>"`: place a worktree through the wave
/// registry, then loop the flow over it until the loop file says done.
pub fn run_flowloop(repo: &Path, seed: &str, options: &LoopOptions) -> OpsResult<()> {
    require_loop_flow(repo, &options.flow)?;
    let wave_name = crate::ops::util::resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let mut run = FlowloopRun::start(&wave_name, &options.flow, seed.to_string())?;
    let worktree = run.worktree();
    eprintln!(
        "flowloop {} running in {}",
        options.flow,
        worktree.display()
    );
    let result = drive(
        &worktree,
        seed,
        options,
        |pass, flow, seed, pass_options| {
            run.start_pass(pass)?;
            run_pass(&worktree, flow, seed, pass_options)
        },
    );
    run.finish(result)
}

/// Resolve the loop target before creating a worktree. A skill is not a
/// one-step flow by implication: loop callers name a flow explicitly.
pub(crate) fn require_loop_flow(repo: &Path, flow: &str) -> OpsResult<()> {
    crate::engine::load_flow(flow, repo)
        .map(|_| ())
        .map_err(|err| OpsError::Message(format!("cannot loop flow '{flow}': {err}")))
}

/// Ask the live wave server to own the same loop invocation and return its
/// read-only inspection session. The server, not this short-lived CLI, owns
/// launch and observation.
pub fn detach_flowloop(repo: &Path, seed: &str, options: &LoopOptions) -> OpsResult<String> {
    let wave = crate::ops::util::resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let origin = crate::engine::wave_context::wave_origin(repo);
    let endpoint = crate::lfq::env_endpoint()
        .or_else(|| crate::engine::wave_context::read_endpoint_pointer(&origin, &wave))
        .ok_or_else(|| {
            OpsError::Message(format!(
                "wave '{wave}' has no live server; start it with `lf wave {wave}`"
            ))
        })?;
    let (header, token) = detached_loop_credential(&origin, &wave).ok_or_else(|| {
        OpsError::Message(format!(
            "wave '{wave}' has no loop-launch credential; restart its live server"
        ))
    })?;
    let request = DetachedLoopRequest {
        flow: options.flow.clone(),
        seed: seed.to_string(),
        max_passes: options.max_passes,
        pass_timeout_secs: options.pass_timeout.as_secs(),
        wall_clock_secs: options.wall_clock.as_secs(),
        poll_secs: options.poll.as_secs(),
        max_turns: options.max_turns,
    };
    let url = format!("http://{endpoint}/loops");
    let response = reqwest::blocking::Client::new()
        .post(&url)
        .header(header, token)
        .json(&request)
        .send()
        .map_err(|err| OpsError::Message(format!("POST {url} failed: {err}")))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|err| OpsError::Message(format!("reading {url} response failed: {err}")))?;
    if !status.is_success() {
        return Err(OpsError::Message(format!(
            "wave loop door refused ({status}): {body}"
        )));
    }
    let response: DetachedLoopResponse = serde_json::from_str(&body)
        .map_err(|err| OpsError::Parse(format!("invalid loop response: {err}")))?;
    Ok(response.session)
}

/// A sandboxed hand presents its own subagent capability; a shell beside the
/// wave falls back to the resident token file.
fn detached_loop_credential(origin: &Path, wave: &str) -> Option<(&'static str, String)> {
    if let Some(token) = crate::lfq::subagent_token() {
        return Some((SUBAGENT_TOKEN_HEADER, token));
    }
    crate::wave::server::read_resident_token(origin, wave)
        .map(|token| (RESIDENT_TOKEN_HEADER, token))
}

/// The standing instruction appended to every pass's seed: the generic
/// how-to-terminate contract. The WHEN belongs to the flow's skills.
fn loop_instruction(pass: u32, max_passes: u32) -> String {
    format!(
        "<lf:flowloop>\n\
         This flow is running inside a flowloop — pass {pass} of at most \
         {max_passes}. The loop repeats until you mark it terminated. To \
         terminate, write `scratch/loop.yaml` before this pass ends:\n\n\
         - `done: true` — the loop stops at this boundary. Flip it only when \
         the work's real-world condition is satisfied and you have checked \
         it yourself; \"done\" in prose counts for nothing.\n\
         - `recheck: <shell command>` — you are waiting on the world (a PR \
         review, CI, a human). The runner polls the command for free and \
         runs one more pass when it exits 0.\n\n\
         The file is consumed at every boundary — write it fresh each pass \
         or the loop simply continues. Exhausting the pass budget without \
         `done` escalates to the parent as a failure.\n\
         </lf:flowloop>"
    )
}

/// The loop itself, pass execution injected so tests drive it with closures.
fn drive(
    worktree: &Path,
    seed: &str,
    options: &LoopOptions,
    mut pass: impl FnMut(u32, &str, &str, &PassOptions) -> OpsResult<()>,
) -> OpsResult<()> {
    let started = Instant::now();
    let pass_options = PassOptions {
        timeout: options.pass_timeout,
        max_turns: options.max_turns,
    };
    for n in 1..=options.max_passes {
        check_wall_clock(started, options.wall_clock).inspect_err(|err| {
            escalate_parent(&err.to_string());
        })?;
        eprintln!("{} pass {n}/{}", options.flow, options.max_passes);
        let pass_seed = format!("{seed}\n\n{}", loop_instruction(n, options.max_passes));
        pass(n, &options.flow, &pass_seed, &pass_options)?;

        match take_loop_file(worktree)? {
            LoopFile { done: true, .. } => return Ok(()),
            LoopFile {
                recheck: Some(predicate),
                ..
            } => wait_for_recheck(worktree, &predicate, options, started)?,
            LoopFile { .. } => {}
        }
    }

    let message = format!(
        "flowloop {} exhausted {} pass(es) without done",
        options.flow, options.max_passes
    );
    escalate_parent(&message);
    Err(OpsError::Message(message))
}

/// Read and remove the loop file — it speaks for one boundary only.
fn take_loop_file(worktree: &Path) -> OpsResult<LoopFile> {
    let path = worktree.join(LOOP_FILE);
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(LoopFile::default()),
        Err(err) => return Err(OpsError::from(err)),
    };
    std::fs::remove_file(&path)?;
    serde_yaml_ng::from_str(&raw)
        .map_err(|err| OpsError::Parse(format!("unparseable {LOOP_FILE}: {err}")))
}

/// Poll an agent-authored predicate (free — no pass burned) until it exits 0,
/// then return so the loop runs a close-out pass.
fn wait_for_recheck(
    worktree: &Path,
    predicate: &str,
    options: &LoopOptions,
    started: Instant,
) -> OpsResult<()> {
    eprintln!("waiting: {predicate}");
    loop {
        check_wall_clock(started, options.wall_clock).inspect_err(|err| {
            escalate_parent(&err.to_string());
        })?;
        let fired = Command::new("sh")
            .args(["-c", predicate])
            .current_dir(worktree)
            .status()?
            .success();
        if fired {
            return Ok(());
        }
        thread::sleep(options.poll);
    }
}

fn check_wall_clock(started: Instant, wall_clock: Duration) -> OpsResult<()> {
    if started.elapsed() >= wall_clock {
        return Err(OpsError::Message(format!(
            "flowloop exceeded wall-clock cap of {}s",
            wall_clock.as_secs()
        )));
    }
    Ok(())
}

fn escalate_parent(message: &str) {
    // In tests current_exe is the test binary; execing it as `lf` is noise.
    if cfg!(test) {
        return;
    }
    let exe = std::env::current_exe()
        .map(Command::new)
        .unwrap_or_else(|_| Command::new("lf"));
    let mut cmd = exe;
    let _ = cmd.arg("chat").arg("--parent").arg(message).status();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(max_passes: u32) -> LoopOptions {
        LoopOptions {
            flow: "task".to_string(),
            wave: None,
            max_passes,
            pass_timeout: Duration::from_secs(5),
            wall_clock: Duration::from_secs(5),
            poll: Duration::from_millis(10),
            max_turns: None,
        }
    }

    fn write_loop_file(dir: &Path, content: &str) {
        std::fs::create_dir_all(dir.join("scratch")).unwrap();
        std::fs::write(dir.join(LOOP_FILE), content).unwrap();
    }

    #[test]
    fn done_stops_the_loop_at_the_boundary() {
        let tmp = tempfile::tempdir().unwrap();
        let mut passes = 0;
        drive(tmp.path(), "seed", &options(8), |_, _, _, _| {
            passes += 1;
            write_loop_file(tmp.path(), "done: true\n");
            Ok(())
        })
        .expect("done terminates cleanly");
        assert_eq!(passes, 1);
        assert!(!tmp.path().join(LOOP_FILE).exists(), "file consumed");
    }

    #[test]
    fn missing_file_keeps_passing_until_the_cap_escalates() {
        let tmp = tempfile::tempdir().unwrap();
        let mut passes = 0;
        let err = drive(tmp.path(), "seed", &options(3), |_, _, _, _| {
            passes += 1;
            Ok(())
        })
        .expect_err("cap fires");
        assert_eq!(passes, 3);
        assert!(err.to_string().contains("exhausted 3 pass(es)"));
    }

    #[test]
    fn recheck_polls_free_then_runs_a_closeout_pass() {
        let tmp = tempfile::tempdir().unwrap();
        let flag = tmp.path().join("merged");
        std::fs::write(&flag, "").unwrap();
        let mut passes = 0;
        drive(tmp.path(), "seed", &options(8), |_, _, _, _| {
            passes += 1;
            if passes == 1 {
                // Submitted; wait for the (already-set) external bit.
                write_loop_file(
                    tmp.path(),
                    &format!("recheck: test -f {}\n", flag.display()),
                );
            } else {
                // Close-out pass observes the bit and finishes.
                write_loop_file(tmp.path(), "done: true\n");
            }
            Ok(())
        })
        .expect("recheck then done");
        assert_eq!(passes, 2, "one work pass + one close-out pass");
    }

    #[test]
    fn recheck_that_never_fires_hits_the_wall_clock() {
        let tmp = tempfile::tempdir().unwrap();
        let mut opts = options(8);
        opts.wall_clock = Duration::from_millis(50);
        let err = drive(tmp.path(), "seed", &opts, |_, _, _, _| {
            write_loop_file(tmp.path(), "recheck: false\n");
            Ok(())
        })
        .expect_err("wall clock fires during recheck");
        assert!(err.to_string().contains("wall-clock"));
    }

    #[test]
    fn garbage_loop_file_is_an_error_not_a_silent_continue() {
        let tmp = tempfile::tempdir().unwrap();
        let err = drive(tmp.path(), "seed", &options(8), |_, _, _, _| {
            write_loop_file(tmp.path(), ": not yaml [");
            Ok(())
        })
        .expect_err("unparseable file errors");
        assert!(err.to_string().contains("unparseable"));
    }

    #[test]
    fn loop_target_must_resolve_to_a_flow() {
        let tmp = tempfile::tempdir().unwrap();
        require_loop_flow(tmp.path(), "task").expect("builtin task flow");
        let err = require_loop_flow(tmp.path(), "definitely-not-a-flow").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }
}
