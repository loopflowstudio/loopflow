use std::path::Path;

use serde_json::Value;

use crate::lf::output::{column_width, truncate, Colors};

#[derive(Debug, clap::Args)]
pub struct ShowOptions {
    /// Include settings supplied only by built-in defaults
    #[arg(long)]
    pub all: bool,
    /// Show every candidate, its file, and why it won or lost
    #[arg(long)]
    pub explain: bool,
    /// Emit complete structured output, including defaults and candidates
    #[arg(long, conflicts_with_all = ["all", "explain"])]
    pub json: bool,
}

pub fn retained(replace: bool, keep: bool) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&crate::config::remote::inspect(replace, keep)?)?
    );
    Ok(())
}

pub fn print_user(json: bool) -> anyhow::Result<()> {
    let name = crate::config::participant_name()?;
    if json {
        println!("{}", serde_json::to_string(&name)?);
    } else if let Some(name) = name {
        println!("{name}");
    }
    Ok(())
}

pub fn show(
    repo: &std::path::Path,
    wave: Option<&str>,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
    options: &ShowOptions,
) -> anyhow::Result<()> {
    let output = crate::config::api::inspect(repo, wave, skill, graph, step)?;
    if options.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        print!("{}", render(&output, repo, options, &Colors::new()));
    }
    Ok(())
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn display_value(value: &Value) -> String {
    match value.as_str() {
        Some(value) if !value.chars().any(char::is_control) => value.to_owned(),
        _ => value.to_string(),
    }
}

fn target(candidate: &Value) -> String {
    let target = &candidate["target"];
    match text(&target["kind"]) {
        "skill" => format!("skill {}", text(&target["name"])),
        "graph_occurrence" => format!(
            "{}/{}",
            text(&target["graph"]),
            target["id"].as_str().unwrap_or("unnamed step")
        ),
        _ => String::new(),
    }
}

fn source(candidate: &Value) -> String {
    let owner = match (text(&candidate["source"]), text(&candidate["scope"])) {
        ("personal", "global") => "Personal global",
        ("personal", "repository") => "Personal project",
        ("personal", "wave") => "Personal Wave",
        ("repository", "wave") => "Shared Wave",
        ("repository", _) => "Shared project",
        ("command_line", _) => "Command line",
        ("environment", _) => "Environment",
        ("builtin", _) => "Built-in",
        (other, _) => other,
    };
    let target = target(candidate);
    if target.is_empty() {
        owner.to_owned()
    } else {
        format!("{owner} · {target}")
    }
}

fn contributes(candidate: &Value) -> bool {
    matches!(
        text(&candidate["reason"]),
        "selected" | "contributes" | "clears earlier contributions"
    )
}

fn effective_source(candidates: &[Value]) -> String {
    let mut sources = Vec::new();
    for candidate in candidates.iter().filter(|c| contributes(c)) {
        let label = source(candidate);
        if !sources.contains(&label) {
            sources.push(label);
        }
    }
    if sources.is_empty() {
        if candidates
            .iter()
            .any(|c| text(&c["reason"]).starts_with("inactive:"))
        {
            "Inactive".into()
        } else {
            "Not selected".into()
        }
    } else {
        sources.join(" + ")
    }
}

fn short_path(locator: &str, repo: &Path) -> String {
    let path = Path::new(locator);
    // The locator may use a canonical spelling, e.g. /private/var on macOS.
    let repo = repo.canonicalize().unwrap_or_else(|_| repo.to_owned());
    if let Ok(relative) = path.strip_prefix(&repo) {
        return relative.display().to_string();
    }
    if let Ok(relative) = path.strip_prefix(crate::store::lf_home_dir()) {
        return format!("$LF_HOME/{}", relative.display());
    }
    if let Some(home) = dirs::home_dir() {
        if let Ok(relative) = path.strip_prefix(home) {
            return format!("~/{}", relative.display());
        }
    }
    locator.to_owned()
}

fn render(output: &[Value], repo: &Path, options: &ShowOptions, colors: &Colors) -> String {
    let mut out = String::new();
    for item in output {
        let skill = item["skill"].as_str().unwrap_or("Configuration");
        let occurrence = &item["occurrence"];
        let title = match occurrence["graph"].as_str() {
            Some(graph) => format!(
                "{graph} / {} · {skill}",
                occurrence["id"].as_str().unwrap_or("unnamed step")
            ),
            None => skill.to_owned(),
        };
        out.push_str(&format!("\n{}{}{}\n\n", colors.bold, title, colors.reset));
        let Some(settings) = item["settings"].as_object() else {
            continue;
        };
        let visible: Vec<_> = settings
            .iter()
            .filter(|(key, setting)| {
                options.all
                    || key.as_str() == "agent"
                    || setting["candidates"].as_array().is_some_and(|candidates| {
                        candidates.iter().any(|c| c["source"] != "builtin")
                    })
            })
            .collect();
        let rows: Vec<_> = visible
            .iter()
            .map(|(key, setting)| {
                let candidates = setting["candidates"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                (
                    key.as_str(),
                    truncate(&display_value(&setting["value"]), 36),
                    effective_source(candidates),
                )
            })
            .collect();
        let key_width = column_width("SETTING", rows.iter().map(|row| row.0));
        let value_width = column_width("VALUE", rows.iter().map(|row| &row.1));
        out.push_str(&format!(
            "  {}{:<key_width$}  {:<value_width$}  FROM{}\n",
            colors.dim, "SETTING", "VALUE", colors.reset
        ));
        for (key, value, from) in &rows {
            out.push_str(&format!(
                "  {}{key:<key_width$}{}  {}{value:<value_width$}{}  {from}\n",
                colors.cyan, colors.reset, colors.green, colors.reset
            ));
        }
        let hidden = settings.len() - visible.len();
        if hidden > 0 {
            out.push_str(&format!(
                "\n  {}{hidden} built-in settings hidden · --all to include them{}\n",
                colors.dim, colors.reset
            ));
        }
        if options.explain {
            for (key, setting) in &visible {
                out.push_str(&format!(
                    "\n  {}{key}{} = {}\n",
                    colors.bold,
                    colors.reset,
                    display_value(&setting["value"])
                ));
                if let Some(candidates) = setting["candidates"].as_array() {
                    for candidate in candidates.iter().rev() {
                        let reason = text(&candidate["reason"]);
                        let color = if contributes(candidate) {
                            colors.green
                        } else {
                            colors.dim
                        };
                        out.push_str(&format!(
                            "    {color}{reason}{}\n      {} = {} · {}\n      {}{}{}\n",
                            colors.reset,
                            text(&candidate["key"]),
                            display_value(&candidate["value"]),
                            source(candidate),
                            colors.dim,
                            short_path(text(&candidate["locator"]), repo),
                            colors.reset
                        ));
                    }
                }
            }
        } else {
            out.push_str(&format!(
                "  {}--explain shows files and why each value won{}\n",
                colors.dim, colors.reset
            ));
        }
        out.push('\n');
    }
    out
}

/// The catalogue is available before a store or provider is installed.
pub fn catalog(json: bool) -> anyhow::Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(crate::config::catalog::DEFINITIONS)?
        );
    } else {
        for definition in crate::config::catalog::DEFINITIONS {
            println!(
                "{}: {}{}",
                definition.key,
                definition.description,
                if definition.lifecycle == crate::config::catalog::Lifecycle::Inactive {
                    " (inactive)"
                } else {
                    ""
                }
            );
            println!(
                "  {:?} · {:?} · {:?}",
                definition.applicability, definition.combination, definition.portability
            );
        }
    }
    Ok(())
}
