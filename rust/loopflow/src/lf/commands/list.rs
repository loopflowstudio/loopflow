use crate::lf::commands::util::find_repo_root;
use crate::lf::discovery::{
    builtin_flow_infos, builtin_flows, builtin_skill_description, builtin_skills, list_all_skills,
    list_user_flows, FlowInfo, BUILTIN_FLOW_CATEGORIES, BUILTIN_SKILL_CATEGORIES,
};
use crate::lf::output::Colors;
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

const NAME_WIDTH: usize = 26;
const DESC_WIDTH: usize = 48;

#[derive(Debug, Default)]
struct CatalogEntry {
    flow: Option<FlowInfo>,
    skill_description: Option<String>,
    customized: bool,
}

pub fn show_all() -> Result<()> {
    let repo_root = find_repo_root().ok();
    print!("{}", render_all(repo_root.as_deref(), Colors::default()));
    Ok(())
}

fn render_all(repo_root: Option<&Path>, colors: Colors) -> String {
    let (user_skills, global_skills, _builtin_only, external_skills) = list_all_skills(repo_root);
    let user_skill_set: HashSet<_> = user_skills.iter().cloned().collect();
    let builtin_skill_set = builtin_skills();
    let builtin_flow_set = builtin_flows();
    let flow_infos = builtin_flow_infos(repo_root);
    let user_flows = repo_root.map(list_user_flows).unwrap_or_default();
    let user_flow_by_name: HashMap<_, _> = user_flows
        .iter()
        .map(|flow| (flow.name.clone(), flow.clone()))
        .collect();

    let mut output = String::new();
    writeln!(
        output,
        "{cyan}{bold}CATALOG{reset}\n",
        cyan = colors.cyan,
        bold = colors.bold,
        reset = colors.reset
    )
    .expect("write catalog heading");

    for category in builtin_category_order() {
        let mut entries = BTreeMap::new();

        for name in category_members(BUILTIN_FLOW_CATEGORIES, &category) {
            let custom_flow = user_flow_by_name.get(*name);
            let entry = entries
                .entry((*name).to_string())
                .or_insert_with(CatalogEntry::default);
            entry.flow = custom_flow
                .cloned()
                .or_else(|| flow_infos.get(*name).cloned());
            entry.customized = custom_flow.is_some();
        }

        for name in category_members(BUILTIN_SKILL_CATEGORIES, &category) {
            let entry = entries
                .entry((*name).to_string())
                .or_insert_with(CatalogEntry::default);
            entry.skill_description = Some(builtin_skill_description(name));
            entry.customized |= user_skill_set.contains(*name);
        }

        if !entries.is_empty() {
            render_category(&mut output, &category, &entries, colors);
        }
    }

    let mut user_entries = BTreeMap::new();
    for flow in user_flows
        .iter()
        .filter(|flow| !builtin_flow_set.contains(&flow.name))
    {
        user_entries
            .entry(flow.name.clone())
            .or_insert_with(CatalogEntry::default)
            .flow = Some(flow.clone());
    }
    for name in user_skills
        .iter()
        .filter(|name| !builtin_skill_set.contains(*name))
    {
        let entry = user_entries
            .entry(name.clone())
            .or_insert_with(CatalogEntry::default);
        entry.skill_description = Some(String::new());
    }
    if !user_entries.is_empty() {
        render_category(&mut output, "User", &user_entries, colors);
    }

    if !global_skills.is_empty() {
        let entries = skill_only_entries(&global_skills);
        render_category(&mut output, "Global", &entries, colors);
    }

    let mut external_by_source: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, source) in external_skills {
        external_by_source.entry(source).or_default().push(name);
    }
    for (source, mut skills) in external_by_source {
        skills.sort();
        let entries = skill_only_entries(&skills);
        render_category(&mut output, &source, &entries, colors);
    }

    writeln!(
        output,
        "{dim}Run lf <name>, lf flow <name>, or lf skill <name> [args]{reset}",
        dim = colors.dim,
        reset = colors.reset
    )
    .expect("write catalog footer");
    output
}

