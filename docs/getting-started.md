---
layout: default
title: Get Started
---

# Get Started

## Install

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
```

Install the Python CLI/API (lfq + loopflow) separately:

```bash
uv tool install loopflow
```

Requires macOS and one of: [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [Gemini CLI](https://github.com/google-gemini/gemini-cli).

---

## Quick Fix

Copy an error, run one command.

```bash
lf debug -c
```

```
Tokens: 8,247

files          4,892 ████
  STYLE.md     1,854 █
  README.md      588 ▏
diff_files     2,721 ██
  src/calc.py  1,847 █
  tests/         874 ▏
clipboard        634 ▏
```

The `-c` flag pastes your clipboard. `lf` assembles context—repo docs, branch files, clipboard—and passes it to the coding agent.

### Inline prompts

```bash
lf : "fix the typo in README"
lf : "add type hints to utils.py"
```

### Context flags

| Flag | What it adds |
|------|--------------|
| `-c` | Clipboard content |
| `--area PATH` | Scope to specific area |
| `--diff` | Raw `git diff` output |
| `-a` | Auto mode (no interaction) |

---

## Feature Workflow

Design, implement, polish, ship.

```bash
lf ops wt create auth-feature       # create worktree
lf design: add OAuth login         # discuss approach
lf implement                       # build it
lf polish                          # run tests, fix issues
lf ops pr                           # open PR
```

### Built-in steps

| Step | What it does |
|------|--------------|
| `design` | Explore the problem, write spec to `scratch/<branch>.md` |
| `implement` | Read spec, build it |
| `polish` | Run tests, fix failures |
| `review` | Assess quality, fix issues |
| `lint` | Run ruff, fix issues |

### How steps chain

| Step | Reads | Writes |
|------|-------|--------|
| design | — | `scratch/<branch>.md` |
| implement | `scratch/<branch>.md` | code |
| polish | code, tests | code |
| review | code | `scratch/review.md` |

### Shipping

```bash
lf ops pr      # create or update PR
lf ops land    # submit to merge queue
```

---

## Waves

Run waves overnight, review PRs when you wake.

```bash
lfq create shipper .
python - <<'PY'
import loopflow.api as loopflow

loopflow.update_wave("shipper", flow="ship", area=["src/"])
loopflow.run_wave("shipper")
PY
```

A wave is **area × direction × flow × stimulus**. [Learn more →](waves.md)

---

## Try It

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -c                       # fix it
```

---

## Reference

[`lf` commands](lf.md) · [`lf ops` commands](lfops.md) · [`lfd` commands](lfd.md) · [Configuration](config.md)
