//! Read-only dump of the actual Clap tree, including hidden commands and aliases.
use clap::Command;
use serde_json::{json, Value};

fn collect(command: &Command, parents: &[String], rows: &mut Vec<Value>) {
    let mut path = parents.to_vec();
    path.push(command.get_name().to_string());
    rows.push(json!({
        "kind": "command",
        "path": path,
        "about": command.get_about().map(ToString::to_string),
        "hidden": command.is_hide_set(),
        "aliases": command.get_all_aliases().collect::<Vec<_>>(),
    }));
    for argument in command.get_arguments() {
        rows.push(json!({
            "kind": "argument",
            "path": path,
            "id": argument.get_id().as_str(),
            "long": argument.get_long(),
            "short": argument.get_short(),
            "long_aliases": argument.get_all_aliases().unwrap_or_default(),
            "short_aliases": argument.get_all_short_aliases().unwrap_or_default(),
            "help": argument.get_help().map(ToString::to_string),
            "hidden": argument.is_hide_set(),
            "global": argument.is_global_set(),
            "required": argument.is_required_set(),
            "action": format!("{:?}", argument.get_action()),
            "defaults": argument.get_default_values().iter().map(|v| v.to_string_lossy()).collect::<Vec<_>>(),
        }));
    }
    for child in command.get_subcommands() {
        collect(child, &path, rows);
    }
}

fn main() {
    let mut rows = Vec::new();
    collect(&loopflow::lf::navigation::command_tree(), &[], &mut rows);
    println!(
        "{}",
        serde_json::to_string_pretty(&rows).expect("catalog is JSON")
    );
}
