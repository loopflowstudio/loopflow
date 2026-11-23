//! Generate only the builtin definition catalogue; no Git or checkout identity.

#[allow(dead_code)]
#[path = "src/name.rs"]
mod definition_name;
use definition_name::portable_name;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Category directories whose skill/graph names are registered flat (no prefix).
/// Everything else is a namespaced category: public names carry a `<cat>-` prefix.
/// Core categories share one flat namespace and must not collide with each other.
/// Within a filename, `_` encodes a namespace separator (`wave_clarify.md`
/// registers as `wave-clarify`); `-` remains a word separator.
const CORE_CATEGORIES: &[&str] = &["task", "project", "wave", "ops"];

/// Each ongoing conversation carries its scope's operating procedure inline:
/// the registered session skill is its own file followed by the body of its
/// operate skill. The operate skill stays registered on its own.
const SESSION_OPERATE_PAIRS: &[(&str, &str)] = &[
    ("repo-session", "repo-operate"),
    ("wave-session", "wave-operate"),
    ("task-session", "task-operate"),
];

fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets the manifest directory"),
    );
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets the output directory"));
    let builtins_dir = manifest_dir.join("src/builtins");

    for (kind, extension) in [("skill", "md"), ("graph", "yaml")] {
        generate_builtins(&builtins_dir, &out_dir, kind, extension);
    }

    let docs_dir = manifest_dir.join("../../docs");
    generate_docs(&docs_dir, &out_dir.join("builtin_docs.rs"));

    // Re-run if any file in the builtins tree or top-level user docs changes
    println!("cargo:rerun-if-changed={}", builtins_dir.display());
    for entry in walkdir(&builtins_dir) {
        println!("cargo:rerun-if-changed={}", entry.display());
    }
    println!("cargo:rerun-if-changed={}", docs_dir.display());
    for entry in top_level_markdown(&docs_dir) {
        println!("cargo:rerun-if-changed={}", entry.1.display());
    }
}

fn generate_docs(docs_dir: &Path, out_path: &Path) {
    let entries = top_level_markdown(docs_dir);
    let mut code = String::new();
    writeln!(code, "pub const BUILTIN_DOCS: &[BuiltinDoc] = &[").expect("write to String");
    for (slug, path) in &entries {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let title = first_heading(&content).unwrap_or(slug.as_str());
        writeln!(
            code,
            "    BuiltinDoc {{ slug: {slug:?}, title: {title:?}, content: {content:?} }},"
        )
        .expect("write to String");
    }
    writeln!(code, "];").expect("write to String");
    fs::write(out_path, code).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
}

fn top_level_markdown(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut entries = Vec::new();
    let Ok(read_dir) = fs::read_dir(dir) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let slug = path
            .file_stem()
            .expect("markdown file has a stem")
            .to_string_lossy()
            .to_string();
        entries.push((slug, path));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn first_heading(content: &str) -> Option<&str> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
}

/// Collect each kind once so registration, shortcuts and categories share names.
fn generate_builtins(builtins_dir: &Path, out_dir: &Path, kind: &str, extension: &str) {
    let mut entries = Vec::new();
    let mut categories = std::collections::BTreeMap::<String, Vec<String>>::new();
    if let Ok(cats) = fs::read_dir(builtins_dir) {
        for cat in cats.flatten() {
            let cat_path = cat.path();
            let kind_dir = cat_path.join(kind);
            if !kind_dir.is_dir() {
                continue;
            }
            let cat_name = cat_path
                .file_name()
                .expect("dir has no name")
                .to_string_lossy()
                .to_string();
            let is_core = CORE_CATEGORIES.contains(&cat_name.as_str());
            let mut files: Vec<(String, PathBuf)> = Vec::new();
            collect_files(&kind_dir, extension, &mut files);
            for (stem, path) in files {
                let stem = stem.replace('_', "/");
                let name = if is_core {
                    stem
                } else {
                    format!("{cat_name}/{stem}")
                };
                categories
                    .entry(title_case(&cat_name))
                    .or_default()
                    .push(portable_name(&name));
                entries.push((name, path));
            }
        }
    }
    if kind == "skill" {
        compose_sessions(&mut entries, out_dir);
    }
    emit_map(
        &mut entries,
        &format!("BUILTIN_{}S", kind.to_uppercase()),
        &out_dir.join(format!("builtin_{kind}s.rs")),
    );
    emit_category_map(
        categories,
        &format!("BUILTIN_{}_CATEGORIES", kind.to_uppercase()),
        &out_dir.join(format!("builtin_{kind}_categories.rs")),
    );
}

