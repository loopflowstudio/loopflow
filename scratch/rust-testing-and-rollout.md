# 01b: Testing and Rollout

Verify Rust `lf` is feature-complete with Python `lf`, then roll out the transition.

## Context

Phase 1 claims Rust `lf` has ops parity with Python. Before moving to Phase 2 (service, distribution), we need:

1. Confidence that behavior matches (testing)
2. A plan to transition users (rollout)
3. A path to remove Python code (migration)

## Goal

**Testing:**
1. Audit Python tests → Rust test coverage
2. Establish prompt parity tests (same input → same prompt)
3. Golden file tests for context gathering
4. End-to-end workflow tests

**Rollout:**
5. Gradual transition from Python → Rust `lf`
6. PyPI package ships Rust binaries (like uv, ruff)
7. Eventually remove Python implementation, keep thin PyO3 bindings

---

## 1. Unit Test Parity

### Current State

| Python Test File | Tests | Rust Equivalent | Status |
|------------------|-------|-----------------|--------|
| **Core Engine** ||||
| `test_config.py` | 40 | `config_tests.rs` + inline | ✅ Good |
| `test_context.py` | 42 | `context_tests.rs` + `prompt.rs` | ✅ Good |
| `test_flows.py` | 7 | `flow_tests.rs` + `flow.rs` | ✅ Covered |
| `test_tokens.py` | 18 | `token_tests.rs` + `prompt.rs` | ⚠️ Partial |
| `test_launcher.py` | 46 | `agent.rs` (8 tests) | ⚠️ Partial |
| `test_directions.py` | 22 | `context_tests.rs` + `prompt.rs` | ✅ Covered |
| `test_frontmatter.py` | 19 | `flow.rs` inline | ✅ Covered |
| **Git Operations** ||||
| `test_git.py` | 15 | `git_tests.rs` (20) + `git.rs` (13) | ✅ Good |
| `test_worktrees.py` | 17 | `git_tests.rs` worktree tests | ⚠️ Partial |
| `test_branch_names.py` | 13 | `naming.rs` (4) | ⚠️ Partial |
| `test_commit.py` | 3 | `loopflow-ops` | ⚠️ Minimal |
| **Files & Collection** ||||
| `test_files.py` | 35 | `prompt.rs` gather_files | ⚠️ Partial |
| `test_collector.py` | 21 | None | ❌ Missing |
| **lfd Daemon** ||||
| `test_lfd.py` | 85 | `store/mod.rs` + `scheduler.rs` | ⚠️ Partial |
| `test_lfd_cli.py` | 1 | None | ❌ Missing |
| `test_protocol_v1.py` | 2 | None | ❌ Missing |
| `test_proto_fixtures.py` | 29 | None | ❌ Missing |
| **Skills/Design** ||||
| `test_skills.py` | 31 | None | N/A (Python-only) |
| `test_design.py` | 21 | None | ❌ Missing |
| **Ops Commands** ||||
| `test_add.py` | 4 | None | ❌ Missing |
| `test_cli_ops.py` | 3 | None | ❌ Missing |
| `test_next.py` | 10 | None | ❌ Missing |
| `test_cp.py` | 7 | None | N/A (deferred) |
| `test_publish.py` | 5 | None | N/A (deferred) |
| **Summaries** ||||
| `test_summarize.py` | 16 | None | N/A (deferred) |
| `test_summarize_integration.py` | 23 | None | N/A (deferred) |
| **Other** ||||
| `test_naming.py` | 19 | `naming.rs` (4) | ⚠️ Partial |
| `test_messages.py` | 9 | `messages.rs` (2) | ⚠️ Partial |
| `test_logging.py` | 15 | None | ❌ Missing |
| `test_pr_poller.py` | 10 | None | N/A (lfd feature) |
| `test_git_hooks.py` | 10 | None | N/A (deferred) |
| `test_roadmap.py` | 18 | None | N/A (Python-only) |

### Action Items

**High Priority (blocking parity claim):**

1. **`test_launcher.py` → `agent.rs`**: Add tests for all command-building edge cases
   - Interactive vs auto mode combinations
   - All model variants (claude:opus, gemini:2.5-pro, codex:o3)
   - Permission skip flags
   - Environment variable handling

2. **`test_tokens.py` → `prompt.rs`**: Add TokenNode/TokenTree parity
   - Hierarchical token analysis
   - Category-based aggregation
   - Budget trimming strategies

