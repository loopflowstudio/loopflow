//! Scans the builtins directory and generates registration code so that
//! adding a new .md or .yaml file is all you need — no manual HashMap insert.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Category directories whose skill/flow names are registered flat (no prefix).
/// Everything else is a namespaced category: names are stored as `<cat>/<name>`.
/// Core categories share one flat namespace and must not collide with each other.
const CORE_CATEGORIES: &[&str] = &["build", "govern", "ops"];

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    emit_build_provenance(&manifest_dir);
    let builtins_dir = manifest_dir.join("src/engine/builtins");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Builtins live at `<cat>/<kind>/*.ext`. Skills and flows from CORE_CATEGORIES
    // are registered flat; skills and flows from other categories are registered
    // as `<cat>/<name>` and are also reachable by bare name when unambiguous.
    generate_kind_map(
        &builtins_dir,
        "skill",
        "md",
        "BUILTIN_STEPS",
        &out_dir.join("builtin_skills.rs"),
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
        "skill",
        "md",
        "BUILTIN_STEP_CATEGORIES",
        &out_dir.join("builtin_skill_categories.rs"),
    );

    generate_map(
        &builtins_dir.join("directions"),
        "md",
        "BUILTIN_DIRECTIONS",
        &out_dir.join("builtin_directions.rs"),
    );
    assert_unique_direction_node_names(&builtins_dir.join("directions"));
    generate_direction_groups(
        &builtins_dir.join("directions"),
        &out_dir.join("builtin_direction_groups.rs"),
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

fn emit_build_provenance(manifest_dir: &Path) {
    let package_version =
        env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION not set by Cargo");
    let git_root = manifest_dir
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists());
    let provenance = env::var("LOOPFLOW_BUILD_PROVENANCE").unwrap_or_else(|_| {
        if git_root.is_some() {
            "development".to_string()
        } else {
            "release".to_string()
        }
    });
    if !matches!(provenance.as_str(), "development" | "release") {
        panic!("LOOPFLOW_BUILD_PROVENANCE must be `development` or `release`, got `{provenance}`");
    }
    let source_root = if provenance == "release" {
        "release".to_string()
    } else {
        git_root
            .unwrap_or(manifest_dir)
            .canonicalize()
            .unwrap_or_else(|error| panic!("canonicalize Loopflow source root: {error}"))
            .display()
            .to_string()
    };
    let migration_authority =
        env::var("LOOPFLOW_MIGRATION_AUTHORITY").unwrap_or_else(|_| "validation_only".to_string());
    if !matches!(
        migration_authority.as_str(),
        "published" | "validation_only"
    ) {
        panic!(
            "LOOPFLOW_MIGRATION_AUTHORITY must be `published` or `validation_only`, got `{migration_authority}`"
        );
    }
    let source_revision = git_root
        .and_then(|root| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(root)
                .output()
                .ok()
                .filter(|output| output.status.success())
        })
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|revision| revision.trim().to_string())
        .filter(|revision| !revision.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let dirty = git_root.is_some_and(|root| {
        Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(root)
            .output()
            .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
    });
    let version_tag = format!("v{package_version}");
    let is_tagged_release = !dirty
        && git_root.is_some_and(|root| {
            Command::new("git")
                .args(["tag", "--points-at", "HEAD", "--list", &version_tag])
                .current_dir(root)
                .output()
                .is_ok_and(|output| {
                    output.status.success()
                        && String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .any(|tag| tag == version_tag)
                })
        });
    let build_version = if is_tagged_release || source_revision == "unknown" {
        package_version
    } else {
        let short_revision = source_revision.get(..9).unwrap_or(&source_revision);
        let dirty_suffix = if dirty { ".dirty" } else { "" };
        format!("{package_version}+{short_revision}{dirty_suffix}")
    };
    let mut source_revision = source_revision;
    if dirty {
        source_revision.push_str("-dirty");
    }
    println!("cargo:rustc-env=LOOPFLOW_BUILD_PROVENANCE={provenance}");
    println!("cargo:rustc-env=LOOPFLOW_BUILD_SOURCE_ROOT={}", source_root);
    println!("cargo:rustc-env=LOOPFLOW_MIGRATION_AUTHORITY={migration_authority}");
    println!("cargo:rustc-env=LOOPFLOW_BUILD_SOURCE_REVISION={source_revision}");
    println!("cargo:rustc-env=LOOPFLOW_BUILD_VERSION={build_version}");
    println!("cargo:rerun-if-env-changed=LOOPFLOW_BUILD_PROVENANCE");
    println!("cargo:rerun-if-env-changed=LOOPFLOW_MIGRATION_AUTHORITY");
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src").display()
    );
    if let Some(root) = git_root {
        for git_path in ["HEAD", "refs/heads", "refs/tags", "packed-refs"] {
            if let Ok(output) = Command::new("git")
                .args(["rev-parse", "--git-path", git_path])
                .current_dir(root)
                .output()
            {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout);
                    let path = Path::new(path.trim());
                    let path = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        root.join(path)
                    };
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
    }
}