fn builtin_category_order() -> Vec<String> {
    let mut categories: BTreeSet<String> = BUILTIN_FLOW_CATEGORIES
        .iter()
        .chain(BUILTIN_SKILL_CATEGORIES.iter())
        .map(|(category, _)| (*category).to_string())
        .collect();
    let mut ordered = Vec::new();
    for core in ["Task", "Project", "Wave", "Ops"] {
        if categories.remove(core) {
            ordered.push(core.to_string());
        }
    }
    ordered.extend(categories);
    ordered
}

fn category_members<'a>(categories: &'a [(&str, &[&str])], category: &str) -> &'a [&'a str] {
    categories
        .iter()
        .find_map(|(name, members)| (*name == category).then_some(*members))
        .unwrap_or(&[])
}

fn skill_only_entries(skills: &[String]) -> BTreeMap<String, CatalogEntry> {
    skills
        .iter()
        .map(|name| {
            (
                name.clone(),
                CatalogEntry {
                    skill_description: Some(String::new()),
                    ..CatalogEntry::default()
                },
            )
        })
        .collect()
}

fn render_category(
    output: &mut String,
    category: &str,
    entries: &BTreeMap<String, CatalogEntry>,
    colors: Colors,
) {
    writeln!(
        output,
        "{dim}{category}{reset}",
        dim = colors.dim,
        reset = colors.reset
    )
    .expect("write catalog category");

    for (name, entry) in entries {
        let kind = match (entry.flow.is_some(), entry.skill_description.is_some()) {
            (true, true) => "flow+skill",
            (true, false) => "flow",
            (false, true) => "skill",
            (false, false) => continue,
        };
        let description = entry
            .skill_description
            .as_deref()
            .map(|description| truncate(description, DESC_WIDTH))
            .unwrap_or_default();
        let detail = if description.is_empty() {
            kind.to_string()
        } else {
            format!("{kind:<10} {description}")
        };
        let customized = if entry.customized {
            format!(
                "  {dim}customized{reset}",
                dim = colors.dim,
                reset = colors.reset
            )
        } else {
            String::new()
        };

        writeln!(
            output,
            "  {bold}{name:<NAME_WIDTH$}{reset} {dim}{detail}{reset}{customized}",
            bold = colors.bold,
            reset = colors.reset,
            dim = colors.dim,
        )
        .expect("write catalog entry");

        if let Some(flow) = &entry.flow {
            if flow.written == flow.collapsed {
                writeln!(
                    output,
                    "    {dim}{value}{reset}",
                    dim = colors.dim,
                    value = flow.written,
                    reset = colors.reset
                )
                .expect("write unchanged flow");
            } else {
                writeln!(
                    output,
                    "    {dim}{label:<10}{value}{reset}",
                    dim = colors.dim,
                    label = "written",
                    value = flow.written,
                    reset = colors.reset
                )
                .expect("write authored flow");
                writeln!(
                    output,
                    "    {dim}{label:<10}{value}{reset}",
                    dim = colors.dim,
                    label = "collapsed",
                    value = flow.collapsed,
                    reset = colors.reset
                )
                .expect("write collapsed flow");
            }
        }
    }
    output.push('\n');
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let truncated: String = value.chars().take(width.saturating_sub(1)).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_COLORS: Colors = Colors {
        cyan: "",
        bold: "",
        dim: "",
        yellow: "",
        green: "",
        red: "",
        reset: "",
    };

    #[test]
    fn unified_catalog_shows_written_and_collapsed_flows() {
        let rendered = render_all(None, NO_COLORS);

        assert!(rendered.starts_with("CATALOG\n\nTask\n"));
        assert!(!rendered.contains("\nFLOWS\n"));
        assert!(!rendered.contains("\nSKILLS\n"));
        assert!(rendered.contains("build                      flow"));
        assert!(rendered.contains("written   kickoff → code → review-slice → demo"));
        assert!(rendered.contains("collapsed kickoff → implement → compress → review-slice → demo"));
        assert!(rendered.contains("code                       flow\n    implement → compress\n"));
        assert!(!rendered
            .lines()
            .any(|line| line == "    written   implement → compress"));
        assert!(!rendered
            .lines()
            .any(|line| line == "    collapsed implement → compress"));
        assert!(rendered.contains("wave/clarify"));
        assert!(!rendered.contains("wave_clarify"));
        assert!(rendered.lines().all(|line| line.trim_end() == line));
    }
}
