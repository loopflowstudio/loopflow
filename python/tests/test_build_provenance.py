"""Application provenance follows source edits and the selected Git ref."""

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture
def provenance_workspace(tmp_path):
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("LF_", "LOOPFLOW_", "CARGO_", "SCCACHE_"))
        and key not in {"RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS"}
    }
    env["RUSTC_WRAPPER"] = ""

    def run(*args: str, cwd=tmp_path) -> str:
        return subprocess.run(
            args,
            cwd=cwd,
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
    )
    shutil.copyfile(ROOT / "rust/loopflow/build.rs", app / "build.rs")
    (app / "src/main.rs").write_text(
        'fn main() { println!("{}|{}|{}", env!("LOOPFLOW_BUILD_VERSION"), '
        'env!("LOOPFLOW_BUILD_SOURCE_ROOT"), loopflow_storage::value()); }\n'
    )
    (lower / "Cargo.toml").write_text(
        '[package]\nname="loopflow-storage"\nversion="0.1.0"\n'
        'edition="2021"\nlinks="loopflow_storage"\n'
    )
    (lower / "build.rs").write_text(
        'fn main() { println!("cargo:rerun-if-changed=build.rs"); println!("cargo:unmerged="); }\n'
    )
    record = lower / "src/lib.rs"
    record.write_text("pub fn value() -> u8 { 1 }\n")
    run("git", "init", "-q")
    run("git", "config", "core.logAllRefUpdates", "true")
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

    def build(directory=tmp_path) -> tuple[list[str], str]:
        raw = run(
            "cargo", "build", "--offline", "--jobs", "1", "--message-format=json", cwd=directory
        )
        artifacts = [json.loads(line) for line in raw.splitlines() if line.startswith("{")]
        changed = [
            item["target"]["name"]
            for item in artifacts
            if item.get("reason") == "compiler-artifact" and not item["fresh"]
        ]
        return changed, run(
            str(directory / "target/debug/provenance-fixture"), cwd=directory
        ).strip()

    return tmp_path, record, run, build


def test_initial_lower_crate_edit_refreshes_clean_application_provenance(provenance_workspace):
    tmp_path, record, _, build = provenance_workspace
    _, clean = build()
    assert ".dirty" not in clean
    assert clean.endswith(f"|{tmp_path.resolve()}|1")
    record.write_text("pub fn value() -> u8 { 2 }\n")
    changed, dirty = build()
    assert "provenance-fixture" in changed
    assert ".dirty" in dirty, dirty
    assert dirty.endswith(f"|{tmp_path.resolve()}|2")


@pytest.mark.parametrize("linked", [False, True])
def test_packed_branch_ignores_sibling_commits_and_tracks_own_commits(provenance_workspace, linked):
    root, _, run, build = provenance_workspace
    sibling = root.parent / f"{root.name}-sibling"
    run("git", "branch", "-m", "jack/selected")
    run("git", "worktree", "add", "-qb", "jack/sibling", str(sibling))
    branch = "jack/selected"
    if linked:
        root, sibling = sibling, root
        branch = "jack/sibling"

    run("git", "pack-refs", "--all", "--prune", cwd=root)
    common = Path(
        run("git", "rev-parse", "--path-format=absolute", "--git-common-dir", cwd=root).strip()
    )
    assert not (common / "refs/heads" / branch).exists()
    _, initial = build(root)
    assert build(root) == ([], initial)
    run(
        "git",
        "-C",
        str(sibling),
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.com",
        "commit",
        "--allow-empty",
        "-qm",
        "sibling",
    )
    assert build(root) == ([], initial)
    # Ref updates from another checkout must also refresh this checkout's HEAD.
    sibling_revision = run("git", "rev-parse", "HEAD", cwd=sibling).strip()
    run("git", "update-ref", f"refs/heads/{branch}", sibling_revision, cwd=sibling)
    changed, updated = build(root)
    assert "provenance-fixture" in changed
    assert sibling_revision[:9] in updated
    run(
        "git",
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.com",
        "commit",
        "--allow-empty",
        "-qm",
        "selected",
        cwd=root,
    )
    changed, current = build(root)
    assert "provenance-fixture" in changed
    assert current != updated
    assert run("git", "rev-parse", "--short=9", "HEAD", cwd=root).strip() in current
    assert build(root) == ([], current)
