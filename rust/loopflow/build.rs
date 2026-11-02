//! Scans the builtins directory and generates registration code so that
//! adding a new .md or .yaml file is all you need — no manual HashMap insert.

// Build registration only uses the portable projection.
#[allow(dead_code)]
#[path = "src/definition/name.rs"]
mod definition_name;
use definition_name::portable_name;

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// Identities are shared with the runtime; ordering and the additivity proof
// run only here (and in the runtime's tests).
#[path = "src/store/migration_build.rs"]
mod migration_build;
#[allow(dead_code)]
#[path = "src/store/migration_catalog.rs"]
mod migration_catalog;
#[path = "src/store/migration_schema.rs"]
mod migration_schema;

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
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/journal/os_process/macos.c");
        cc::Build::new()
            .file("src/journal/os_process/macos.c")
            .compile("loopflow_process_info");
    }
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let git_root = manifest_dir
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf);
    let unmerged = emit_migration_catalog(&manifest_dir, git_root.as_deref(), &out_dir);
    emit_build_provenance(&manifest_dir, git_root.as_deref(), &unmerged);
    let builtins_dir = manifest_dir.join("src/builtins");

    for (kind, extension) in [("skill", "md"), ("graph", "yaml")] {
        generate_builtins(&builtins_dir, &out_dir, kind, extension);
    }

    // Re-run if any file in the builtins tree changes
    println!("cargo:rerun-if-changed={}", builtins_dir.display());
    for entry in walkdir(&builtins_dir) {
        println!("cargo:rerun-if-changed={}", entry.display());
    }
}

/// Generate `migrations.rs`, the ordered catalog, and the schema it builds;
/// replaying every migration would cost each process that opens a store.
/// Returns the slug migrations `origin/main` has not added.
fn emit_migration_catalog(
    manifest_dir: &Path,
    git_root: Option<&Path>,
    out_dir: &Path,
) -> Vec<String> {
    let directory = manifest_dir.join("src/store/migrations");
    println!("cargo:rerun-if-changed={}", directory.display());
    // A source snapshot without Git history (a container build) names the
    // order origin/main gave its migrations, one slug per line.
    println!("cargo:rerun-if-env-changed=LOOPFLOW_MIGRATION_ORDER");
    let merged_order = match env::var_os("LOOPFLOW_MIGRATION_ORDER") {
        Some(path) => {
            println!("cargo:rerun-if-changed={}", Path::new(&path).display());
            fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read LOOPFLOW_MIGRATION_ORDER: {error}"))
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        }
        None => git_root
            .map(|root| {
                let root = root
                    .canonicalize()
                    .expect("canonicalize Loopflow source root");
                let directory = directory
                    .canonicalize()
                    .expect("canonicalize migrations directory");
                migration_build::merged_order(&root, &directory)
            })
            .unwrap_or_default(),
    };
    let ordered = migration_build::order_catalog(&directory, &merged_order)
        .unwrap_or_else(|error| panic!("order migrations: {error}"));
    let (catalog, connection) = migration_build::build_catalog(ordered)
        .unwrap_or_else(|error| panic!("build migration catalog: {error}"));

    let mut code = String::from("static MIGRATIONS: &[Migration] = &[\n");
    for entry in &catalog {
        let path = entry
            .path
            .canonicalize()
            .unwrap_or_else(|error| panic!("canonicalize {}: {error}", entry.path.display()));
        writeln!(
            code,
            "    Migration {{ version: {:?}, sql: include_str!({:?}), reader_floor: {:?}, merged: {} }},",
            entry.version,
            path.to_string_lossy(),
            entry.reader_floor,
            entry.merged
        )
        .expect("write migration catalog");
    }
    code.push_str("];\n");
    fs::write(out_dir.join("migrations.rs"), code).expect("write migration catalog");

    let schema =
        migration_schema::product_schema(&connection).expect("project catalog schema reference");
    let json = serde_json::to_vec(&*schema).expect("serialize catalog schema reference");
    fs::write(out_dir.join("canonical_schema.json"), json).expect("write catalog schema reference");
    catalog
        .into_iter()
        .filter(|entry| !entry.merged)
        .map(|entry| entry.version)
        .collect()
}

