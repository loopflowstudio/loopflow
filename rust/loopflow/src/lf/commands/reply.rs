//! `lf reply <wave> [message]` — the L2 chat-reply capability as a one-shot
//! command. Observes the given message(s) for the wave and prints a reply *only
//! if one is warranted* (a channel may be humans talking to each other). No
//! listener, no resident, no governance loop — a direct capability an operator
//! can invoke, and the same one an independent Discord responder composes.

use anyhow::Result;

use crate::lf::commands::util::{find_repo_root, message_text};

pub fn run(
    wave: &str,
    text_args: &[String],
    agent: Option<String>,
    max_turns: Option<u32>,
) -> Result<()> {
    let repo = find_repo_root()?;
    let conversation = message_text(text_args, std::io::stdin())?;
    let rt = tokio::runtime::Runtime::new()?;
    // A standalone reply reads the wave's identity from the canonical repo, so
    // origin and resident are the same worktree here.
    let reply = rt.block_on(crate::controller::wave::chat_reply::reply(
        &repo,
        &repo,
        wave,
        &conversation,
        agent,
        max_turns,
    ))?;
    match reply {
        Some(reply) => println!("{reply}"),
        None => eprintln!("(no reply — nothing warranted)"),
    }
    Ok(())
}
