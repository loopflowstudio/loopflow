//! `docs/lf-reference.md` renders the live Clap tree below its authored preamble,
//! so a pull request that changes the public CLI also changes this file.
//! Run with `UPDATE_CLI_REFERENCE=1` to rewrite it.
use std::path::Path;

use serde_json::Value;

fn cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn render(command: &Value, lines: &mut Vec<String>) {
    let path = command["command"].as_str().unwrap();
    lines.push(format!("## {path}"));
    lines.push(String::new());
    lines.push(
        command["description"]
            .as_str()
            .unwrap_or("Command namespace.")
            .to_owned(),
    );
    lines.push(String::new());
    let arguments: Vec<&Value> = command["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| path == "lf" || item["global"] != true)
        .collect();
    if !arguments.is_empty() {
        lines.push("| Argument | What it does |".into());
        lines.push("|---|---|".into());
        for item in arguments {
            let (long, short) = (item["long"].as_str(), item["short"].as_str());
            let mut label = match (long, short) {
                (Some(long), _) => format!("--{long}"),
                (None, Some(short)) => format!("-{short}"),
                (None, None) => format!("<{}>", item["name"].as_str().unwrap()),
            };
            if let (Some(_), Some(short)) = (long, short) {
                label += &format!(" / -{short}");
            }
            let mut detail = item["description"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| item["name"].as_str().unwrap().replace('_', " "));
            if item["global"] == true {
                detail += " Applies everywhere.";
            }
            let defaults: Vec<&str> = item["defaults"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect();
            if !defaults.is_empty() {
                detail += &format!(" Default: {}.", defaults.join(", "));
            }
            lines.push(format!("| `{}` | {} |", cell(&label), cell(&detail)));
        }
        lines.push(String::new());
    }
    for child in command["commands"].as_array().unwrap() {
        render(child, lines);
    }
}

#[test]
fn reference_matches_the_compiled_cli() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/lf-reference.md");
    let current = std::fs::read_to_string(&path).unwrap();
    let preamble = &current[..current
        .find("\n## lf\n")
        .expect("reference has the lf root")
        + 1];
    let catalog = loopflow::lf::navigation::describe_command(&[], true).unwrap();
    let mut lines = vec![preamble.trim_end_matches('\n').to_owned(), String::new()];
    render(&catalog["definition"], &mut lines);
    let expected = lines.join("\n");
    if std::env::var_os("UPDATE_CLI_REFERENCE").is_some() {
        std::fs::write(&path, &expected).unwrap();
    } else {
        assert!(
            current == expected,
            "docs/lf-reference.md is stale; rerun with UPDATE_CLI_REFERENCE=1"
        );
    }
}