3. **`test_files.py` → `prompt.rs`**: Add file gathering edge cases
   - Gitignore interaction
   - Binary detection
   - Image handling
   - Symlink behavior

4. **`test_worktrees.py` → `git_tests.rs`**: Add worktree edge cases
   - Stacked worktree detection
   - Base commit tracking
   - Merge detection

**Medium Priority (ops completeness):**

5. **`test_next.py`**: Add to `loopflow-ops`
6. **`test_commit.py`**: Expand `loopflow-ops` commit tests
7. **`test_branch_names.py` → `naming.rs`**: Full template expansion tests

**Low Priority (can defer):**

8. `test_collector.py` - Context collection internals
9. `test_design.py` - Design doc loading
10. `test_logging.py` - Logging internals

**N/A (intentionally not porting):**

- `test_skills.py` - Python skill system, not in Rust scope
- `test_roadmap.py` - Python roadmap CLI, not in Rust scope
- `test_summarize*.py` - Deferred to 02b-summarize
- `test_cp.py`, `test_publish.py` - Deferred features

---

## 2. Prompt Parity Tests

The most critical verification: given identical inputs, Rust produces the same prompt as Python.

### Approach

Create test fixtures with known repo states, run both implementations with `--dry-run` or equivalent, compare output.

```
tests/
└── parity/
    ├── fixtures/
    │   ├── empty-repo/           # Minimal repo
    │   ├── basic-changes/        # Repo with uncommitted changes
    │   ├── with-worktree/        # Repo with active worktree
    │   ├── with-wave/            # Repo with wave config
    │   ├── with-directions/      # Repo with direction files
    │   └── complex/              # All features combined
    ├── expected/
    │   ├── empty-repo-debug.txt
    │   ├── basic-changes-debug.txt
    │   └── ...
    └── test_prompt_parity.py     # Runs both, compares
```

### Test Cases

| Fixture | Step | Flags | Validates |
|---------|------|-------|-----------|
| empty-repo | debug | (none) | Minimal context gathering |
| basic-changes | debug | (none) | Diff inclusion |
| basic-changes | debug | `--no-diff` | Diff exclusion |
| with-directions | debug | `-d coding` | Direction loading |
| with-wave | debug | `-w mywave` | Wave context |
| complex | implement | `-c -d coding` | Full context assembly |

### Initial Fixture Set (v1)

Start small to get green parity runs quickly, then expand:

| Fixture | What it tests |
|---------|---------------|
| minimal | Empty repo with `.lf/` config |
| with-diff | Dirty working tree; diff inclusion |
| with-flow | Flow parsing and step prompt |

For v1, run `lf step <name> --dry-run` in both implementations so the parity surface stays narrow and avoids agent launch differences.

### Implementation

```python
# tests/parity/test_prompt_parity.py
import subprocess
import tempfile
from pathlib import Path

FIXTURES = Path(__file__).parent / "fixtures"

def get_python_prompt(fixture: Path, step: str, flags: list[str]) -> str:
    """Run Python lf and capture the prompt it would send."""
    result = subprocess.run(
        ["python", "-m", "loopflow.lf.cli", "run", step, "--dry-run", *flags],
        cwd=fixture,
        capture_output=True,
        text=True,
    )
    return result.stdout

def get_rust_prompt(fixture: Path, step: str, flags: list[str]) -> str:
    """Run Rust lf and capture the prompt it would send."""
    result = subprocess.run(
        ["lf", "run", step, "--dry-run", *flags],
        cwd=fixture,
        capture_output=True,
        text=True,
    )
    return result.stdout

def test_empty_repo_debug():
    fixture = FIXTURES / "empty-repo"
    python_prompt = get_python_prompt(fixture, "debug", [])
    rust_prompt = get_rust_prompt(fixture, "debug", [])
    assert python_prompt == rust_prompt

def test_basic_changes_debug():
    fixture = FIXTURES / "basic-changes"
    python_prompt = get_python_prompt(fixture, "debug", [])
    rust_prompt = get_rust_prompt(fixture, "debug", [])
    assert python_prompt == rust_prompt

# ... etc
```

### Handling Expected Differences

Some differences are acceptable:
- Timestamps (if any)
- Absolute paths (normalize to relative)
- Ordering of unordered sections

Create a `normalize_prompt(text)` function to handle these before comparison.

---

## 3. Golden File Tests

For context gathering internals, use golden files that capture expected output.

### Structure

