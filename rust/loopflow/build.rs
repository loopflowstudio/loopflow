//! Embed application provenance.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let git_root = manifest_dir
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf);
    let unmerged = env::var("DEP_LOOPFLOW_STORAGE_UNMERGED")
        .expect("storage migration catalogue metadata")
        .split(',')
        .filter(|slug| !slug.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    emit_build_provenance(&manifest_dir, git_root.as_deref(), &unmerged);
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
    // Provenance describes the whole workspace, including an initial edit in a
    // lower crate before any application file changes.
    let sources = git_root.map_or_else(|| manifest_dir.join("src"), |root| root.join("rust"));
    println!("cargo:rerun-if-changed={}", sources.display());
    if let Some(root) = git_root {
        for input in ["Cargo.toml", "Cargo.lock"] {
            println!("cargo:rerun-if-changed={}", root.join(input).display());
        }
    }
    if let Some(root) = git_root {
        let mut git_paths = vec!["HEAD".to_string(), "refs/tags".into(), "packed-refs".into()];
        if let Ok(output) = Command::new("git")
            .args(["symbolic-ref", "--quiet", "HEAD"])
            .current_dir(root)
            .output()
        {
            if output.status.success() {
                let reference = String::from_utf8_lossy(&output.stdout).trim().to_string();
                // A commit on a packed branch creates its loose ref. Watching
                // the parent also sees every sibling branch; this checkout's
                // branch reflog follows updates to just the selected ref.
                let reflog = git_path(root, &format!("logs/{reference}"));
                if let Some(reflog) = reflog.filter(|path| path.exists()) {
                    println!("cargo:rerun-if-changed={}", reflog.display());
                } else {
                    // Repositories without a branch reflog still need to notice
                    // their first loose ref, including an empty commit.
                    if let Some(path) = git_path(root, &reference).and_then(|path| {
                        path.ancestors()
                            .find(|path| path.exists())
                            .map(Path::to_path_buf)
                    }) {
                        println!("cargo:rerun-if-changed={}", path.display());
                    }
                }
                git_paths.push(reference);
            }
        }
        // Storage owns origin/main and propagates catalogue changes through
        // its build metadata. Never broaden a missing input to shared refs.
        for reference in git_paths {
            if let Some(path) = git_path(root, &reference).filter(|path| path.exists()) {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

fn git_path(root: &Path, reference: &str) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", reference])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}
