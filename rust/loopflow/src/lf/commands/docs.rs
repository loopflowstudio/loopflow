use std::io::{self, IsTerminal, Write};

use anyhow::{bail, Result};
use loopflow_definitions::builtins::{builtin_docs, get_builtin_doc};

pub fn show(topic: Option<&str>, json: bool) -> Result<()> {
    let output = match topic {
        None => render_index(json)?,
        Some(topic) => render_topic(topic, json)?,
    };
    let stdout = io::stdout();
    let output = if stdout.is_terminal() {
        output.replace('\n', "\r\n")
    } else {
        output
    };
    stdout.lock().write_all(output.as_bytes())?;
    Ok(())
}

fn render_index(json: bool) -> Result<String> {
    let docs = builtin_docs();
    if json {
        let rows: Vec<_> = docs
            .iter()
            .map(|doc| {
                serde_json::json!({
                    "topic": doc.slug,
                    "title": doc.title,
                    "bytes": doc.content.len(),
                })
            })
            .collect();
        return Ok(format!("{}\n", serde_json::to_string_pretty(&rows)?));
    }
    let mut output = String::from("Embedded Loopflow docs:\n\n");
    for doc in docs {
        output.push_str(&format!("  {:<20} {}\n", doc.slug, doc.title));
    }
    output.push_str("\nRun `lf docs <topic>` to read a page, or `lf help --all` for commands.\n");
    Ok(output)
}

fn render_topic(topic: &str, json: bool) -> Result<String> {
    let Some(doc) = get_builtin_doc(topic) else {
        let available = builtin_docs()
            .iter()
            .map(|doc| doc.slug)
            .collect::<Vec<_>>()
            .join(", ");
        bail!("unknown doc topic {topic:?}; available topics: {available}");
    };
    if json {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&serde_json::json!({
                "topic": doc.slug,
                "title": doc.title,
                "content": doc.content,
            }))?
        ));
    }
    if doc.content.ends_with('\n') {
        Ok(doc.content.to_string())
    } else {
        Ok(format!("{}\n", doc.content))
    }
}