```
rust/loopflow-engine/tests/
└── golden/
    ├── context/
    │   ├── minimal.yaml          # Input config
    │   ├── minimal.expected.md   # Expected prompt
    │   ├── with-diff.yaml
    │   ├── with-diff.expected.md
    │   └── ...
    └── golden_tests.rs
```

### Test Implementation

```rust
// rust/loopflow-engine/tests/golden_tests.rs

use std::fs;
use std::path::Path;

fn run_golden_test(name: &str) {
    let golden_dir = Path::new("tests/golden/context");
    let config_path = golden_dir.join(format!("{}.yaml", name));
    let expected_path = golden_dir.join(format!("{}.expected.md", name));

    let config: TestConfig = serde_yaml::from_str(
        &fs::read_to_string(&config_path).unwrap()
    ).unwrap();

    let actual = gather_and_format_prompt(&config);
    let expected = fs::read_to_string(&expected_path).unwrap();

    if actual != expected {
        // Write actual for easy update
        fs::write(golden_dir.join(format!("{}.actual.md", name)), &actual).unwrap();
        panic!("Golden mismatch for {}. See .actual.md file.", name);
    }
}

#[test]
fn golden_minimal() { run_golden_test("minimal"); }

#[test]
fn golden_with_diff() { run_golden_test("with-diff"); }

#[test]
fn golden_with_directions() { run_golden_test("with-directions"); }

#[test]
fn golden_with_wave() { run_golden_test("with-wave"); }

#[test]
fn golden_full_context() { run_golden_test("full-context"); }
```

### Updating Goldens

```bash
# Regenerate all golden files from Python (source of truth)
python scripts/update_goldens.py
```

---

## 4. End-to-End Workflow Tests

Verify complete workflows work in Rust.

### Workflow: Branch → Commit → PR → Land

```bash
#!/bin/bash
# tests/e2e/test_full_cycle.sh

set -e

REPO=$(mktemp -d)
cd "$REPO"
git init
git commit --allow-empty -m "Initial commit"

# Create worktree
lf ops wt create test-feature

# Make changes
cd ../test-feature
echo "change" > file.txt
git add file.txt

# Commit (skip agent for test)
lf ops commit -m "Add file"

# Verify commit exists
git log --oneline | grep "Add file"

# PR (would need mock gh or skip)
# lf ops pr

# Land (local mode)
# lf ops land --local

echo "✓ Workflow complete"
```

### Workflow: Rebase with Conflicts

```bash
#!/bin/bash
# tests/e2e/test_rebase_conflict.sh

set -e

# Setup repo with divergent branches
# ... setup ...

# Attempt rebase
lf ops rebase || true

# Verify conflict state
git status | grep "rebase in progress"

# Abort for cleanup
git rebase --abort

echo "✓ Conflict detection works"
```

### CI Integration

```yaml
# .github/workflows/e2e.yml
name: E2E Tests

on: [push, pull_request]

jobs:
  e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build Rust
        run: cargo build --release -p lf

      - name: Run E2E tests
        run: |
          export PATH="$PWD/target/release:$PATH"
          ./tests/e2e/test_full_cycle.sh
          ./tests/e2e/test_rebase_conflict.sh
```

---

## 5. CLI Feature Parity

### Missing Commands

| Command | Python | Rust | Decision |
|---------|--------|------|----------|
| `lf ops add` | ✅ | ❌ | **Add** - Simple, useful |
| `lf ops cp` | ✅ | ❌ | Defer - Web workflow |
| `lf ops doctor` | ✅ | ❌ | **Add** - Useful for debugging |
| `lf ops version` | ✅ | ❌ | **Add** - Use `lf -V` |
| `lf ops summarize` | ✅ | ❌ | Defer - See 02b-summarize |

### Missing Flags

| Flag | Python | Rust | Decision |
|------|--------|------|----------|
| `--lfdocs/--no-lfdocs` | ✅ | ❌ | **Add** - Affects context |
| `--diff-mode` | ✅ | ❌ | **Add** - Affects context |

### Implementation Priority

1. `--lfdocs` and `--diff-mode` flags (affects prompt parity)
2. `lf ops doctor` (helps users debug issues)
3. `lf ops add` (simple utility)
4. Defer: `cp`, `summarize`

---

## 6. Test Infrastructure

### Shared Test Fixtures

Create a shared fixture library for both Python and Rust tests:

