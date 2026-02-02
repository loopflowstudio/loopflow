use crate::commands::util::find_repo_root;
use crate::discovery::{
    builtin_descriptions, builtin_steps, is_step_interactive, list_all_steps,
    list_flows_with_steps, BUILTIN_CATEGORIES,
};
use crate::output::Colors;
use anyhow::Result;
use std::collections::{HashMap, HashSet};

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

    let descriptions = builtin_descriptions();
    let builtins = builtin_steps();

    // =========================================================================
    // FLOWS section
    // =========================================================================
    if let Some(ref repo) = repo_root {
        let flows = list_flows_with_steps(repo);
        if !flows.is_empty() {
            println!(
                "{cyan}{bold}FLOWS{reset}",
                cyan = colors.cyan,
                bold = colors.bold,
                reset = colors.reset
            );
            for flow in flows {
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
                    "  {bold}{name:<14}{reset} {dim}{chain}{reset}",
                    bold = colors.bold,
                    name = flow.name,
                    reset = colors.reset,
                    dim = colors.dim,
                    chain = chain
                );
            }
            println!();
        }
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

    // Built-ins by category
    for (category, step_names) in BUILTIN_CATEGORIES {
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
            let desc = descriptions.get(name).copied().unwrap_or("");
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
                "  {bold}{name:<14}{reset} {dim}{desc:<34}{reset}{badge}{custom_tag}",
                bold = colors.bold,
                name = name,
                reset = colors.reset,
                dim = colors.dim,
                desc = desc,
                badge = badge,
                custom_tag = custom_tag
            );
        }
        println!();
    }

    // =========================================================================
    // Custom steps (user-defined, not overriding builtins)
    // =========================================================================
    let custom: Vec<_> = user_steps
        .iter()
        .filter(|t| !builtins.contains(*t))
        .collect();
    if !custom.is_empty() {
        println!(
            "{green}Custom{reset}",
            green = colors.green,
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
                "  {bold}{name:<14}{reset}{badge}",
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
            "{green}Global{reset}",
            green = colors.green,
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
                "  {bold}{name:<14}{reset}{badge}",
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
        // Group by source
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
                    "  {bold}{name:<20}{reset}",
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
        "{dim}Built-ins work anywhere. Run lf <step> or lf <step>: args{reset}",
        dim = colors.dim,
        reset = colors.reset
    );

    Ok(())
}
