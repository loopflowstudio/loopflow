//! `lf wavechat` — one terminal pane that both monitors and steers a wave.
//!
//! The thread surface has two halves: `lf chat` publishes a `say` op through
//! the wave's door, and [`thread::follow`] reads its `/events` stream. Each is
//! a one-way verb, and each is right on its own.
//!
//! A human steering a wave wants both at once, in one pane: the wave's turns,
//! state transitions, and memory scroll past while a typed line goes into the
//! thread. That is this command, and it is composition rather than plumbing —
//! it reuses [`chat::resolve_target`] for targeting and endpoint discovery,
//! [`thread::follow`] for the stream (which replays on connect and reconnects
//! across server restarts), and [`chat::post_json`] for the `say` op.
//!
//! Slash commands are the steering verbs that are not speech. Everything else
//! typed is spoken into the thread.

use std::io::BufRead;

use anyhow::Result;

use crate::lf::commands::chat::{
    post_json, resolve_target, sender_attribution, CliContext, ResolvedWave,
};
use crate::lf::commands::thread;
use crate::lf::WaveTargetArgs;
use crate::wave::journal::Attribution;

pub fn run(wave: Option<&str>, from_label: Option<&str>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        let target = WaveTargetArgs {
            wave: wave.map(str::to_string),
            parent: false,
        };
        let Some(resolved) = resolve_target(
            &target,
            context.store.as_ref(),
            context.repo.as_deref(),
            context.env_wave_id.as_deref(),
            context.env_channel.as_deref(),
        )
        .await?
        else {
            // `lf chat` drops here (publish to no subscriber). A reader has
            // nothing to read, so say so and exit non-zero.
            anyhow::bail!("no wave here — name one with `lf wavechat <wave>`");
        };

        let mut from = sender_attribution(false, resolved.own_name.as_deref());
        if let Some(label) = from_label {
            from.label = label.to_string();
        }
        steer(&resolved, from, target.wave.clone()).await
    })
}

async fn steer(resolved: &ResolvedWave, from: Attribution, wave_arg: Option<String>) -> Result<()> {
    let endpoint = resolved.require_endpoint()?;
    println!(
        "wavechat: {} @ {endpoint}   (/help, Ctrl-D to leave)",
        resolved.name
    );

    // The stream replays on connect, so the session opens with the thread's
    // recent history rather than an empty screen.
    let stream = tokio::spawn(async move { thread::follow(wave_arg.as_deref(), false).await });

    // stdin blocks, so it reads on its own thread and hands lines to the loop.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(16);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.blocking_send(line).is_err() {
                break;
            }
        }
    });

    loop {
        let line = tokio::select! {
            line = rx.recv() => line,
            _ = tokio::signal::ctrl_c() => None,
        };
        // EOF (Ctrl-D) or Ctrl-C: leave. The wave keeps running.
        let Some(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(command) = line.strip_prefix('/') {
            if !handle_command(command, &endpoint).await? {
                break;
            }
            continue;
        }
        say(&endpoint, line, &from).await?;
    }

    stream.abort();
    Ok(())
}

/// A steering verb that is not speech.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Quit,
    Status,
    Help,
    Unknown,
}

fn parse_command(command: &str) -> Command {
    match command.trim() {
        "q" | "quit" | "exit" => Command::Quit,
        "status" => Command::Status,
        "help" | "?" => Command::Help,
        _ => Command::Unknown,
    }
}

/// Returns false when the session should end.
async fn handle_command(command: &str, endpoint: &str) -> Result<bool> {
    match parse_command(command) {
        Command::Quit => return Ok(false),
        Command::Status => print_status(endpoint).await?,
        Command::Help => println!(
            "  /status   the wave's loop state\n  \
               /quit     leave (Ctrl-D also works)\n  \
             anything else is spoken into the thread"
        ),
        Command::Unknown => eprintln!("unknown command '/{}' — try /help", command.trim()),
    }
    Ok(true)
}

/// The wave's own view of itself, straight off its health door.
async fn print_status(endpoint: &str) -> Result<()> {
    let health: serde_json::Value = reqwest::Client::new()
        .get(format!("http://{endpoint}/health"))
        .send()
        .await?
        .json()
        .await?;
    println!("{}", serde_json::to_string_pretty(&health)?);
    Ok(())
}

/// Speak into the thread — the same `say` op `lf chat` posts.
async fn say(endpoint: &str, text: &str, from: &Attribution) -> Result<()> {
    let body = serde_json::json!({ "op": "say", "text": text, "from": from });
    post_json(endpoint, "/messages", &body).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_command, Command};

    #[test]
    fn slash_commands_parse_and_everything_else_is_speech() {
        assert_eq!(parse_command("quit"), Command::Quit);
        assert_eq!(parse_command(" q "), Command::Quit);
        assert_eq!(parse_command("exit"), Command::Quit);
        assert_eq!(parse_command("status"), Command::Status);
        assert_eq!(parse_command("help"), Command::Help);
        assert_eq!(parse_command("?"), Command::Help);
        // A bare word is speech, not a mistyped command.
        assert_eq!(parse_command("deploy"), Command::Unknown);
    }
}