fn generate_map(dir: &Path, extension: &str, map_name: &str, out_path: &Path) {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    collect_files(dir, extension, &mut entries);
    emit_map(&mut entries, map_name, out_path);
}

/// Collect files of the given extension from `<builtins_dir>/<cat>/<kind>/`
/// for each top-level category directory. Core categories (build/govern/ops)
/// share one flat namespace — duplicate stems across cores panic. Non-core
/// categories get their name as a prefix: `gstack/office-hours`.
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
            if is_core {
                entries.extend(files);
            } else {
                for (stem, path) in files {
                    entries.push((format!("{cat_name}/{stem}"), path));
                }
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
/// `<builtins_dir>/<cat>/<kind>/*.<ext>`. Categories are title-cased. Names
/// for core categories stay bare; names for non-core categories get the
/// `<cat>/` prefix so they match the keys in BUILTIN_STEPS / BUILTIN_FLOWS.
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
        let is_core = CORE_CATEGORIES.contains(&cat_name.as_str());
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
                    let name = if is_core {
                        stem
                    } else {
                        format!("{cat_name}/{stem}")
                    };
                    names.push(name);
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

/// Generate `BUILTIN_DIRECTION_GROUPS` from immediate subdirectories under
/// builtins/directions. Top-level files are excluded from groups.
fn generate_direction_groups(directions_dir: &Path, out_path: &Path) {
    let mut groups: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    let Ok(entries) = fs::read_dir(directions_dir) else {
        fs::write(
            out_path,
            "static BUILTIN_DIRECTION_GROUPS: std::sync::LazyLock<std::collections::HashMap<&'static str, Vec<&'static str>>> = std::sync::LazyLock::new(|| std::collections::HashMap::new());\n",
        )
        .expect("write empty direction groups");
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let group_name = path
            .file_name()
            .expect("group dir has no name")
            .to_string_lossy()
            .to_string();

        let mut members = Vec::new();
        if let Ok(files) = fs::read_dir(&path) {
            for file in files.flatten() {
                let file_path = file.path();
                if file_path.extension().is_some_and(|e| e == "md") {
                    let member = file_path
                        .file_stem()
                        .expect("direction file has no stem")
                        .to_string_lossy()
                        .to_string();
                    members.push(member);
                }
            }
        }
        members.sort();
        if !members.is_empty() {
            groups.insert(group_name, members);
        }
    }

    let mut code = String::new();
    writeln!(
        code,
        "static BUILTIN_DIRECTION_GROUPS: std::sync::LazyLock<std::collections::HashMap<&'static str, Vec<&'static str>>> = std::sync::LazyLock::new(|| {{"
    )
    .expect("write to String");
    writeln!(code, "    let mut m = std::collections::HashMap::new();").expect("write to String");
    for (group, members) in &groups {
        let member_list = members
            .iter()
            .map(|member| format!("\"{member}\""))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(code, "    m.insert(\"{group}\", vec![{member_list}]);").expect("write to String");
    }
    writeln!(code, "    m").expect("write to String");
    writeln!(code, "}});").expect("write to String");

    fs::write(out_path, code).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
}

fn assert_unique_direction_node_names(directions_dir: &Path) {
    let mut leaves: Vec<(String, PathBuf)> = Vec::new();
    collect_files(directions_dir, "md", &mut leaves);

    let mut groups = Vec::new();
    if let Ok(entries) = fs::read_dir(directions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .expect("group dir has no name")
                .to_string_lossy()
                .to_string();
            groups.push((name, path));
        }
    }

    let mut origins: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    for (name, path) in groups {
        origins
            .entry(name)
            .or_default()
            .push(format!("group {}", path.display()));
    }
    for (name, path) in leaves {
        origins
            .entry(name)
            .or_default()
            .push(format!("direction {}", path.display()));
    }

    let collisions: Vec<String> = origins
        .into_iter()
        .filter_map(|(name, paths)| {
            if paths.len() > 1 {
                Some(format!("{name}: {}", paths.join(", ")))
            } else {
                None
            }
        })
        .collect();

    if !collisions.is_empty() {
        panic!(
            "duplicate builtin direction node names detected (groups + leaves share one namespace):\n{}",
            collisions.join("\n")
        );
    }
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
