# Rust Testing and Rollout

## Status (2026-02-04)

What is implemented today:
- Prompt parity tooling (`lf-prompt` + golden tests) compares Rust prompt assembly against Python.
- Ops parity tracing in Python and Rust, with a parity test that compares traces.
- E2E shell tests (smoke, full cycle, rebase conflict) exist; smoke runs in CI.
- Release workflow bundles platform `lf` binaries into the Python wheel; `src/loopflow/_bin.py` dispatches to the Rust binary.
- Docs updated for Rust-first behavior and testing entry points.

Key choices:
- Trace-based ops parity emits JSON instead of running side effects for deterministic tests.
- Golden prompts keep Python as the source of truth; Rust matches fixtures.
- Rust-first CLI uses `LF_RUST=0` as the only escape hatch; default prefers Rust when a bundled binary exists.

Risks and bottlenecks:
- Ops trace parity currently covers `commit` only; other ops commands rely on behavioral tests.
- E2E scripts compile `lf`/`loopflow-engine` during execution; CI runtime may be heavier than expected.
- Release workflow ships only `lf`; `lfd` bundling remains undecided.

## Problem

Rust `lf` claims feature parity with Python. We need confidence before shipping it as primary.

Current state:
- Parity tests exist and pass (prompt parity, ops parity)
- 182 Rust unit tests run in CI
- But: test coverage gaps remain, rollout path unclear

Users benefit from:
- Single-binary distribution (like ruff, uv)
- Faster startup, lower memory
- One implementation to maintain

Why now: Phase 1 complete. Phase 2 (service, distribution) blocked until we ship confidently.

## Approach

**Skip the test coverage treadmill.** The existing parity tests prove behavior matches. More unit tests test implementation, not behavior. Focus on:

1. **Behavioral parity gates** - CI fails if Rust diverges from Python
2. **E2E smoke tests** - Real workflows work end-to-end
3. **Ship incrementally** - `LF_RUST=1` opt-in, then flip default

Don't chase unit test parity for its own sake. The Python tests test Python implementation details. Rust tests should test Rust implementation details. What matters is that the *outputs* match.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Port all Python tests to Rust | Comprehensive but slow; tests implementation not behavior | Tests become maintenance burden; doesn't prove parity |
| Golden file expansion | More fixtures catch more edge cases | Diminishing returns; existing 3 cases + ops parity cover the critical paths |
| Ship without rollout phase | Faster to market | Breaks users who hit edge cases we missed |

## Key decisions

### 1. Parity tests are the gate, not unit tests

> "Design must follow the wave's principles" - Wave alignment requires testing via parity, not coverage metrics.

Current parity tests:
- `test_prompt_parity.py` - Given same inputs, Rust produces same prompt as Python
- `test_ops_parity.py` - Commit workflow traces match

This is sufficient. If prompts match and ops traces match, the Rust implementation is correct. Add more parity fixtures only when bugs are found.

### 2. E2E smoke tests prove it works

Add one test: `tests/e2e/test_smoke.sh`

```bash
#!/bin/bash
set -e
REPO=$(mktemp -d)
cd "$REPO"
git init && git commit --allow-empty -m "init"
mkdir -p .lf/steps
echo "# Test" > .lf/steps/debug.md

# Core: prompt generation works
lf run debug --dry-run | grep -q "Test"

# Ops: branch workflow works
lf ops wt create smoke-test
cd ../smoke-test
echo "change" > file.txt
git add file.txt
lf ops commit -m "smoke test" --skip-agent

echo "PASS"
```

This catches catastrophic failures. Not edge cases - those are covered by parity tests.

### 3. Opt-in rollout via environment variable

Phase 1: `LF_RUST=1` uses Rust binary
Phase 2: Rust is default, `LF_RUST=0` falls back to Python
Phase 3: Python fallback removed

Implementation: One-line change in `src/loopflow/_bin.py`:

```python
def main():
    if os.environ.get("LF_RUST", "1") != "0":
        binary = _get_rust_binary()
        if binary and binary.exists():
            os.execv(str(binary), [str(binary)] + sys.argv[1:])
    # Fall through to Python
    from loopflow.lf.cli import main as lf_main
    lf_main()
```

### 4. PyPI ships bundled binaries on tag

GitHub Actions builds binaries for:
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`

On release tag, binaries are bundled into wheel at `src/loopflow/bin/`.

Not building Windows binaries. Loopflow is Unix-focused. Windows users can use WSL.

### 5. Cut Python code immediately after Phase 3

No gradual deprecation. When `LF_RUST=0` fallback is removed:
- Delete `loopflow.lf.launcher`
- Delete `loopflow.lf.context`
- Delete `loopflow.lf.config`
- Delete `loopflow.lf.git`
- Delete `loopflow.lf.ops.*`
- Keep: `loopflow.lf.skills` (Python-native)
- Keep: `loopflow.lf.roadmap` (internal tooling)

## Scope

**In scope:**
- E2E smoke test script
- `LF_RUST` environment variable support
- Release workflow for binary bundling
- CI job for E2E smoke test
- Flipping `LF_RUST` default to 1

**Out of scope:**
- Porting Python unit tests to Rust
- Additional parity test fixtures (unless bugs found)
- lfd rollout (separate work item)
- PyO3 Python API bindings (not needed for CLI users)
- Windows binary builds

## Done when

```bash
# 1. E2E smoke test passes in CI
./tests/e2e/test_smoke.sh  # exits 0

# 2. LF_RUST=1 works for all supported platforms
LF_RUST=1 lf run debug --dry-run  # produces prompt

# 3. Release workflow produces working wheel
uv pip install dist/loopflow-*.whl
LF_RUST=1 lf --version  # shows version

# 4. Default flipped (Phase 2 gate)
lf run debug --dry-run  # uses Rust by default
LF_RUST=0 lf run debug --dry-run  # uses Python fallback
```

Phase 3 (Python removal) is a separate PR after 2 weeks of Phase 2 with no reported issues.
