# Rust LF Parity

Tracking parity between Python and Rust implementations.

## Status

| Area | Status | Notes |
|------|--------|-------|
| Prompt assembly (`lf run`) | ✅ Done | `test_prompt_parity.py` compares Python vs Rust byte-for-byte |
| Ops workflows (`lf ops`) | 🔜 Next | E2E smoke tests exist but don't verify parity with Python |

## What's done

**Prompt parity** — `tests/parity/test_prompt_parity.py`
- Fixtures under `tests/parity/fixtures/`
- Runs Python `gather_prompt_components` and Rust `lf-prompt` binary
- Normalizes paths/timestamps and compares output
- Covers: steps, directions, diff modes, clipboard

**E2E smoke tests** — `tests/e2e/`
- `test_full_cycle.sh`: commit → land workflow runs without crashing
- `test_rebase_conflict.sh`: conflict detection works
- These verify Rust ops runs, but don't compare against Python

## What's next: Ops parity

The Rust `lf ops` commands run, but we haven't verified they produce the same behavior as Python. Need mock-based logic comparison.

### Approach

Mock side effects and capture operation traces. Compare traces between Python and Rust.

```
Python lf ops commit:
  1. git.stage(["file.txt"])
  2. git.diff() -> "add file"
  3. agent.run(prompt="generate commit message", ...)
  4. git.commit(message="lf test: add file")

Rust lf ops commit:
  [should produce identical trace]
```

### What to mock

| Side effect | What to capture |
|-------------|-----------------|
| Git operations | Command + args (stage, commit, push, rebase, etc.) |
| Agent invocations | Prompt text, model, flags |
| GitHub CLI | gh command + args (pr create, pr merge, etc.) |
| Lint runner | Command invoked, pass/fail decision |

### Test cases

| Command | Key behaviors to verify |
|---------|------------------------|
| `commit` | Staging logic, commit message format, flow lineage prefix |
| `pr` | PR title/body generation, draft/ready flags, auto-merge setup |
| `land` | Merge strategy, branch cleanup, PR close |
| `rebase` | Conflict detection, rebase assistant prompt |
| `next` | Branch naming, worktree creation |
| `abandon` | Branch deletion, worktree cleanup |

### Implementation

1. Add `OpTrace` type to both Python and Rust that records operations
2. Add `--trace` flag (or env var) that outputs JSON trace instead of executing
3. Parity test runs both, compares traces
4. Fixtures: small repos with specific states (dirty, conflicts, stacked branches)

### Done when

- [ ] Python ops commands can emit traces (mock mode)
- [ ] Rust ops commands can emit traces (mock mode)
- [ ] `test_ops_parity.py` compares traces for each command
- [ ] All ops commands pass parity on fixture set

## Rollout (after parity)

- Phase B: Rust CLI primary with explicit fallback to Python for not-implemented paths
- Phase C: PyPI ships Rust binaries; Python CLI becomes a thin wrapper
- Phase D (optional): PyO3 bindings for Python API users

## Risks

- Trace format must capture enough detail to catch logic differences
- Some behaviors may be intentionally different (improvements) — need way to mark these
- Agent prompts may differ in whitespace/ordering — need normalization
