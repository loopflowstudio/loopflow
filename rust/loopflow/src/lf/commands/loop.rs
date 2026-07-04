use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::engine::stream::StreamParser;
use crate::engine::worktrees::main_repo_root;
use crate::lf::commands::util::find_repo_root;
use crate::lfd::conversations::server::{self, ChatState};
use crate::lfd::executor::helpers::resolve_lf_binary;
use crate::ops::util::resolve_wave_name;

/// Dropping this file into `wave/<wave>/` stops the loop after the current pass.
const STOP_FILE: &str = "STOP";

/// Per-wave NDJSON sink: each inner pass appends its raw agent stream-json here
/// (via `LF_WAVE_EVENT_SINK`), and the chat server tails it into `ChatTurn`s.
const EVENT_SINK_FILE: &str = ".chat-events.ndjson";

/// Cooldown after a failed pass so a broken inner run can't hot-spin the loop.
/// A successful pass repeats immediately — loopflow owns the cadence, gated only
/// on the inner pass finishing.
const FAILURE_COOLDOWN: Duration = Duration::from_secs(3);

/// Run the progress loop for a wave, hosting its live chat server in-process.
///
/// loopflow owns the *outer* loop: each pass is a single bounded
/// `lf -b goal <wave> --once`, and the loop fires the next pass as soon as the
/// previous one finishes. This is the deterministic controller that replaces
/// relying on the model's own goal loop (which gets stuck). It repeats until
/// interrupted (Ctrl-C) or until `wave/<wave>/STOP` appears.
///
/// Alongside the loop, `lf wave` hosts a per-wave chat server (see
/// [`crate::lfd::conversations::server`]). Each pass writes its raw agent
/// stream-json to a per-wave sink; a tailer folds that into `ChatTurn`s the
/// server streams to Concerto. The server's `host:port` is published to
/// `wave/<wave>/.chat-endpoint`.
pub fn run(name: &str) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or(repo_root);
    let wave_name = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;

    let wave_dir = main_repo.join("wave").join(&wave_name);
    std::fs::create_dir_all(&wave_dir)
        .with_context(|| format!("create wave dir {}", wave_dir.display()))?;
    let stop_file = wave_dir.join(STOP_FILE);
    let sink_path = wave_dir.join(EVENT_SINK_FILE);
    // Fresh sink per run so the transcript reflects this session's passes.
    let _ = std::fs::write(&sink_path, b"");
    let lf = resolve_lf_binary();

    // Bring up the chat server before the first pass so Concerto can attach
    // immediately. Held in `_rt` for the life of the loop; dropping it stops the
    // server.
    let state = ChatState::new(wave_name.clone(), wave_dir.clone());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build wave chat runtime")?;
    let listener = rt
        .block_on(server::bind(&wave_dir))
        .context("start wave chat server")?;
    let addr = listener.local_addr().context("resolve chat server addr")?;
    let router = server::router(state.clone());
    rt.spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            tracing::error!(error = %err, "wave chat server exited");
        }
    });

    // Tail the sink file into the chat server on a plain thread — ingest is sync.
    let stop_tailer = Arc::new(AtomicBool::new(false));
    let tailer = spawn_tailer(sink_path.clone(), state.clone(), stop_tailer.clone());

    // Inner passes inherit this env and append their stream-json to the sink.
    std::env::set_var("LF_WAVE_EVENT_SINK", &sink_path);

    println!("lf wave · {wave_name} · loopflow owns the outer loop (Ctrl-C to stop)");
    println!("lf wave · {wave_name} · chat: http://{addr} (wave/{wave_name}/.chat-endpoint)");

    let result = run_loop(&lf, &main_repo, &wave_name, &stop_file, &state);

    // Tear down: stop tailing, remove discovery file, let the runtime drop.
    stop_tailer.store(true, Ordering::SeqCst);
    let _ = tailer.join();
    server::remove_endpoint_file(&wave_dir);
    result
}

fn run_loop(
    lf: &Path,
    main_repo: &Path,
    wave_name: &str,
    stop_file: &Path,
    state: &Arc<ChatState>,
) -> Result<()> {
    let mut pass: u32 = 0;
    loop {
        if stop_file.exists() {
            println!(
                "lf wave · {wave_name} · stopping: stop file present ({}) ({pass} passes)",
                stop_file.display()
            );
            return Ok(());
        }

        pass += 1;
        state.begin_pass();
        println!("-- lf wave · {wave_name} · pass {pass} --");

        match run_pass(lf, main_repo, wave_name)? {
            PassOutcome::Ok => {}
            PassOutcome::Failed(status) => {
                eprintln!(
                    "lf wave · pass {pass} exited with {status}; cooling down {}s",
                    FAILURE_COOLDOWN.as_secs()
                );
                std::thread::sleep(FAILURE_COOLDOWN);
            }
            PassOutcome::Signaled => {
                // Ctrl-C (or another signal) killed the inner pass — treat it as
                // the operator stopping the loop, not a pass to retry.
                println!("lf wave · {wave_name} · interrupted ({pass} passes)");
                return Ok(());
            }
        }
    }
}

