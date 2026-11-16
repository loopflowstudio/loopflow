//! Link the pinned terminal parser for the build target.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"));
    let output = PathBuf::from(env::var("OUT_DIR").expect("Cargo build output directory"));
    link_ghostty_vt(&manifest, &output);
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
