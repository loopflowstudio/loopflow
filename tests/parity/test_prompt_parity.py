from __future__ import annotations

import shutil
import subprocess
import tempfile
from pathlib import Path

import yaml

from loopflow.lf.context import (
    ContextConfig,
    DiffMode,
    FilesetConfig,
    format_prompt,
    gather_prompt_components,
)
from loopflow.lf.directions import resolve_directions


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _golden_cases() -> list[Path]:
    root = _repo_root() / "tests" / "goldens"
    return sorted(path for path in root.glob("*.yaml") if path.is_file())


def _normalize_prompt(prompt: str, repo_root: Path) -> str:
    return prompt.replace("\r\n", "\n").replace(str(repo_root), "<REPO>").strip()


def _python_prompt(case: dict, repo_root: Path) -> str:
    directions = case.get("directions") or []
    direction_objs = resolve_directions(repo_root, directions) if directions else None

    diff = case.get("diff", False)
    diff_files = case.get("diff_files", False)
    if diff:
        diff_mode = DiffMode.DIFF
    elif diff_files:
        diff_mode = DiffMode.FILES
    else:
        diff_mode = DiffMode.NONE

    context_config = ContextConfig(
        diff_mode=diff_mode,
        files=FilesetConfig(paths=[], exclude=[], parent_docs=False),
        area=case.get("area"),
        wave=case.get("wave"),
        include_loopflow_doc=False,
        clipboard=case.get("clipboard", False),
        budget_area=0,
        budget_docs=0,
        budget_diff=0,
    )

    components = gather_prompt_components(
        repo_root=repo_root,
        step=case.get("step"),
        inline=None,
        step_args=None,
        run_mode=case.get("run_mode"),
        direction=direction_objs,
        context_config=context_config,
        config=None,
    )

    return format_prompt(components)


def _build_rust_prompt_bin() -> Path:
    root = _repo_root()
    target = root / "target" / "debug" / "lf-prompt"
    if target.exists():
        return target

    subprocess.run(
        ["cargo", "build", "-p", "loopflow-engine", "--bin", "lf-prompt"],
        cwd=root,
        check=True,
    )
    if not target.exists():
        raise RuntimeError("lf-prompt binary not found after build")
    return target


def _rust_prompt(case: dict, repo_root: Path) -> str:
    binary = _build_rust_prompt_bin()

    cmd = [
        str(binary),
        "--repo",
        str(repo_root),
        f"--lfdocs={str(case.get('lfdocs', False)).lower()}",
        f"--diff-files={str(case.get('diff_files', False)).lower()}",
        f"--diff={str(case.get('diff', False)).lower()}",
        f"--clipboard={str(case.get('clipboard', False)).lower()}",
    ]

    if case.get("step"):
        cmd += ["--step", case["step"]]
    if case.get("run_mode"):
        cmd += ["--run-mode", case["run_mode"]]
    for direction in case.get("directions") or []:
        cmd += ["--direction", direction]
    if case.get("area"):
        cmd += ["--area", case["area"]]
    if case.get("wave"):
        cmd += ["--wave", case["wave"]]

    result = subprocess.run(
        cmd,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout


def _prepare_repo(fixture: Path) -> Path:
    temp_dir = Path(tempfile.mkdtemp(prefix="lf-parity-"))
    shutil.copytree(fixture, temp_dir, dirs_exist_ok=True)
    subprocess.run(["git", "init"], cwd=temp_dir, check=True, capture_output=True)
    subprocess.run(["git", "add", "-A"], cwd=temp_dir, check=True)
    subprocess.run(
        [
            "git",
            "-c",
            "user.name=Loopflow",
            "-c",
            "user.email=loopflow@example.com",
            "commit",
            "-m",
            "init",
        ],
        cwd=temp_dir,
        check=True,
        capture_output=True,
    )
    return temp_dir


def test_prompt_parity() -> None:
    root = _repo_root()
    for case_path in _golden_cases():
        case = yaml.safe_load(case_path.read_text())
        fixture = root / case["repo"]
        repo_root = _prepare_repo(fixture)
        try:
            py_prompt = _normalize_prompt(_python_prompt(case, repo_root), repo_root)
            rs_prompt = _normalize_prompt(_rust_prompt(case, repo_root), repo_root)
        finally:
            shutil.rmtree(repo_root)

        assert py_prompt == rs_prompt, f"prompt mismatch for {case['name']}"
