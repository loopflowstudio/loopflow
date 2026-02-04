# Rust Testing and Rollout: First Steps

## Problem

Rust `lf` claims ops parity with Python, but there's no automated way to verify this. The existing `01b-testing-and-rollout.md` design is comprehensive but hasn't been implemented. We need confidence that Rust produces identical behavior before shipping it to users.

The gap isn't test coverage within Rust (that's decent). The gap is **cross-implementation parity tests** that prove Rust and Python produce the same outputs for the same inputs.

## Approach

Start with the highest-leverage verification: **prompt parity tests**. If Rust and Python produce identical prompts for identical inputs, the rest of the differences are mechanical (git operations, subprocess spawning) and can be trusted.

Implement parity tests as Python tests that call both implementations and compare outputs. This is simpler than maintaining separate test suites and ensures we're testing the actual user-facing behavior.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Port all Python tests to Rust | Comprehensive coverage | Tests internal implementation, not parity; massive effort |
| Golden file tests only | Simple to implement | Requires manual updates; doesn't prove parity |
| Manual QA | Catches regressions | Doesn't scale; human error |
| Ship and fix bugs | Fast to ship | Damages user trust; hard to regain |

Parity tests win: they directly verify what users care about (identical behavior) with minimal infrastructure.

## Key decisions

**1. Python orchestrates tests, not Rust.**

Parity tests need to run both implementations. Python is already set up for testing (`pytest`, fixtures, etc.). Adding a Rust test harness that calls Python would be more complex. Python calls Rust via subprocess, matching real usage.

> Wave principle (Rust roadmap): "UX invariants: prompts, flows, directions, and artifact paths must not change."

**2. Test `--dry-run` output, not agent behavior.**

Both implementations have `--dry-run` flags that output the assembled prompt without launching an agent. This is the critical parity surface. Agent spawning is tested separately (and mostly delegates to the CLIs anyway).

**3. Start with three fixtures, expand later.**

| Fixture | What it tests |
|---------|---------------|
| `minimal` | Empty repo with `.lf/` config, no changes |
| `with-diff` | Repo with uncommitted changes, tests diff inclusion |
| `with-flow` | Repo with flow files, tests flow parsing |

These cover the core prompt assembly paths. Directions, waves, and areas can be added incrementally.

**4. Normalize before comparing.**

Prompts may differ in:
- Timestamps
- Absolute paths
- Whitespace in certain positions

The normalization function strips these before comparison, allowing the test to focus on semantic equivalence.

**5. Wire into existing CI, not new workflow.**

Add parity tests to the existing Python test suite. They run alongside other tests. No new CI configuration needed initially.

## Scope

**In scope:**
- `tests/parity/` directory with fixtures and test runner
- `test_prompt_parity.py` that calls both implementations
- Three initial fixtures (minimal, with-diff, with-flow)
- Normalization function for prompt comparison
- Documentation in `TESTING.md`

**Out of scope:**
- E2E workflow tests (commit, PR, land cycles) - deferred
- `lfd` parity tests - separate doc (02-lfd-primary)
- PyPI binary bundling - separate doc (04-distribution)
- Missing CLI flags (`--lfdocs`, `--diff-mode`) - implementation work, not testing

## Implementation

### Directory Structure

```
tests/
├── parity/
│   ├── __init__.py
│   ├── conftest.py           # Fixtures and helpers
│   ├── test_prompt_parity.py # Main test file
│   └── fixtures/
│       ├── minimal/          # Git repo with .lf/
│       │   ├── .git/
│       │   ├── .lf/
│       │   │   └── config.yaml
│       │   └── README.md
│       ├── with-diff/        # Repo with uncommitted changes
│       │   ├── .git/
│       │   ├── .lf/
│       │   │   └── config.yaml
│       │   ├── README.md
│       │   └── src/
│       │       └── main.py   # Modified file
│       └── with-flow/        # Repo with flow files
│           ├── .git/
│           ├── .lf/
│           │   ├── config.yaml
│           │   └── flows/
│           │       └── implement/
│           │           ├── flow.yaml
│           │           └── implement.md
│           └── README.md
```

### conftest.py

