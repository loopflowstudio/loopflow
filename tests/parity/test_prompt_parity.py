import pytest

from tests.parity.conftest import (
    get_python_prompt,
    get_rust_prompt,
    normalize_prompt,
    run_git,
)


@pytest.mark.parametrize("fixture_repo", ["minimal"], indirect=True)
def test_minimal_debug(fixture_repo, rust_binary):
    python_prompt = get_python_prompt(fixture_repo, ["run", "debug", "--dry-run"])
    rust_prompt = get_rust_prompt(fixture_repo, rust_binary, ["run", "debug", "--dry-run"])

    assert normalize_prompt(python_prompt, fixture_repo) == normalize_prompt(
        rust_prompt, fixture_repo
    )


@pytest.mark.parametrize("fixture_repo", ["with-diff"], indirect=True)
def test_with_diff_implement(fixture_repo, rust_binary):
    run_git(fixture_repo, ["checkout", "-b", "feature"])
    (fixture_repo / "src" / "main.py").write_text("print('modified')\n")
    run_git(fixture_repo, ["add", "src/main.py"])
    run_git(fixture_repo, ["commit", "-m", "Modify main"])

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
