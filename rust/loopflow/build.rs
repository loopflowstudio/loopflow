//! Generate the migration catalogue and application provenance; link native helpers.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

// Identities are shared with the runtime; ordering runs only here (and in the
// runtime's tests).
#[path = "src/store/migration_build.rs"]
mod migration_build;
#[allow(dead_code)]
#[path = "src/store/migration_catalog.rs"]
mod migration_catalog;
#[path = "src/store/migration_schema.rs"]
mod migration_schema;

fn main() {
    println!("cargo:rerun-if-changed=src/installation/registration.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/installation/registration.m")
            .flag("-fobjc-arc")
            .compile("server_registration");
        println!("cargo:rustc-link-lib=framework=ServiceManagement");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/journal/os_process/macos.c");
        cc::Build::new()
            .file("src/journal/os_process/macos.c")
            .compile("loopflow_process_info");
    }
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    link_ghostty_vt(&manifest_dir, &out_dir);
    let git_root = manifest_dir
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf);
    let unmerged = emit_migration_catalog(&manifest_dir, git_root.as_deref(), &out_dir);
    emit_build_provenance(&manifest_dir, git_root.as_deref(), &unmerged);
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
        None => match git_root {
            Some(root) => {
                let root = root
                    .canonicalize()
                    .expect("canonicalize Loopflow source root");
                let directory = directory
                    .canonicalize()
                    .expect("canonicalize migrations directory");
                migration_build::merged_order(&root, &directory)
            }
            None => {
                let path = manifest_dir.join("migration-order.txt");
                println!("cargo:rerun-if-changed={}", path.display());
                fs::read_to_string(path)
                    .expect("read packaged migration order")
                    .lines()
                    .map(str::to_string)
                    .collect()
            }
        },
    };
    let catalog = migration_build::order_catalog(&directory, &merged_order)
        .unwrap_or_else(|error| panic!("order migrations: {error}"));
    let connection = migration_build::apply_catalog(&catalog)
        .unwrap_or_else(|error| panic!("apply migration catalog: {error}"));

    let mut code = String::from("static MIGRATIONS: &[Migration] = &[\n");
    for entry in &catalog {
        let path = entry
            .path
            .canonicalize()
            .unwrap_or_else(|error| panic!("canonicalize {}: {error}", entry.path.display()));
        writeln!(
            code,
            "    Migration {{ version: {:?}, sql: include_str!({:?}), merged: {} }},",
            entry.version,
            path.to_string_lossy(),
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
    // Test processes each get an independent copy, including nextest's one-test
    // processes. Replaying the catalog here keeps that cost out of every fixture.
    let fixture = out_dir.join("test-store.db");
    let _ = fs::remove_file(&fixture);
    for (index, entry) in catalog.iter().enumerate() {
        let sql = fs::read(&entry.path).expect("read fixture migration");
        let checksum = format!("{:x}", Sha256::digest(&sql));
        connection
            .execute(
                "INSERT INTO schema_migrations(version,applied_at,checksum) VALUES(?1,?2,?3)",
                rusqlite::params![entry.version, index as i64, checksum],
            )
            .expect("record fixture migration");
    }
    connection
        .execute(
            "VACUUM INTO ?1",
            [fixture.to_str().expect("UTF-8 build directory")],
        )
        .expect("write test store image");

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

// --- libghostty-vt ---

/// Link the prebuilt libghostty-vt archive for the build target. The pin file
/// names the Ghostty revision, the artifact generation and each target's
/// SHA-256; `scripts/loopflow-dev.py ghostty-vt-build` produces and publishes
/// the archives. `LOOPFLOW_GHOSTTY_VT_DIR` names a directory holding a locally
/// built `libghostty-vt.a` instead, for offline work or a Ghostty bump.
fn link_ghostty_vt(manifest_dir: &Path, out_dir: &Path) {
    let pin_path = manifest_dir.join("ghostty-vt.json");
    println!("cargo:rerun-if-changed={}", pin_path.display());
    println!("cargo:rerun-if-env-changed=LOOPFLOW_GHOSTTY_VT_DIR");
    let pin: GhosttyVtPin =
        serde_json::from_str(&fs::read_to_string(&pin_path).expect("ghostty-vt.json is readable"))
            .expect("ghostty-vt.json is valid");
    println!("cargo:rustc-env=LOOPFLOW_GHOSTTY_REVISION={}", pin.revision);
    let lib_dir = match env::var_os("LOOPFLOW_GHOSTTY_VT_DIR") {
        Some(dir) => {
            let archive = PathBuf::from(dir).join("libghostty-vt.a");
            println!("cargo:rerun-if-changed={}", archive.display());
            // Zig also emits a dylib here. Apple's linker prefers it even for
            // rustc's static request; search a directory containing only the archive.
            let dir = out_dir.join("ghostty-vt-local");
            fs::create_dir_all(&dir).expect("create local libghostty-vt directory");
            fs::copy(&archive, dir.join("libghostty-vt.a"))
                .unwrap_or_else(|error| panic!("copy {}: {error}", archive.display()));
            dir
        }
        None => fetch_ghostty_vt(&pin, out_dir),
    };
    assert!(
        lib_dir.join("libghostty-vt.a").is_file(),
        "{} holds no libghostty-vt.a",
        lib_dir.display()
    );
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=ghostty-vt");
}

/// `ghostty-vt.json`: the Ghostty revision, the artifact generation (bumped
/// when a patch or build flag changes) and each target's archive checksum.
#[derive(serde::Deserialize)]
struct GhosttyVtPin {
    revision: String,
    artifact: String,
    base_url: String,
    sha256: std::collections::BTreeMap<String, String>,
}

/// The published archive for this target, downloaded once per build directory.
fn fetch_ghostty_vt(pin: &GhosttyVtPin, out_dir: &Path) -> PathBuf {
    let target = env::var("TARGET").expect("TARGET not set");
    let name = format!(
        "libghostty-vt-{}-{}-{target}.tar.gz",
        &pin.revision[..7],
        pin.artifact
    );
    let expected = pin
        .sha256
        .get(&target)
        .unwrap_or_else(|| panic!("ghostty-vt.json pins no SHA-256 for {target}"));
    let dir = out_dir.join("ghostty-vt").join(&name);
    let stamp = dir.join("sha256");
    if fs::read_to_string(&stamp).is_ok_and(|pinned| pinned.trim() == expected) {
        return dir;
    }
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create libghostty-vt directory");
    let archive = dir.join(&name);
    let url = format!("{}/{name}", pin.base_url);
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&archive)
        .arg(&url)
        .status()
        .expect("run curl");
    assert!(
        status.success(),
        "downloading {url} failed; set LOOPFLOW_GHOSTTY_VT_DIR to a local libghostty-vt build"
    );
    let bytes = fs::read(&archive).expect("read downloaded archive");
    let digest = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&bytes));
    assert!(
        &digest == expected,
        "{name} has SHA-256 {digest}; ghostty-vt.json pins {expected}"
    );
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .status()
        .expect("run tar");
    assert!(status.success(), "extracting {name} failed");
    fs::write(&stamp, expected).expect("record archive checksum");
    dir
}