```
tests/
├── fixtures/
│   ├── repos/
│   │   ├── minimal/          # Git repo with .lf/
│   │   ├── with-changes/     # Repo with dirty state
│   │   └── with-worktree/    # Repo + worktree
│   ├── configs/
│   │   ├── default.yaml
│   │   ├── with-directions.yaml
│   │   └── with-wave.yaml
│   └── steps/
│       ├── debug.md
│       └── implement.md
├── python/                   # Python-specific tests
├── rust/                     # Rust-specific tests (symlink to rust/*/tests)
└── parity/                   # Cross-implementation tests
```

### CI Matrix

```yaml
test:
  strategy:
    matrix:
      test-type: [unit-python, unit-rust, parity, e2e]
  steps:
    - run: ./scripts/test-${{ matrix.test-type }}.sh
```

---

## Done When

- [ ] All ⚠️ Partial test files have Rust equivalents
- [ ] Prompt parity tests pass for all fixtures
- [ ] Golden file tests established and passing
- [ ] E2E workflow tests pass
- [ ] `--lfdocs` and `--diff-mode` flags added to Rust
- [ ] `lf ops doctor` implemented
- [ ] CI runs all test types

## Dependencies

- Requires: 01-lf-ops-parity (the code being tested)
- Blocks: 03-service, 04-distribution (shouldn't ship until parity verified)

## Sequence

1. Add missing flags (`--lfdocs`, `--diff-mode`)
2. Set up prompt parity test infrastructure
3. Create fixtures and run first parity tests
4. Fix any differences found
5. Expand to golden files and E2E
6. Add to CI

---

## 7. Rollout Strategy

Transition from Python to Rust in phases, maintaining backwards compatibility.

### Phase A: Rust Behind Python (Current)

```
User runs: lf step debug
    ↓
Python CLI (loopflow.lf.cli)
    ↓
Calls lf-engine binary for git ops
    ↓
Python handles prompt assembly, agent launch
```

Status: **Current state**. Rust `lf-engine` handles git operations, Python handles everything else.

### Phase B: Rust Primary, Python Fallback

```
User runs: lf step debug
    ↓
Rust lf binary (primary)
    ↓
If missing feature → fall back to Python
```

Implementation:

```bash
# Wrapper script or alias during transition
lf() {
    if /path/to/rust/lf "$@" 2>/dev/null; then
        return 0
    else
        # Fallback to Python for unimplemented features
        python -m loopflow.lf.cli "$@"
    fi
}
```

Or build fallback into Rust:

```rust
// rust/lf/src/main.rs
fn main() {
    match run_command() {
        Ok(_) => {}
        Err(e) if e.is_not_implemented() => {
            // Fall back to Python
            let status = Command::new("python")
                .args(["-m", "loopflow.lf.cli"])
                .args(std::env::args().skip(1))
                .status();
            std::process::exit(status.map(|s| s.code().unwrap_or(1)).unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
```

### Phase C: Rust Only, PyPI Ships Binaries

Like ruff and uv, the Python package becomes a thin wrapper around Rust binaries.

```
pyproject.toml
├── [project.scripts]
│   └── lf = "loopflow._bin:main"
├── src/loopflow/
│   ├── _bin.py          # Thin wrapper
│   └── bin/
│       ├── lf-darwin-arm64
│       ├── lf-darwin-x86_64
│       ├── lf-linux-x86_64
│       └── lf-linux-aarch64
```

```python
# src/loopflow/_bin.py
import os
import sys
import platform
import subprocess

def _get_binary_path() -> str:
    """Find the bundled Rust binary for this platform."""
    pkg_dir = os.path.dirname(__file__)

    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "darwin":
        if machine == "arm64":
            binary = "lf-darwin-arm64"
        else:
            binary = "lf-darwin-x86_64"
    elif system == "linux":
        if machine == "aarch64":
            binary = "lf-linux-aarch64"
        else:
            binary = "lf-linux-x86_64"
    else:
        raise RuntimeError(f"Unsupported platform: {system}/{machine}")

    path = os.path.join(pkg_dir, "bin", binary)
    if not os.path.exists(path):
        raise RuntimeError(f"Binary not found: {path}")

    return path

def main():
    binary = _get_binary_path()
    os.execv(binary, [binary] + sys.argv[1:])

def daemon():
    binary = _get_binary_path().replace("lf-", "lfd-")
    os.execv(binary, [binary] + sys.argv[1:])
```

Build process (maturin or manual):

```yaml
# .github/workflows/release.yml
jobs:
  build-binaries:
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Build
        run: cargo build --release --target ${{ matrix.target }} -p lf -p lfd
      - name: Upload
        uses: actions/upload-artifact@v4
        with:
          name: binaries-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/lf*

  publish-pypi:
    needs: build-binaries
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Download binaries
        uses: actions/download-artifact@v4
      - name: Package binaries into wheel
        run: |
          mkdir -p src/loopflow/bin
          cp binaries-*/lf* src/loopflow/bin/
          chmod +x src/loopflow/bin/*
      - name: Build wheel
        run: uv build
      - name: Publish
        run: uv publish
```

### Phase D: PyO3 Bindings (Optional)

For users who want to call loopflow from Python code (not just CLI), expose Rust functions via PyO3.

```rust
// rust/loopflow-engine/src/python.rs
use pyo3::prelude::*;

#[pyfunction]
fn gather_context(
    repo_path: &str,
    step: Option<&str>,
    directions: Vec<String>,
    areas: Vec<String>,
) -> PyResult<String> {
    let config = crate::config::load_config(repo_path.as_ref())
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let ctx = crate::prompt::gather_context(&config)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    Ok(crate::prompt::format_prompt(&ctx, step))
}

#[pyfunction]
fn count_tokens(text: &str) -> usize {
    crate::prompt::count_tokens(text)
}

#[pymodule]
fn loopflow_engine(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(gather_context, m)?)?;
    m.add_function(wrap_pyfunction!(count_tokens, m)?)?;
    Ok(())
}
```

Usage from Python:

```python
# After: import the Rust extension
from loopflow_engine import gather_context, count_tokens

prompt = gather_context("/path/to/repo", step="debug", directions=["coding"])
tokens = count_tokens(prompt)
```

This is **optional** - only needed if we want Python API access. The CLI wrapper (Phase C) is sufficient for most users.

---

## 8. Python Code Removal

As Rust takes over, systematically remove Python code.

### Removal Sequence

| Module | Remove When | Replace With |
|--------|-------------|--------------|
| `loopflow.lf.launcher` | Phase C | Rust `lf` binary |
| `loopflow.lf.context` | Phase C | Rust `lf` binary |
| `loopflow.lf.config` | Phase C | Rust `lf` binary |
| `loopflow.lf.git` | Phase C | Rust `lf` binary |
| `loopflow.lf.ops.*` | Phase C | Rust `lf ops` |
| `loopflow.lf.cli` | Phase C | `_bin.py` wrapper |
| `loopflow.lfd.*` | Phase C + lfd rollout | Rust `lfd` binary |

### What Stays in Python

Some things may remain Python-only (or get PyO3 bindings later):

- **Skills system** - Superpowers integration, Python-native
- **Roadmap CLI** - Internal tooling, not user-facing
- **Test utilities** - Python test infrastructure

### Migration Checklist

```
[ ] All lf CLI tests pass against Rust binary
[ ] PyPI package builds with bundled binaries
[ ] `uv tool install loopflow` installs Rust lf
[ ] Remove loopflow.lf.launcher module
[ ] Remove loopflow.lf.context module
[ ] Remove loopflow.lf.config module
[ ] Remove loopflow.lf.git module
[ ] Remove loopflow.lf.ops.* modules
[ ] Simplify loopflow.lf.cli to thin wrapper
[ ] Update CLAUDE.md to reflect Rust-primary
[ ] Remove Python-only tests that are now redundant
```

---

## 9. lfd Rollout (Later)

Same pattern for the daemon, but sequenced after `lf`:

1. **lfd parity testing** - Verify Rust lfd matches Python lfd behavior
2. **lfd behind Python** - Python lfd calls Rust lfd for operations
3. **lfd primary** - Rust lfd is the default, Python falls back
4. **lfd only** - Remove Python lfd, ship Rust binary

This is Phase 2 work (02-lfd-primary) but follows the same rollout pattern.

---

## Done When

**Testing:**
- [ ] All ⚠️ Partial test files have Rust equivalents
- [ ] Prompt parity tests pass for all fixtures
- [ ] Golden file tests established and passing
- [ ] E2E workflow tests pass
- [ ] `--lfdocs` and `--diff-mode` flags added to Rust
- [ ] `lf ops doctor` implemented
- [ ] CI runs all test types

**Rollout:**
- [ ] Phase B: Rust primary with Python fallback tested
- [ ] Phase C: PyPI package builds with Rust binaries
- [ ] Phase C: `uv tool install loopflow` installs working Rust `lf`
- [ ] Python CLI modules removed
- [ ] CLAUDE.md updated for Rust-primary workflow
