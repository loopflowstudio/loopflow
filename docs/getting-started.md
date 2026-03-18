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

Default install location is `~/.local/bin`. Override with `LF_INSTALL_DIR=/path`.

Install the Python CLI/API (lfq + loopflow) separately:

```bash
uv tool install loopflow
```

Requires macOS or Linux, and one of: [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [OpenCode](https://github.com/anomalyco/opencode).

### Setup Paths

| I want to... | Start here |
|---|---|
| Try loopflow from terminal | `lf init` |
| Run autonomous waves | `lf init` → `lfd install` |
| Use the visual app (macOS) | Download Concerto (handles the rest) |
| Connect from iPhone | Concerto iOS → discovers your lfd |
| Set up remote dev server | `mode: container` in `~/.lf/lfd.yaml`, then `lfd install` |

---

## Try It

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

Or try the demo repo:

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -c                       # fix it
```

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

## Build Features

Design, implement, gate, ship.

```bash
lf ops wt create auth-feature       # create worktree
lf design: add OAuth login         # discuss approach
lf implement                       # build it
lf gate                            # ship-ready check
lf ops pr                           # open PR
```

### Steps chain

| Step | What it does |
|------|--------------|
| `design` | Explore the problem, write spec to `scratch/<branch>.md` |
| `implement` | Read spec, build it |
| `gate` | Ship-ready check: tests, quality, PR description |
| `review` | Assess quality, fix issues |
| `lint` | Run ruff, fix issues |

### How steps chain

| Step | Reads | Writes |
|------|-------|--------|
| design | — | `scratch/<branch>.md` |
| implement | `scratch/<branch>.md` | code |
| gate | code, tests | code, PR description |
| review | code | `scratch/review.md` |

### Named flows

Chain steps manually, or use a named flow:

```bash
lf design && lf implement && lf gate    # manual chain
lf build                                # same thing, named flow
```

Flows automate the handoffs. `build` runs implement → compress → lint → gate → update-wave.

### Custom steps

Add your own in `.lf/steps/`:

```markdown
# .lf/steps/audit.md
Check this branch for security issues.
Focus on input validation and auth boundaries.
```

```bash
lf audit    # runs your custom step
```

### Shipping

```bash
lf ops pr      # create or update PR
lf ops land    # submit to merge queue
```

---

## Scale with Waves

Ready to automate? Waves run your workflows continuously.

`lf` steps are manual building blocks. Waves automate them — picking tasks from a backlog, running flows, creating PRs, and looping until the work is done.

```bash
lfq create shipper .
```

```python
import loopflow.api as loopflow

loopflow.update_wave("shipper", flow="build", area=["src/"], direction=["clarity"])
loopflow.run_wave("shipper")
```

A wave is **area × direction × flow**. The wave picks a task, runs the flow, opens a PR, and loops. Triggers fire flows in response to signals (repo changes, other waves completing, CI failures).

**Concerto** (macOS) is the native wave experience — create waves, monitor progress, review PRs. Requires `lfd`.

**lfq** is the CLI equivalent — same `lfd` backend, terminal interface.

```bash
lfq list             # list waves
lfq logs shipper     # tail agent output
lfq stop shipper     # stop a wave
```

You can draft wave content with `lf design` locally, or write it by hand. Once `wave/` files exist, Concerto and lfq pick them up and run them.

[Wave Authoring Guide →](wave-authoring.md) · [Waves Reference →](waves.md)

---

## Go Remote

Run agents while you sleep. Install `lfd` on a server and your waves run 24/7.

```bash
mkdir -p ~/.lf
cat > ~/.lf/lfd.yaml <<'YAML'
mode: container
YAML
lfd install
```

Concerto mobile connects to remote `lfd` — monitor and manage waves from your phone. `lfq` works the same way over the network. See `deploy/README.md` for the Docker + TLS recipe.

Auth connects your providers:

```bash
lf ops auth asana    # connect Asana for local `lf` commands like `lf ops export`
lf ops auth configure linear  # store Linear API key for local `lf` PM commands
lf ops auth status   # check local lf credentials

lfq auth github      # connect GitHub
lfq auth claude      # connect Claude
lfq auth asana       # connect Asana with OAuth
lfq auth linear      # store Linear API key
lfq auth status      # check connections
```

---

## tmux Plugin

Status bar, keybindings, layouts — all from the terminal.

```bash
# Add to .tmux.conf
set -g @plugin 'loopflowstudio/loopflow.tmux'
run '~/.tmux/plugins/tpm/tpm'
```

Status bar shows wave state: `[lf: main]` or `[lf: 3 waves | engbot]`.

| Key | Action |
|-----|--------|
| `prefix+l r` | Run step/wave |
| `prefix+l s` | Stop |
| `prefix+l o` | Open logs |
| `prefix+l w` | Pick wave/worktree |
| `prefix+l L` | Pick layout |

Two built-in layouts: `lf-dev` (editor + agent + shell), `lf-swarm` (monitor + 3 worktree workers).

---

## Reference

[`lf` commands](lf.md) · [`lf ops` commands](lfops.md) · [`lfd` commands](lfd.md) · [Configuration](config.md) · [Wave Authoring](wave-authoring.md) · [Waves](waves.md)
