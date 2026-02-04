"""Ops parity tests comparing Python and Rust traces."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _create_test_repo() -> Path:
    """Create a minimal git repo for testing."""
    temp_dir = Path(tempfile.mkdtemp(prefix="lf-ops-parity-"))
    subprocess.run(["git", "init"], cwd=temp_dir, check=True, capture_output=True)
    subprocess.run(
        ["git", "config", "user.email", "test@example.com"],
        cwd=temp_dir,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Test"],
        cwd=temp_dir,
        check=True,
        capture_output=True,
    )

    # Create .lf/steps/commit.md
    steps_dir = temp_dir / ".lf" / "steps"
    steps_dir.mkdir(parents=True)
    (steps_dir / "commit.md").write_text("# Commit\n\nGenerate a commit message.")

    # Initial commit
    (temp_dir / "file.txt").write_text("initial")
    subprocess.run(["git", "add", "-A"], cwd=temp_dir, check=True, capture_output=True)
    subprocess.run(
        ["git", "commit", "-m", "init"], cwd=temp_dir, check=True, capture_output=True
    )

    # Make a change to test commit
    (temp_dir / "file.txt").write_text("changed")

    return temp_dir


def _python_commit_trace(repo: Path, add: bool = True, lint: bool = True, push: bool = False) -> dict:
    """Run Python lf ops commit with tracing and return trace."""
    env = os.environ.copy()
    env["LF_TRACE"] = "1"

    # Use the repo's venv lf binary, not the global one
    lf_binary = _repo_root() / ".venv" / "bin" / "lf"
    args = [str(lf_binary), "ops", "commit"]
    if not add:
        args.append("--no-add")
    if not lint:
        args.append("--no-lint")
    if push:
        args.append("--push")

    result = subprocess.run(
        args,
        cwd=repo,
        capture_output=True,
        text=True,
        env=env,
    )

    # Parse JSON from output
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        raise RuntimeError(f"Failed to parse Python trace: {result.stdout}\nstderr: {result.stderr}")


def _rust_commit_trace(add: bool = True, lint: bool = True, push: bool = False) -> dict:
    """Run Rust commit trace and return trace."""
    root = _repo_root()

    # Build a small Rust binary that calls commit_workflow_traced
    # For now, use cargo run with a test binary
    # Note: Rust's bool::parse expects lowercase "true"/"false"
    result = subprocess.run(
        [
            "cargo",
            "run",
            "-p",
            "loopflow-ops",
            "--example",
            "commit_trace",
            "--",
            f"--add={str(add).lower()}",
            f"--lint={str(lint).lower()}",
            f"--push={str(push).lower()}",
        ],
        cwd=root,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        raise RuntimeError(f"Rust trace failed: {result.stderr}")

    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        raise RuntimeError(f"Failed to parse Rust trace: {result.stdout}")


def _normalize_trace(trace: dict) -> dict:
    """Normalize a trace for comparison."""
    # Remove any timestamps or varying fields
    normalized = {
        "command": trace["command"],
        "options": trace["options"],
        "operations": [],
    }

    for op in trace["operations"]:
        normalized_op = {"op": op["op"]}
        if "result" in op:
            normalized_op["result"] = op["result"]
        # Skip prompt_hash as it may differ
        if "message" in op:
            # Normalize message to just check prefix
            msg = op["message"]
            if msg.startswith("lf test:"):
                normalized_op["message"] = "lf test: <generated>"
            else:
                normalized_op["message"] = msg
        if "model" in op:
            normalized_op["model"] = op["model"]
        normalized["operations"].append(normalized_op)

    return normalized


def test_commit_trace_parity() -> None:
    """Test that Python and Rust commit traces match."""
    repo = _create_test_repo()
    try:
        py_trace = _python_commit_trace(repo, add=True, lint=True, push=False)
        rs_trace = _rust_commit_trace(add=True, lint=True, push=False)

        py_normalized = _normalize_trace(py_trace)
        rs_normalized = _normalize_trace(rs_trace)

        assert py_normalized == rs_normalized, (
            f"Trace mismatch:\nPython: {json.dumps(py_normalized, indent=2)}\n"
            f"Rust: {json.dumps(rs_normalized, indent=2)}"
        )
    finally:
        shutil.rmtree(repo)


def test_commit_with_push_trace_parity() -> None:
    """Test commit --push traces match."""
    repo = _create_test_repo()
    try:
        py_trace = _python_commit_trace(repo, add=True, lint=True, push=True)
        rs_trace = _rust_commit_trace(add=True, lint=True, push=True)

        py_normalized = _normalize_trace(py_trace)
        rs_normalized = _normalize_trace(rs_trace)

        assert py_normalized == rs_normalized, (
            f"Trace mismatch:\nPython: {json.dumps(py_normalized, indent=2)}\n"
            f"Rust: {json.dumps(rs_normalized, indent=2)}"
        )
    finally:
        shutil.rmtree(repo)
