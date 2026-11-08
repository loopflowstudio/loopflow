//! The terminal face of the same Task reading used by Desktop. Navigation never
//! launches a Graph, prepares a conversation, or takes a viewer's input claim.
use std::io::{IsTerminal, Read, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use super::{subscribe, waves};

pub fn run(repo: &Path, issue: Option<&str>, once: bool) -> Result<()> {
    let status = crate::ops::task::task_status(repo, issue)?;
    let issue = status
        .task
        .as_ref()
        .context("Task is unavailable")?
        .identifier
        .clone();
    let mut reading = serde_json::to_value(&status)?;
    if once || !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        println!("{}", render(&reading));
        return Ok(());
    }
    let subscribing = waves::subscribe_task_status(
        repo,
        issue,
        subscribe::Transport::Pull {
            wake: Box::new(|| {}),
            select: None,
        },
    )?
    .context("Task subscription did not return its handle")?;
    let _terminal = Terminal::enter()?;
    let mut offset = 0usize;
    let mut dirty = true;
    let mut unavailable = None;
    let mut selected_step = 0usize;
    let mut log = None;
    loop {
        if let Some(bytes) = subscribing.pull() {
            let frame: Value = serde_json::from_slice(&bytes)?;
            unavailable = frame["unavailable"].as_str().map(str::to_owned);
            if !frame["body"].is_null() {
                reading = frame["body"].clone();
            }
            dirty = true;
        }
        if dirty {
            let text = log.clone().unwrap_or_else(|| render(&reading));
            let lines: Vec<_> = text.lines().collect();
            offset = offset.min(lines.len().saturating_sub(1));
            print!("\x1b[H\x1b[2J");
            if let Some(error) = &unavailable {
                println!("Reading unavailable: {}\r", plain(error));
            }
            for line in lines.iter().skip(offset).take(height().saturating_sub(3)) {
                println!("{line}\r");
            }
            print!(
                "\r\nj/k scroll · n/p step {} · l log · Esc overview · q close\r\n",
                selected_step + 1
            );
            std::io::stdout().flush()?;
            dirty = false;
        }
        let mut fd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: fd is one valid poll descriptor; no pointers outlive this call.
        if unsafe { libc::poll(&mut fd, 1, 100) } > 0 && fd.revents != 0 {
            let mut byte = [0];
            if std::io::stdin().read(&mut byte)? == 0 {
                break;
            }
            match byte[0] {
                b'q' | 3 | 29 => break,
                b'j' | b' ' => offset = offset.saturating_add(1),
                b'k' => offset = offset.saturating_sub(1),
                b'g' => offset = 0,
                b'n' => {
                    selected_step =
                        (selected_step + 1).min(steps(&reading).count().saturating_sub(1));
                    log = None;
                }
                b'p' => {
                    selected_step = selected_step.saturating_sub(1);
                    log = None;
                }
                b'l' => {
                    log = Some(
                        step_log(&reading, selected_step)
                            .unwrap_or_else(|error| format!("Step log unavailable: {error:#}")),
                    );
                    offset = 0;
                }
                27 => {
                    log = None;
                    offset = 0;
                }
                _ => continue,
            }
            dirty = true;
        }
        if subscribing.ended() {
            break;
        }
    }
    Ok(())
}

fn steps(reading: &Value) -> impl Iterator<Item = &Value> {
    reading["graph_processes"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|graph| graph["steps"].as_array().into_iter().flatten())
}

fn step_log(reading: &Value, index: usize) -> Result<String> {
    let step = steps(reading).nth(index).context("No step selected")?;
    let id = crate::id::LfProcessId::parse(word(step, "lf_process_id"))?;
    let store = crate::store::read_existing_registry()?.context("Work registry is unavailable")?;
    let process = store
        .sqlite
        .lf_process(&id)?
        .context("Step Process is unavailable")?;
    let mut text = format!(
        "{} · {}\n{}\nOutcome: {} · exit {:?}\n",
        word(step, "label"),
        id,
        process.command.as_deref().unwrap_or("command unavailable"),
        process.outcome.as_deref().unwrap_or("running or unknown"),
        process.exit_code
    );
    if let Some((session, input, _)) = store.sqlite.process_input(&id)? {
        text.push_str(&format!("\nSession {session}\nCaptured input {input}\n\n"));
        for envelope in store.sqlite.input_events(&input)? {
            let event = &envelope["event"];
            match event["type"].as_str() {
                Some("text_delta") => text.push_str(event["content"].as_str().unwrap_or_default()),
                Some("item_completed") if event["item"]["type"] != "message" => {
                    text.push_str(&format!(
                        "\n{}\n",
                        serde_json::to_string_pretty(&event["item"])?
                    ));
                }
                Some("error") => text.push_str(&format!(
                    "\n{}: {}\n",
                    word(event, "code"),
                    word(event, "message")
                )),
                _ => {}
            }
        }
        if let Some(answer) = store.sqlite.input_final_answer(&input)? {
            text.push_str(&format!("\nRecorded conclusion\n{answer}\n"));
        }
    } else {
        text.push_str(
            "\nThis operation captured no conversation log. Its result is shown above.\n",
        );
    }
    Ok(plain(&text))
}

fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

fn word<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("unknown")
}

fn render(reading: &Value) -> String {
    let task = &reading["task"];
    let mut text = format!(
        "{} · {}\n\n{}\n",
        word(task, "identifier"),
        word(task, "title"),
        word(task, "description")
    );
    if let Some(error) = reading["planning_error"].as_str() {
        text.push_str(&format!("\nPlanning unavailable: {error}\n"));
    }
    let execution = &reading["execution"];
    text.push_str(&format!(
        "\nTask: {}\nCheckout: {}\n",
        word(execution, "status"),
        word(execution, "worktree")
    ));
    if let Some(url) = execution
        .pointer("/pr/publication/github/url")
        .and_then(Value::as_str)
    {
        text.push_str(&format!("PR: {url}\n"));
    }
    let graph = &reading["overview"]["graph"];
    if graph.is_object() {
        text.push_str(&format!(
            "\nWorkflow: {} · {} ({})\n",
            word(graph, "name"),
            word(&graph["position"], "step"),
            word(&graph["position"], "kind")
        ));
    } else {
        text.push_str("\nNo workflow taken up.\n");
        text.push_str(&format!(
            "Start explicitly: lf task run {}\n",
            word(task, "identifier")
        ));
    }
    text.push_str("\nConversations\n");
    if let Some(sessions) = reading["overview"]["sessions"].as_array() {
        for session in sessions {
            text.push_str(&format!(
                "  {} · {}\n    lf resume {}\n",
                word(session, "title"),
                word(session, "id"),
                word(session, "id")
            ));
        }
    }
    text.push_str("\nAssociated Graph processes (each is independent of workflow position)\n");
    let mut step_number = 0;
    if let Some(graphs) = reading["graph_processes"].as_array() {
        for graph in graphs {
            let entry = &graph["entry"];
            text.push_str(&format!(
                "  {} · {} · {}\n",
                word(entry, "name"),
                word(entry, "state"),
                word(entry, "id")
            ));
            if let Some(steps) = graph["steps"].as_array() {
                for step in steps {
                    step_number += 1;
                    text.push_str(&format!(
                        "    [{step_number}] {} · {} · exit {}\n      lf session history show {}\n",
                        word(step, "label"),
                        word(step, "outcome"),
                        step["exit_code"],
                        word(step, "lf_process_id")
                    ));
                }
            }
        }
    }
    plain(&text)
}

fn height() -> usize {
    // SAFETY: winsize is an all-zero-valid C output structure.
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: ioctl receives a valid winsize output pointer for the terminal fd.
    if unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut size) } == 0 && size.ws_row > 3 {
        size.ws_row as usize
    } else {
        24
    }
}

struct Terminal(libc::termios);
impl Terminal {
    fn enter() -> Result<Self> {
        // SAFETY: termios is a plain C structure and tcgetattr initializes it.
        let mut saved: libc::termios = unsafe { std::mem::zeroed() };
        anyhow::ensure!(
            unsafe { libc::tcgetattr(0, &mut saved) } == 0,
            "read terminal mode"
        );
        let mut raw = saved;
        // SAFETY: raw is initialized; descriptor 0 is a terminal checked by the caller.
        unsafe {
            libc::cfmakeraw(&mut raw);
        }
        anyhow::ensure!(
            unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw) } == 0,
            "set terminal mode"
        );
        let terminal = Self(saved);
        print!("\x1b[?1049h\x1b[?25l");
        std::io::stdout().flush()?;
        Ok(terminal)
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        // SAFETY: saved contains the original settings for descriptor 0.
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.0);
        }
        let _ = std::io::stdout().write_all(b"\x1b[?25h\x1b[?1049l");
        let _ = std::io::stdout().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::render;
    use serde_json::json;

    #[test]
    fn task_view_keeps_workflow_position_distinct_from_associated_runs() {
        let text = render(&json!({
            "task": {"identifier":"LOO-1","title":"Workspace","description":"Keep existing work\u{1b}[2J"},
            "execution":{"status":"active","worktree":"/task"},
            "overview":{"graph":{"name":"feature","position":{"kind":"at","step":"demo"}},"sessions":[]},
            "graph_processes":[{"entry":{"id":"run-a","name":"debug","state":"stopped"},"steps":[{"label":"repair","outcome":"failed","exit_code":1,"lf_process_id":"step-a"}]}]
        }));
        assert!(text.contains("Workflow: feature · demo (at)"));
        assert!(text.contains("debug · stopped · run-a"));
        assert!(text.contains("repair · failed · exit 1"));
        assert!(text.contains("lf session history show step-a"));
        assert!(!text.contains('\u{1b}'));
    }
}