/// The outcome of a pass that actually ran. Setup failures (spawning the inner
/// `lf`) propagate as `Err` from `run_pass`, not as a variant here.
enum PassOutcome {
    Ok,
    Failed(ExitStatus),
    Signaled,
}

/// Run one bounded pass: `lf -b goal <wave> --once`, inheriting the terminal so
/// the inner agent streams straight to the operator. The inner pass writes its
/// own durable logs under the agent's log dir, so the loop keeps no copy.
fn run_pass(lf: &Path, repo: &Path, wave: &str) -> Result<PassOutcome> {
    let status = Command::new(lf)
        .arg("-b")
        .arg("goal")
        .arg(wave)
        .arg("--once")
        .current_dir(repo)
        .status()
        .with_context(|| format!("failed to run `lf -b goal {wave} --once`"))?;

    Ok(if status.success() {
        PassOutcome::Ok
    } else if status.code().is_none() {
        PassOutcome::Signaled
    } else {
        PassOutcome::Failed(status)
    })
}

/// Follow the per-wave event sink, feeding each complete stream-json line to the
/// chat server. Plain thread + polling — the child pass is the only writer, so a
/// short poll is enough and avoids inotify/kqueue platform differences.
fn spawn_tailer(
    sink: PathBuf,
    state: Arc<ChatState>,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut parser = StreamParser::new();
        let mut offset: u64 = 0;
        while !stop.load(Ordering::SeqCst) {
            offset = drain_sink(&sink, offset, &mut parser, &state);
            std::thread::sleep(Duration::from_millis(100));
        }
        // Final drain so the last pass's tail isn't lost on shutdown.
        drain_sink(&sink, offset, &mut parser, &state);
    })
}

/// Read complete newline-terminated lines from `sink` starting at `offset`,
/// feeding each to the chat server. Returns the new offset (past the last full
/// line); a trailing partial line is left for the next drain.
fn drain_sink(sink: &Path, offset: u64, parser: &mut StreamParser, state: &Arc<ChatState>) -> u64 {
    let Ok(file) = std::fs::File::open(sink) else {
        return offset;
    };
    let mut reader = BufReader::new(file);
    if reader.seek(SeekFrom::Start(offset)).is_err() {
        return offset;
    }
    let mut pos = offset;
    let mut line = String::new();
    loop {
        line.clear();
        let read = match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        if line.ends_with('\n') {
            pos += read as u64;
            state.ingest_line(parser, line.trim_end_matches(['\n', '\r']));
        } else {
            // Partial line: leave it for the next drain.
            break;
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_failure_propagates_as_error() {
        // A missing inner binary is a setup failure, not a pass outcome — it
        // surfaces as `Err`, bubbling out of the loop rather than cooling down.
        let missing = Path::new("/definitely/not/a/real/lf-binary");
        let result = run_pass(missing, Path::new("/tmp"), "ghost");
        assert!(result.is_err());
    }

    #[test]
    fn drain_sink_feeds_complete_lines_and_holds_partial() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sink = dir.path().join("events.ndjson");
        let state = ChatState::new("demo".to_string(), dir.path().to_path_buf());
        let mut parser = StreamParser::new();

        state.begin_pass();
        std::fs::write(
            &sink,
            "{\"type\":\"item.completed\",\"item\":{\"id\":\"i1\",\"type\":\"agent_message\",\"text\":\"hi\"}}\n{\"type\":\"turn.completed\"",
        )
        .expect("write sink");

        let offset = drain_sink(&sink, 0, &mut parser, &state);
        // The complete first line produced an in-progress turn; the partial
        // second line is held (offset stops before it).
        assert!(offset > 0);
        assert_eq!(state.turn_count(), 1);

        // Complete the partial line; the next drain finalizes the turn.
        std::fs::write(
            &sink,
            "{\"type\":\"item.completed\",\"item\":{\"id\":\"i1\",\"type\":\"agent_message\",\"text\":\"hi\"}}\n{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}\n",
        )
        .expect("rewrite sink");
        let _ = drain_sink(&sink, offset, &mut parser, &state);
        assert_eq!(state.turn_count(), 1);
    }
}
