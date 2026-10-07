//! Scans the builtins directory and generates registration code so that
//! adding a new .md or .yaml file is all you need — no manual HashMap insert.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// The shared module also declares the runtime view constructed by generated code.
#[allow(dead_code)]
#[path = "src/migration_drafts.rs"]
mod migration_drafts;

// Identity parsing and formatting belong to the runtime migration API.
#[allow(dead_code)]
#[path = "src/store/migration_catalog.rs"]
mod migration_catalog;
#[path = "src/store/migration_schema.rs"]
mod migration_schema;

/// Category directories whose skill/flow names are registered flat (no prefix).
/// Everything else is a namespaced category: names are stored as `<cat>/<name>`.
/// Core categories share one flat namespace and must not collide with each other.
/// Within a filename, `_` encodes a namespace separator (`wave_clarify.md`
/// registers as `wave/clarify`); `-` remains a word separator.
const CORE_CATEGORIES: &[&str] = &["task", "project", "wave", "ops"];

/// Each ongoing conversation carries its scope's operating procedure inline:
/// the registered session skill is its own file followed by the body of its
/// operate skill. The operate skill stays registered on its own.
const SESSION_OPERATE_PAIRS: &[(&str, &str)] = &[
    ("repo/session", "repo/operate"),
    ("wave/session", "wave/operate"),
    ("task/session", "task/operate"),
];

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    emit_build_provenance(&manifest_dir, &out_dir);
    emit_schema_references(&manifest_dir, &out_dir);
    let builtins_dir = manifest_dir.join("src/engine/builtins");

    // Builtins live at `<cat>/<kind>/*.ext`. Skills and flows from CORE_CATEGORIES
    // are registered flat; skills and flows from other categories are registered
    // as `<cat>/<name>` and are also reachable by bare name when unambiguous.
    generate_kind_map(
        &builtins_dir,
        "skill",
        "md",
        "BUILTIN_SKILLS",
        &out_dir.join("builtin_skills.rs"),
    );

    generate_kind_map(
        &builtins_dir,
        "flow",
        "yaml",
        "BUILTIN_FLOWS",
        &out_dir.join("builtin_flows.rs"),
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
        "BUILTIN_SKILL_CATEGORIES",
        &out_dir.join("builtin_skill_categories.rs"),
    );

    // Re-run if any file in the builtins tree changes
    println!("cargo:rerun-if-changed={}", builtins_dir.display());
    for entry in walkdir(&builtins_dir) {
        println!("cargo:rerun-if-changed={}", entry.display());
    }
}

/// Replaying every migration costs each process that opens a store. Embed the
/// canonical schema, and the schema a draft-bearing build validates against.
fn emit_schema_references(manifest_dir: &Path, out_dir: &Path) {
    let connection =
        rusqlite::Connection::open_in_memory().expect("open canonical schema reference database");
    connection.execute_batch(
        "CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
    ).expect("create schema reference migration ledger");
    for migration in migration_catalog::MIGRATIONS {
        connection
            .execute_batch(migration.sql)
            .unwrap_or_else(|error| {
                panic!("build canonical schema at {}: {error}", migration.version())
            });
    }
    let schema =
        migration_schema::product_schema(&connection).expect("project canonical schema reference");
    let json = serde_json::to_vec(&schema).expect("serialize canonical schema reference");
    fs::write(out_dir.join("canonical_schema.json"), json)
        .expect("write canonical schema reference");

    let drafts = migration_drafts(manifest_dir);
    for draft in &drafts {
        connection
            .execute_batch(&draft.sql)
            .unwrap_or_else(|error| panic!("build draft schema at {}: {error}", draft.name));
    }
    let schema = (!drafts.is_empty()).then(|| {
        migration_schema::product_schema(&connection).expect("project draft schema reference")
    });
    let json = serde_json::to_vec(&schema).expect("serialize draft schema reference");
    fs::write(out_dir.join("draft_schema.json"), json).expect("write draft schema reference");
}

