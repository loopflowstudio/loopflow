import subprocess

import pytest

from tests.parity.conftest import get_python_prompt, get_rust_prompt, normalize_prompt


@pytest.mark.parametrize("fixture_repo", ["minimal"], indirect=True)
def test_minimal_debug(fixture_repo, rust_binary):
    python_prompt = get_python_prompt(fixture_repo, ["run", "debug", "--dry-run"])
    rust_prompt = get_rust_prompt(fixture_repo, rust_binary, ["run", "debug", "--dry-run"])

    assert normalize_prompt(python_prompt, fixture_repo) == normalize_prompt(
        rust_prompt, fixture_repo
    )


@pytest.mark.parametrize("fixture_repo", ["with-diff"], indirect=True)
def test_with_diff_implement(fixture_repo, rust_binary):
    subprocess.run(
        ["git", "checkout", "-b", "feature"],
        cwd=fixture_repo,
        check=True,
        capture_output=True,
    )
    (fixture_repo / "src" / "main.py").write_text("print('modified')\n")
    subprocess.run(
        ["git", "add", "src/main.py"],
        cwd=fixture_repo,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Modify main"],
        cwd=fixture_repo,
        check=True,
        capture_output=True,
    )

    python_prompt = get_python_prompt(fixture_repo, ["run", "implement", "--dry-run"])
    rust_prompt = get_rust_prompt(fixture_repo, rust_binary, ["run", "implement", "--dry-run"])

    assert "modified" in python_prompt
    assert "modified" in rust_prompt

    assert normalize_prompt(python_prompt, fixture_repo) == normalize_prompt(
        rust_prompt, fixture_repo
    )


@pytest.mark.parametrize("fixture_repo", ["with-flow"], indirect=True)
def test_with_flow_step(fixture_repo, rust_binary):
    python_prompt = get_python_prompt(fixture_repo, ["run", "implement", "--dry-run"])
    rust_prompt = get_rust_prompt(fixture_repo, rust_binary, ["run", "implement", "--dry-run"])

    assert normalize_prompt(python_prompt, fixture_repo) == normalize_prompt(
        rust_prompt, fixture_repo
    )