fn emit_build_provenance(manifest_dir: &Path, git_root: Option<&Path>, unmerged: &[String]) {
    let package_version =
        env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION not set by Cargo");
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
    let release_candidate_tag = env::var("LOOPFLOW_RELEASE_TAG")
        .ok()
        .filter(|tag| !tag.is_empty());
    let is_release_candidate = release_candidate_tag.as_deref().is_some_and(|tag| {
        if provenance != "release" {
            panic!("LOOPFLOW_RELEASE_TAG requires LOOPFLOW_BUILD_PROVENANCE=release");
        }
        if tag.rsplit('/').next() != Some(&version_tag) {
            panic!("LOOPFLOW_RELEASE_TAG must match package version {version_tag}, got {tag}");
        }
        if dirty {
            panic!("LOOPFLOW_RELEASE_TAG requires a clean source tree");
        }
        if !unmerged.is_empty() {
            panic!(
                "LOOPFLOW_RELEASE_TAG requires every migration on origin/main with full history; unmerged: {}",
                unmerged.join(", ")
            );
        }
        true
    });
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
    let is_release = is_release_candidate || is_tagged_release || source_revision == "unknown";
    let short_revision = source_revision.get(..9).unwrap_or(&source_revision);
    // An unreleased build names what it is built on, and whether it carries
    // schema work origin/main does not have yet.
    let label = if is_release {
        String::new()
    } else {
        let kind = if unmerged.is_empty() {
            "build"
        } else {
            "schema work"
        };
        let dirty = if dirty { "dirty " } else { "" };
        let base = git_root
            .and_then(|root| {
                Command::new("git")
                    .args(["describe", "--tags", "--abbrev=0", "--match", "v[0-9]*"])
                    .current_dir(root)
                    .output()
                    .ok()
                    .filter(|output| output.status.success())
            })
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|tag| !tag.is_empty())
            .map(|tag| format!(" on {tag}"))
            .unwrap_or_default();
        format!("{dirty}{kind} {short_revision}{base}")
    };
    let build_version = if is_release {
        package_version
    } else {
        let dirty_suffix = if dirty { ".dirty" } else { "" };
        format!("{package_version}+{short_revision}{dirty_suffix}")
    };
    let version_line = if label.is_empty() {
        build_version.clone()
    } else {
        format!("{build_version} ({label})")
    };
    let mut source_revision = source_revision;
    if dirty {
        source_revision.push_str("-dirty");
    }
    // Cargo exports rustc-env values to test processes. Keep compiled outputs
    // separate from build inputs so fixture Cargo calls do not change them.
    println!("cargo:rustc-env=LOOPFLOW_COMPILED_PROVENANCE={provenance}");
    println!("cargo:rustc-env=LOOPFLOW_BUILD_SOURCE_ROOT={}", source_root);
    println!("cargo:rustc-env=LOOPFLOW_BUILD_SOURCE_REVISION={source_revision}");
    println!("cargo:rustc-env=LOOPFLOW_BUILD_VERSION={build_version}");
    println!("cargo:rustc-env=LOOPFLOW_BUILD_LABEL={label}");
    println!("cargo:rustc-env=LOOPFLOW_VERSION_LINE={version_line}");
    println!("cargo:rerun-if-env-changed=LOOPFLOW_BUILD_PROVENANCE");
    println!("cargo:rerun-if-env-changed=LOOPFLOW_RELEASE_TAG");
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src").display()
    );
    if let Some(root) = git_root {
        let mut git_paths = vec![
            "HEAD".to_string(),
            "refs/tags".into(),
            "packed-refs".into(),
            "refs/remotes/origin/main".into(),
        ];
        if let Ok(output) = Command::new("git")
            .args(["symbolic-ref", "--quiet", "HEAD"])
            .current_dir(root)
            .output()
        {
            if output.status.success() {
                git_paths.push(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }
        for git_path in git_paths {
            if let Ok(output) = Command::new("git")
                .args(["rev-parse", "--git-path", &git_path])
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
                    // Missing inputs make every build dirty. A packed branch
                    // needs its parent watched until a commit creates a loose ref.
                    if git_path == "packed-refs" && !path.exists() {
                        continue;
                    }
                    if let Some(path) = path.ancestors().find(|path| path.exists()) {
                        println!("cargo:rerun-if-changed={}", path.display());
                    }
                }
            }
        }
    }
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
        let abs = path
            .canonicalize()
            .unwrap_or_else(|e| panic!("canonicalize {}: {e}", path.display()));
        let abs_str = abs.to_string_lossy().replace('\\', "/");
        writeln!(code, "    ({name:?}, include_str!({abs_str:?})),").expect("write to String");
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
