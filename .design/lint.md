# Smart Lint: Fast Path with Agent Fallback

Run lint checks directly first; only invoke the agent if they fail.

## Background

Previously `run_lint()` always invoked `lf lint -a`, spinning up an agent even when lint already passes. Now we try the fast path first, escalating to agent only on failure.

## Config

Optional in `.lf/config.yaml`:

```yaml
lint_check: "ruff check src/ tests/ && ruff format --check src/ tests/"
```

If set, run this command to check if lint passes. If it exits 0, skip the agent.

If not set, fall back to auto-detection (ruff + src/tests dirs).

## Key functions

```python
def _check_lint(repo_root: Path, config: Config | None) -> bool | None:
    """Run lint check. Returns True if passes, False if fails, None if can't check."""

def run_lint(repo_root: Path) -> bool:
    """Check lint first; invoke agent only if checks fail."""
```

## Flow

```
┌──────────────────┐
│ config.lint_check│  ← user-specified command (if set)
│    OR            │
│ auto-detect ruff │  ← fallback: ruff check + format --check
└────────┬─────────┘
         │
     passes? ────yes───→ return True (no agent)
         │
         no (or can't check)
         ↓
┌──────────────────┐
│ lf lint -a       │  ← agent uses lint.md
└────────┬─────────┘
         │
     passes? ────yes───→ return True
         │
         no
         ↓
     return False
```

## Files changed

- `src/loopflow/lfops/_helpers.py` — `_check_lint()` and updated `run_lint()`
- `src/loopflow/lf/config.py` — `lint_check: Optional[str]` field
- `src/loopflow/lfops/commit.py` — `--lint/--no-lint` flag

## Behavior

- If `lint_check` configured: use that command for fast path
- If not configured: auto-detect ruff + src/tests dirs
- If can't fast-check: go straight to agent
- Agent uses customizable `lint.md` step file

## Usage

```bash
# Fast path - lint passes, no agent:
$ lfops commit
Lint passed
Committing...

# Fast path - lint fails, agent fixes:
$ lfops commit
Lint issues found, running fixer...
[agent fixes issues]
Committing...

# Skip lint check:
$ lfops commit --no-lint
Committing...
```