fn compose_sessions(entries: &mut [(String, PathBuf)], out_dir: &Path) {
    let composed_dir = out_dir.join("composed_skills");
    fs::create_dir_all(&composed_dir).expect("create composed skill directory");
    for (session, operate) in SESSION_OPERATE_PAIRS {
        let index = |name: &str| {
            entries
                .iter()
                .position(|(entry, _)| portable_name(entry) == name)
                .unwrap_or_else(|| panic!("builtin skill `{name}` is missing"))
        };
        let read = |path: &Path| {
            fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        };
        let session_index = index(session);
        let composed = format!(
            "{}\n# Operating procedure: `{operate}`\n\n{}",
            read(&entries[session_index].1),
            skill_body(&read(&entries[index(operate)].1))
        );
        let path = composed_dir.join(format!("{session}.md"));
        fs::write(&path, composed).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        entries[session_index].1 = path;
    }
}

fn skill_body(content: &str) -> &str {
    let after_open = content
        .strip_prefix("---")
        .expect("operate skill opens with frontmatter");
    let end = after_open
        .find("\n---")
        .expect("operate skill closes its frontmatter");
    after_open[end + 4..].trim_start_matches('\n')
}

fn emit_map(entries: &mut [(String, PathBuf)], map_name: &str, out_path: &Path) {
    let mut shortcuts = std::collections::BTreeMap::<String, Vec<String>>::new();
    for (name, _) in entries.iter() {
        if let Some((_, bare)) = name.rsplit_once('/') {
            shortcuts
                .entry(bare.into())
                .or_default()
                .push(portable_name(name));
        }
    }
    for (name, _) in entries.iter_mut() {
        *name = portable_name(name);
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    for pair in entries.windows(2) {
        if pair[0].0 == pair[1].0 {
            panic!(
                "duplicate builtin key '{}' for '{}' and '{}'",
                pair[0].0,
                pair[0].1.display(),
                pair[1].1.display()
            );
        }
    }

    let mut code = String::new();
    writeln!(
        code,
        "static {map_name}: std::sync::LazyLock<std::collections::HashMap<&'static str, &'static str>> = std::sync::LazyLock::new(|| std::collections::HashMap::from(["
    )
    .expect("write to String");

    for (name, path) in entries.iter() {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        writeln!(code, "    ({name:?}, {content:?}),").expect("write to String");
    }

    writeln!(code, "]));").expect("write to String");

    writeln!(code, "static {map_name}_SHORTCUTS: &[(&str, &str)] = &[").expect("write shortcuts");
    for (bare, names) in shortcuts {
        if names.len() == 1 {
            writeln!(code, "({bare:?}, {:?}),", names[0]).expect("write shortcut");
        }
    }
    code.push_str("];\n");
    fs::write(out_path, code).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
}

fn emit_category_map(
    mut categories: std::collections::BTreeMap<String, Vec<String>>,
    map_name: &str,
    out_path: &Path,
) {
    for names in categories.values_mut() {
        names.sort();
    }
    let mut code = String::new();
    writeln!(code, "pub const {map_name}: &[(&str, &[&str])] = &[").expect("write to String");
    for (category, names) in &categories {
        let item_list: Vec<String> = names.iter().map(|f| format!("\"{f}\"")).collect();
        writeln!(code, "    (\"{category}\", &[{}]),", item_list.join(", "))
            .expect("write to String");
    }
    writeln!(code, "];").expect("write to String");

    fs::write(out_path, code).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Recursively collect source names, preserving folder hierarchy for shortcuts.
fn collect_files(dir: &Path, extension: &str, entries: &mut Vec<(String, PathBuf)>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() {
            let mut children = Vec::new();
            collect_files(&path, extension, &mut children);
            let folder = path
                .file_name()
                .expect("directory has a name")
                .to_string_lossy();
            entries.extend(
                children
                    .into_iter()
                    .map(|(name, source)| (format!("{folder}/{name}"), source)),
            );
        } else if path
            .extension()
            .is_some_and(|e| e == extension || (extension == "yaml" && e == "yml"))
        {
            let name = path
                .file_stem()
                .expect("file has no stem")
                .to_string_lossy()
                .to_string();
            entries.push((name, path));
        }
    }
}

/// Walk a directory tree, returning all paths.
fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let Ok(read_dir) = fs::read_dir(dir) else {
        return result;
    };
    for entry in read_dir {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        result.push(path.clone());
        if path.is_dir() {
            result.extend(walkdir(&path));
        }
    }
    result
}