```python
import os
import shutil
import subprocess
from pathlib import Path
from typing import Generator

import pytest

FIXTURES_DIR = Path(__file__).parent / "fixtures"

@pytest.fixture
def rust_binary() -> Path:
    """Path to Rust lf binary. Builds if needed."""
    binary = Path(__file__).parents[3] / "target" / "release" / "lf"
    if not binary.exists():
        # Build Rust binary
        subprocess.run(
            ["cargo", "build", "--release", "-p", "lf"],
            cwd=Path(__file__).parents[3],
            check=True,
        )
    return binary

@pytest.fixture
def fixture_repo(request, tmp_path) -> Generator[Path, None, None]:
    """Copy a fixture to temp dir, initialize git if needed."""
    fixture_name = request.param
    fixture_src = FIXTURES_DIR / fixture_name
    fixture_dst = tmp_path / fixture_name

    shutil.copytree(fixture_src, fixture_dst)

    # Initialize git if .git doesn't exist
    git_dir = fixture_dst / ".git"
    if not git_dir.exists():
        subprocess.run(["git", "init"], cwd=fixture_dst, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=fixture_dst, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test"], cwd=fixture_dst, check=True, capture_output=True)
        subprocess.run(["git", "add", "-A"], cwd=fixture_dst, check=True, capture_output=True)
        subprocess.run(["git", "commit", "-m", "Initial"], cwd=fixture_dst, check=True, capture_output=True)

    yield fixture_dst


def get_python_prompt(repo: Path, step: str, flags: list[str] = None) -> str:
    """Run Python lf and capture dry-run prompt."""
    flags = flags or []
    result = subprocess.run(
        ["python", "-m", "loopflow.lf.cli", "step", step, "--dry-run", *flags],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Python lf failed: {result.stderr}")
    return result.stdout


def get_rust_prompt(repo: Path, step: str, binary: Path, flags: list[str] = None) -> str:
    """Run Rust lf and capture dry-run prompt."""
    flags = flags or []
    result = subprocess.run(
        [str(binary), "step", step, "--dry-run", *flags],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Rust lf failed: {result.stderr}")
    return result.stdout


def normalize_prompt(text: str, repo: Path) -> str:
    """Normalize prompt for comparison."""
    # Remove absolute paths
    text = text.replace(str(repo), "/REPO")

    # Normalize timestamps (ISO 8601 format)
    import re
    text = re.sub(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}', 'TIMESTAMP', text)
    text = re.sub(r'\d{4}-\d{2}-\d{2}', 'DATE', text)

    # Normalize git SHAs
    text = re.sub(r'[a-f0-9]{40}', 'SHA', text)
    text = re.sub(r'[a-f0-9]{7,8}(?![a-f0-9])', 'SHORTSHA', text)

    # Normalize trailing whitespace
    text = '\n'.join(line.rstrip() for line in text.splitlines())

    return text
```

### test_prompt_parity.py

```python
import pytest

from tests.parity.conftest import (
    get_python_prompt,
    get_rust_prompt,
    normalize_prompt,
)


@pytest.mark.parametrize("fixture_repo", ["minimal"], indirect=True)
def test_minimal_debug(fixture_repo, rust_binary):
    """Minimal repo produces identical debug prompt."""
    python_prompt = get_python_prompt(fixture_repo, "debug")
    rust_prompt = get_rust_prompt(fixture_repo, "debug", rust_binary)

    assert normalize_prompt(python_prompt, fixture_repo) == normalize_prompt(rust_prompt, fixture_repo)


@pytest.mark.parametrize("fixture_repo", ["with-diff"], indirect=True)
def test_with_diff_debug(fixture_repo, rust_binary):
    """Repo with changes includes diff in prompt."""
    # Make a change first
    (fixture_repo / "src" / "main.py").write_text("print('modified')\n")

    python_prompt = get_python_prompt(fixture_repo, "debug")
    rust_prompt = get_rust_prompt(fixture_repo, "debug", rust_binary)

    # Both should include the diff
    assert "modified" in python_prompt
    assert "modified" in rust_prompt

    assert normalize_prompt(python_prompt, fixture_repo) == normalize_prompt(rust_prompt, fixture_repo)


@pytest.mark.parametrize("fixture_repo", ["with-diff"], indirect=True)
def test_with_diff_no_diff_flag(fixture_repo, rust_binary):
    """--no-diff excludes diff from prompt."""
    (fixture_repo / "src" / "main.py").write_text("print('modified')\n")

    python_prompt = get_python_prompt(fixture_repo, "debug", ["--no-diff"])
    rust_prompt = get_rust_prompt(fixture_repo, "debug", rust_binary, ["--no-diff"])

    # Neither should include the diff
    assert "modified" not in python_prompt or "<lf:diff>" not in python_prompt
    assert "modified" not in rust_prompt or "<lf:diff>" not in rust_prompt

    assert normalize_prompt(python_prompt, fixture_repo) == normalize_prompt(rust_prompt, fixture_repo)


@pytest.mark.parametrize("fixture_repo", ["with-flow"], indirect=True)
def test_with_flow_step(fixture_repo, rust_binary):
    """Flow step produces identical prompt."""
    python_prompt = get_python_prompt(fixture_repo, "implement")
    rust_prompt = get_rust_prompt(fixture_repo, "implement", rust_binary)

    assert normalize_prompt(python_prompt, fixture_repo) == normalize_prompt(rust_prompt, fixture_repo)
```

### Fixture: minimal

```yaml
# tests/parity/fixtures/minimal/.lf/config.yaml
model: claude
```

```markdown
# tests/parity/fixtures/minimal/README.md
# Minimal Test Repo

A minimal repo for parity testing.
```

### Fixture: with-diff

```yaml
# tests/parity/fixtures/with-diff/.lf/config.yaml
model: claude
```

```python
# tests/parity/fixtures/with-diff/src/main.py
print('hello')
```

### Fixture: with-flow

```yaml
# tests/parity/fixtures/with-flow/.lf/config.yaml
model: claude
```

```yaml
# tests/parity/fixtures/with-flow/.lf/flows/implement/flow.yaml
steps:
  - implement
```

```markdown
# tests/parity/fixtures/with-flow/.lf/flows/implement/implement.md
Implement the requested feature.

Focus on:
- Clean code
- Tests
- Documentation
```

## Done when

```bash
# All parity tests pass
pytest tests/parity/ -v

# Output shows:
# tests/parity/test_prompt_parity.py::test_minimal_debug PASSED
# tests/parity/test_prompt_parity.py::test_with_diff_debug PASSED
# tests/parity/test_prompt_parity.py::test_with_diff_no_diff_flag PASSED
# tests/parity/test_prompt_parity.py::test_with_flow_step PASSED
```

Observable: Running `pytest tests/parity/` produces green checks for all fixtures, proving Rust and Python generate identical prompts.
