"""Cargo refreshes application identity on the first lower-crate-only edit."""

import json
import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def test_initial_lower_crate_edit_refreshes_clean_application_provenance(tmp_path):
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("LF_", "LOOPFLOW_", "CARGO_", "SCCACHE_"))
        and key not in {"RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS"}
    }
    env["RUSTC_WRAPPER"] = ""

    def run(*args: str) -> str:
        return subprocess.run(
            args,
            cwd=tmp_path,
            env=env,
            check=True,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        ).stdout

    app = tmp_path / "rust/app"
    lower = tmp_path / "rust/storage"
    (app / "src").mkdir(parents=True)
    (lower / "src").mkdir(parents=True)
    (tmp_path / "Cargo.toml").write_text(
        '[workspace]\nresolver="2"\nmembers=["rust/app", "rust/storage"]\n'
    )
    (tmp_path / ".gitignore").write_text("/target\n")
    (app / "Cargo.toml").write_text(
        '[package]\nname="provenance-fixture"\nversion="0.1.0"\nedition="2021"\n'
        '[dependencies]\nloopflow-storage={path="../storage"}\n'
        '[build-dependencies]\ncc="1"\n'
    )
    shutil.copyfile(ROOT / "rust/loopflow/build.rs", app / "build.rs")
    for file in ["installation/registration.m", "journal/os_process/macos.c"]:
        source = app / "src" / file
        source.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / "rust/loopflow/src" / file, source)
    (app / "src/main.rs").write_text(
        'fn main() { println!("{}|{}|{}", env!("LOOPFLOW_BUILD_VERSION"), '
        'env!("LOOPFLOW_BUILD_SOURCE_ROOT"), loopflow_storage::value()); }\n'
    )
    (lower / "Cargo.toml").write_text(
        '[package]\nname="loopflow-storage"\nversion="0.1.0"\n'
        'edition="2021"\nlinks="loopflow_storage"\n'
    )
    (lower / "build.rs").write_text('fn main() { println!("cargo:unmerged="); }\n')
    record = lower / "src/lib.rs"
    record.write_text("pub fn value() -> u8 { 1 }\n")
    run("git", "init", "-q")
    run("cargo", "generate-lockfile", "--offline")
    run("git", "add", ".")
    run(
        "git",
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.com",
        "commit",
        "-qm",
        "initial",
    )

    def build() -> tuple[list[str], str]:
        raw = run("cargo", "build", "--offline", "--jobs", "1", "--message-format=json")
        artifacts = [json.loads(line) for line in raw.splitlines() if line.startswith("{")]
        changed = [
            item["target"]["name"]
            for item in artifacts
            if item.get("reason") == "compiler-artifact" and not item["fresh"]
        ]
        return changed, run(str(tmp_path / "target/debug/provenance-fixture")).strip()

    _, clean = build()
    assert ".dirty" not in clean
    assert clean.endswith(f"|{tmp_path.resolve()}|1")
    record.write_text("pub fn value() -> u8 { 2 }\n")
    changed, dirty = build()
    assert "provenance-fixture" in changed
    assert ".dirty" in dirty
    assert dirty.endswith(f"|{tmp_path.resolve()}|2")