fn migration_drafts(manifest_dir: &Path) -> Vec<migration_drafts::DraftMigration> {
    use migration_drafts::{read_draft_manifest, read_released_names};

    let migrations_dir = manifest_dir.join("src/store/migrations");
    let drafts_dir = migrations_dir.join("drafts");
    println!("cargo:rerun-if-changed={}", drafts_dir.display());
    println!("cargo:rerun-if-changed={}", migrations_dir.display());
    let released_names = read_released_names(&migrations_dir)
        .unwrap_or_else(|error| panic!("embed migration draft manifest: {error}"));
    read_draft_manifest(&drafts_dir, &released_names)
        .unwrap_or_else(|error| panic!("embed migration draft manifest: {error}"))
}

fn emit_build_provenance(manifest_dir: &Path, out_dir: &Path) {
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
    emit_migration_draft_manifest(manifest_dir, out_dir);
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
    let build_version = if is_release_candidate || is_tagged_release || source_revision == "unknown"
    {
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
    println!("cargo:rerun-if-env-changed=LOOPFLOW_RELEASE_TAG");
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src").display()
    );
    if let Some(root) = git_root {
        let mut git_paths = vec!["HEAD".to_string(), "refs/tags".into(), "packed-refs".into()];
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

fn emit_migration_draft_manifest(manifest_dir: &Path, out_dir: &Path) {
    let drafts = migration_drafts(manifest_dir);
    let mut code = String::from("static MIGRATION_DRAFT_MANIFEST: &[MigrationDraft] = &[\n");
    for draft in drafts {
        writeln!(code, "    MigrationDraft {{").expect("write draft manifest");
        writeln!(code, "        name: {:?},", draft.name).expect("write draft manifest");
        writeln!(code, "        sql: {:?},", draft.sql).expect("write draft manifest");
        code.push_str("    },\n");
    }
    code.push_str("];\n");
    fs::write(out_dir.join("migration_draft_manifest.rs"), code)
        .expect("write embedded migration draft manifest");
}

/// Collect files of the given extension from `<builtins_dir>/<cat>/<kind>/`
/// for each top-level category directory. Core categories
/// (task/project/wave/ops) share one flat namespace — duplicate stems across
/// cores panic. Non-core categories get their name as a prefix:
/// `vendor/review`.
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
            for (stem, path) in files {
                let name = canonical_builtin_name(&stem);
                if is_core {
                    entries.push((name, path));
                } else {
                    entries.push((format!("{cat_name}/{name}"), path));
                }
            }
        }
    }
    if kind == "skill" {
        compose_sessions(
            &mut entries,
            out_path.parent().expect("generated map has a directory"),
        );
    }
    emit_map(&mut entries, map_name, out_path);
}

fn compose_sessions(entries: &mut [(String, PathBuf)], out_dir: &Path) {
    let composed_dir = out_dir.join("composed_skills");
    fs::create_dir_all(&composed_dir).expect("create composed skill directory");
    for (session, operate) in SESSION_OPERATE_PAIRS {
        let index = |name: &str| {
            entries
                .iter()
                .position(|(entry, _)| entry == name)
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
        let path = composed_dir.join(format!("{}.md", session.replace('/', "_")));
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

    if entries.is_empty() {
        fs::write(
            out_path,
            format!(
                "static {map_name}: std::sync::LazyLock<std::collections::HashMap<&'static str, &'static str>> = std::sync::LazyLock::new(std::collections::HashMap::new);\n"
            ),
        )
        .unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
        return;
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
/// `<cat>/` prefix so they match the keys in BUILTIN_SKILLS / BUILTIN_FLOWS.
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
                    let stem = canonical_builtin_name(&stem);
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

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

fn canonical_builtin_name(stem: &str) -> String {
    stem.replace('_', "/")
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
