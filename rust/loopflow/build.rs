//! Scans the builtins directory and generates registration code so that
//! adding a new .md or .yaml file is all you need — no manual HashMap insert.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let builtins_dir = manifest_dir.join("src/engine/builtins");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    generate_map(
        &builtins_dir.join("steps"),
        "md",
        "BUILTIN_STEPS",
        &out_dir.join("builtin_steps.rs"),
    );

    generate_map(
        &builtins_dir.join("flows"),
        "yaml",
        "BUILTIN_FLOWS",
        &out_dir.join("builtin_flows.rs"),
    );

    generate_map(
        &builtins_dir.join("directions"),
        "md",
        "BUILTIN_DIRECTIONS",
        &out_dir.join("builtin_directions.rs"),
    );

    generate_map(
        &builtins_dir.join("ops"),
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
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut code = String::new();
    writeln!(
        code,
        "static {map_name}: std::sync::LazyLock<std::collections::HashMap<&'static str, &'static str>> = std::sync::LazyLock::new(|| {{"
    )
    .unwrap();
    writeln!(code, "    let mut m = std::collections::HashMap::new();").unwrap();

    for (name, path) in &entries {
        // include_str! in generated code needs absolute paths since it's
        // evaluated relative to the generated file in OUT_DIR.
        let abs = path.canonicalize().unwrap();
        let abs_str = abs.to_string_lossy().replace('\\', "/");
        writeln!(code, "    m.insert(\"{name}\", include_str!(\"{abs_str}\"));").unwrap();
    }

    writeln!(code, "    m").unwrap();
    writeln!(code, "}});").unwrap();

    fs::write(out_path, code).unwrap();
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
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
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
