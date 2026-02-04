import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Generator

import pytest

FIXTURES_DIR = Path(__file__).parent / "fixtures"


@pytest.fixture(scope="session")
def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


@pytest.fixture(scope="session")
def rust_binary(repo_root: Path) -> Path:
    """Path to Rust lf binary. Builds if needed."""
    binary = repo_root / "target" / "release" / "lf"
    if not binary.exists():
        subprocess.run(
            ["cargo", "build", "--release", "-p", "lf"],
            cwd=repo_root,
            check=True,
        )
    return binary


@pytest.fixture
def fixture_repo(request, tmp_path: Path) -> Generator[Path, None, None]:
    """Copy a fixture to a temp dir, initialize git if needed."""
    fixture_name = request.param
    fixture_src = FIXTURES_DIR / fixture_name
    fixture_dst = tmp_path / fixture_name

    shutil.copytree(fixture_src, fixture_dst)

    git_dir = fixture_dst / ".git"
    if not git_dir.exists():
        subprocess.run(["git", "init"], cwd=fixture_dst, check=True, capture_output=True)
        subprocess.run(
            ["git", "config", "user.email", "test@test.com"],
            cwd=fixture_dst,
            check=True,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test"],
            cwd=fixture_dst,
            check=True,
            capture_output=True,
        )
        subprocess.run(
            ["git", "checkout", "-b", "main"],
            cwd=fixture_dst,
            check=True,
            capture_output=True,
        )
        subprocess.run(["git", "add", "-A"], cwd=fixture_dst, check=True, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "Initial"],
            cwd=fixture_dst,
            check=True,
            capture_output=True,
        )

    yield fixture_dst


def get_python_prompt(repo: Path, args: list[str]) -> str:
    """Run Python lf and capture dry-run prompt."""
    result = subprocess.run(
        [sys.executable, "-m", "loopflow.lf.cli", *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Python lf failed: {result.stderr}")
    return result.stdout


def get_rust_prompt(repo: Path, binary: Path, args: list[str]) -> str:
    """Run Rust lf and capture dry-run prompt."""
    result = subprocess.run(
        [str(binary), *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Rust lf failed: {result.stderr}")
    return result.stdout


def normalize_prompt(text: str, repo: Path) -> str:
    """Normalize prompt for comparison."""
    normalized = text.replace("\r\n", "\n")
    normalized = normalized.replace(str(repo), "/REPO")
    normalized = normalized.replace(str(repo.resolve()), "/REPO")

    normalized = re.sub(
        r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?",
        "TIMESTAMP",
        normalized,
    )
    normalized = re.sub(r"\d{4}-\d{2}-\d{2}", "DATE", normalized)

    normalized = re.sub(r"[a-f0-9]{40}", "SHA", normalized)
    normalized = re.sub(r"[a-f0-9]{7,8}(?![a-f0-9])", "SHORTSHA", normalized)

    normalized = "\n".join(line.rstrip() for line in normalized.splitlines())

    return normalized.strip() + "\n"
