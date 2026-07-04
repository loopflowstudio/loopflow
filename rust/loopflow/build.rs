//! Scans the builtins directory and generates registration code so that
//! adding a new .md or .yaml file is all you need — no manual HashMap insert.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let builtins_dir = manifest_dir.join("src/engine/builtins");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Builtins live at `<cat>/<kind>/*.ext`. All categories share one flat
    // namespace — duplicate stems across categories panic.
    generate_kind_map(
        &builtins_dir,
        "step",
        "md",
        "BUILTIN_STEPS",
        &out_dir.join("builtin_steps.rs"),
    );

    generate_kind_map(
        &builtins_dir,
        "flow",
        "yaml",
        "BUILTIN_FLOWS",
        &out_dir.join("builtin_flows.rs"),
    );
    generate_kind_map(
        &builtins_dir,
        "goal",
        "md",
        "BUILTIN_GOALS",
        &out_dir.join("builtin_goals.rs"),
    );

    generate_category_map(
        &builtins_dir,
        "flow",
        "yaml",
        "BUILTIN_FLOW_CATEGORIES",
        &out_dir.join("builtin_flow_categories.rs"),
    );
    generate_category_map(
        &builtins_dir,
        "step",
        "md",
        "BUILTIN_STEP_CATEGORIES",
        &out_dir.join("builtin_step_categories.rs"),
    );

    generate_map(
        &builtins_dir.join("ops/prompt"),
        "md",
        "BUILTIN_OPS_PROMPTS",
        &out_dir.join("builtin_ops_prompts.rs"),
    );

    // Re-run if any file in the builtins tree changes
    println!("cargo:rerun-if-changed={}", builtins_dir.display());
    for entry in walkdir(&builtins_dir) {
        println!("cargo:rerun-if-changed={}", entry.display());
    }
}

fn generate_map(dir: &Path, extension: &str, map_name: &str, out_path: &Path) {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    collect_files(dir, extension, &mut entries);
    emit_map(&mut entries, map_name, out_path);
}

/// Collect files of the given extension from `<builtins_dir>/<cat>/<kind>/`
/// for each top-level category directory. All categories share one flat
/// namespace — duplicate stems across categories panic.
fn generate_kind_map(
    builtins_dir: &Path,
    kind: &str,
    extension: &str,
    map_name: &str,
    out_path: &Path,
) {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    if let Ok(cats) = fs::read_dir(builtins_dir) {
        for cat in cats.flatten() {
            let kind_dir = cat.path().join(kind);
            if kind_dir.is_dir() {
                collect_files(&kind_dir, extension, &mut entries);
            }
        }
    }
    emit_map(&mut entries, map_name, out_path);
}

fn emit_map(entries: &mut [(String, PathBuf)], map_name: &str, out_path: &Path) {
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
        "static {map_name}: std::sync::LazyLock<std::collections::HashMap<&'static str, &'static str>> = std::sync::LazyLock::new(|| {{"
    )
    .expect("write to String");
    writeln!(code, "    let mut m = std::collections::HashMap::new();").expect("write to String");

    for (name, path) in entries.iter() {
        let abs = path
            .canonicalize()
            .unwrap_or_else(|e| panic!("canonicalize {}: {e}", path.display()));
        let abs_str = abs.to_string_lossy().replace('\\', "/");
        writeln!(
            code,
            "    m.insert(\"{name}\", include_str!(\"{abs_str}\"));"
        )
        .expect("write to String");
    }

    writeln!(code, "    m").expect("write to String");
    writeln!(code, "}});").expect("write to String");

    fs::write(out_path, code).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
}

/// Generate a `<MAP_NAME>: &[(category, &[name])]` constant from
/// `<builtins_dir>/<cat>/<kind>/*.<ext>`. Categories are title-cased; names
/// stay bare so they match the keys in BUILTIN_STEPS / BUILTIN_FLOWS.
fn generate_category_map(
    builtins_dir: &Path,
    kind: &str,
    extension: &str,
    map_name: &str,
    out_path: &Path,
) {
    let mut categories: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    let Ok(entries) = fs::read_dir(builtins_dir) else {
        fs::write(
            out_path,
            format!("pub const {map_name}: &[(&str, &[&str])] = &[];\n"),
        )
        .expect("write empty category map");
        return;
    };

    for entry in entries.flatten() {
        let cat_path = entry.path();
        if !cat_path.is_dir() {
            continue;
        }
        let kind_dir = cat_path.join(kind);
        if !kind_dir.is_dir() {
            continue;
        }
        let cat_name = cat_path
            .file_name()
            .expect("dir has no name")
            .to_string_lossy()
            .to_string();
        let category = title_case(&cat_name);

        let mut names = Vec::new();
        if let Ok(files) = fs::read_dir(&kind_dir) {
            for file in files.flatten() {
                let file_path = file.path();
                let matches_ext = match extension {
                    "yaml" => file_path
                        .extension()
                        .is_some_and(|e| e == "yaml" || e == "yml"),
                    other => file_path.extension().is_some_and(|e| e == other),
                };
                if matches_ext {
                    let stem = file_path
                        .file_stem()
                        .expect("file has no stem")
                        .to_string_lossy()
                        .to_string();
                    names.push(stem);
                }
            }
        }
        names.sort();
        if !names.is_empty() {
            categories.insert(category, names);
        }
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

/// Recursively collect files with the given extension. The key is the file stem.
fn collect_files(dir: &Path, extension: &str, entries: &mut Vec<(String, PathBuf)>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, extension, entries);
        } else if path.extension().is_some_and(|e| e == extension) {
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
