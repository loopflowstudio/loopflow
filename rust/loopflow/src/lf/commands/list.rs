use crate::lf::commands::util::find_repo_root;
use crate::lf::discovery::{
    builtin_flow_descriptions, builtin_flows, builtin_step_description, builtin_steps,
    is_step_interactive, list_all_steps, list_user_flows, BUILTIN_FLOW_CATEGORIES,
    BUILTIN_STEP_CATEGORIES,
};

use crate::lf::output::Colors;
use anyhow::Result;
use std::collections::{HashMap, HashSet};

const DESC_WIDTH: usize = 48;

pub fn show_all() -> Result<()> {
    let repo_root = find_repo_root().ok();
    let colors = Colors::default();

    let (user_steps, global_steps, builtin_only, external_skills) =
        list_all_steps(repo_root.as_deref());
    let user_step_set: HashSet<_> = user_steps.iter().cloned().collect();
    let all_known_steps: HashSet<_> = user_steps
        .iter()
        .chain(global_steps.iter())
        .chain(builtin_only.iter())
        .cloned()
        .collect();

    let builtins = builtin_steps();

    // =========================================================================
    // FLOWS section
    // =========================================================================
    let flow_descriptions = builtin_flow_descriptions();
    let builtin_flow_set = builtin_flows();

    let user_flows: Vec<_> = repo_root
        .as_ref()
        .map(|r| list_user_flows(r))
        .unwrap_or_default();
    let user_flow_set: HashSet<String> = user_flows.iter().map(|f| f.name.clone()).collect();

    println!(
        "{cyan}{bold}FLOWS{reset}",
        cyan = colors.cyan,
        bold = colors.bold,
        reset = colors.reset
    );
    println!();

    for (category, flow_names) in BUILTIN_FLOW_CATEGORIES {
        println!(
            "{dim}{category}{reset}",
            dim = colors.dim,
            category = category,
            reset = colors.reset
        );

        for name in *flow_names {
            let desc = flow_descriptions
                .get(*name)
                .map(|s| s.as_str())
                .unwrap_or("");
            let custom_tag = if user_flow_set.contains(*name) {
                format!(
                    " {dim}(customized){reset}",
                    dim = colors.dim,
                    reset = colors.reset
                )
            } else {
                String::new()
            };
            println!(
                "  {bold}{name:<20}{reset} {dim}{desc}{reset}{custom_tag}",
                bold = colors.bold,
                name = name,
                reset = colors.reset,
                dim = colors.dim,
                desc = desc,
                custom_tag = custom_tag
            );
        }
        println!();
    }

    // User-defined flows not overriding builtins
    let custom_flows: Vec<_> = user_flows
        .iter()
        .filter(|f| !builtin_flow_set.contains(&f.name))
        .collect();

    if !custom_flows.is_empty() {
        println!(
            "{green}{bold}User{reset}",
            green = colors.green,
            bold = colors.bold,
            reset = colors.reset
        );
        for flow in custom_flows {
            let chain = flow
                .step_names
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(&format!(
                    " {dim}→{reset} ",
                    dim = colors.dim,
                    reset = colors.reset
                ));
            println!(
                "  {bold}{name:<20}{reset} {dim}{chain}{reset}",
                bold = colors.bold,
                name = flow.name,
                reset = colors.reset,
                dim = colors.dim,
                chain = chain
            );
        }
        println!();
    }

    // =========================================================================
    // STEPS section
    // =========================================================================
    println!(
        "{cyan}{bold}STEPS{reset}",
        cyan = colors.cyan,
        bold = colors.bold,
        reset = colors.reset
    );
    println!();

    for (category, step_names) in BUILTIN_STEP_CATEGORIES {
        let category_steps: Vec<_> = step_names
            .iter()
            .filter(|t| all_known_steps.contains(**t))
            .collect();
        if category_steps.is_empty() {
            continue;
        }

        println!(
            "{dim}{category}{reset}",
            dim = colors.dim,
            category = category,
            reset = colors.reset
        );

        for name in category_steps {
            let desc = truncate(&builtin_step_description(name), DESC_WIDTH);
            let is_interactive = repo_root
                .as_ref()
                .is_some_and(|r| is_step_interactive(r, name));
            let badge = if is_interactive {
                format!(
                    "  {yellow}interactive{reset}",
                    yellow = colors.yellow,
                    reset = colors.reset
                )
            } else {
                String::new()
            };
            let custom_tag = if user_step_set.contains(*name) {
                format!(
                    " {dim}(customized){reset}",
                    dim = colors.dim,
                    reset = colors.reset
                )
            } else {
                String::new()
            };
            println!(
                "  {bold}{name:<26}{reset} {dim}{desc:<width$}{reset}{badge}{custom_tag}",
                bold = colors.bold,
                name = name,
                reset = colors.reset,
                dim = colors.dim,
                desc = desc,
                width = DESC_WIDTH,
                badge = badge,
                custom_tag = custom_tag
            );
        }
        println!();
    }

    // =========================================================================
    // User steps (repo-local, not overriding builtins)
    // =========================================================================
    let custom: Vec<_> = user_steps
        .iter()
        .filter(|t| !builtins.contains(*t))
        .collect();
    if !custom.is_empty() {
        println!(
            "{green}{bold}User{reset}",
            green = colors.green,
            bold = colors.bold,
            reset = colors.reset
        );
        for name in custom {
            let is_interactive = repo_root
                .as_ref()
                .is_some_and(|r| is_step_interactive(r, name));
            let badge = if is_interactive {
                format!(
                    "  {yellow}interactive{reset}",
                    yellow = colors.yellow,
                    reset = colors.reset
                )
            } else {
                String::new()
            };
            println!(
                "  {bold}{name:<26}{reset}{badge}",
                bold = colors.bold,
                name = name,
                reset = colors.reset,
                badge = badge
            );
        }
        println!();
    }

    // =========================================================================
    // Global steps
    // =========================================================================
    if !global_steps.is_empty() {
        println!(
            "{green}{bold}Global{reset}",
            green = colors.green,
            bold = colors.bold,
            reset = colors.reset
        );
        for name in &global_steps {
            let is_interactive = repo_root
                .as_ref()
                .is_some_and(|r| is_step_interactive(r, name));
            let badge = if is_interactive {
                format!(
                    "  {yellow}interactive{reset}",
                    yellow = colors.yellow,
                    reset = colors.reset
                )
            } else {
                String::new()
            };
            println!(
                "  {bold}{name:<26}{reset}{badge}",
                bold = colors.bold,
                name = name,
                reset = colors.reset,
                badge = badge
            );
        }
        println!();
    }

    // =========================================================================
    // EXTERNAL SKILLS section
    // =========================================================================
    if !external_skills.is_empty() {
        let mut by_source: HashMap<String, Vec<String>> = HashMap::new();
        for (prefixed_name, source_name) in external_skills {
            by_source
                .entry(source_name)
                .or_default()
                .push(prefixed_name);
        }

        println!(
            "{cyan}{bold}EXTERNAL SKILLS{reset}",
            cyan = colors.cyan,
            bold = colors.bold,
            reset = colors.reset
        );
        println!();

        let mut sources: Vec<_> = by_source.into_iter().collect();
        sources.sort_by(|a, b| a.0.cmp(&b.0));

        for (source_name, skill_names) in sources {
            println!(
                "{dim}{source_name}{reset}",
                dim = colors.dim,
                source_name = source_name,
                reset = colors.reset
            );
            for name in skill_names {
                println!(
                    "  {bold}{name}{reset}",
                    bold = colors.bold,
                    name = name,
                    reset = colors.reset
                );
            }
            println!();
        }
    }

    // =========================================================================
    // Footer
    // =========================================================================
    println!(
        "{dim}Run lf <step>, lf <flow>, or lf <namespace>/<step> [args]{reset}",
        dim = colors.dim,
        reset = colors.reset
    );

    Ok(())
}

/// Truncate `s` to fit within `width` graphemes (simple char-based approximation).
fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let truncated: String = s.chars().take(width.saturating_sub(1)).collect();
    format!("{truncated}…")
}
