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

Requires macOS or Linux, and one of: [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [OpenCode](https://github.com/anomalyco/opencode).

### Setup Paths

| I want to... | Start here |
|---|---|
| Try loopflow from terminal | `lf init` |
| Run autonomous waves | `lf init` → `lfd install` |
| Use the visual app (macOS) | Download Loopflow (handles the rest) |
| Connect from iPhone | Loopflow iOS → discovers your lfd |
| Set up remote dev server | SSH into the host and run native `lf`/`lfd` there |

---

## Try It

Copy an error, run one command.

```bash
lf debug -c
```

```
Tokens: 8,247

system         4,892 ████
step           1,854 █
scratch          867 ▏
clipboard        634 ▏
```

The `-c` flag pastes your clipboard. `lf` assembles context—operating guidance, scratch notes, and clipboard—and passes it to the coding agent. Add repo docs explicitly with `--docs` and changed file bodies with `--diff-files`.

`LOOPFLOW.md` ships as default operating guidance for every run; opt out with `--no-loopflow`.

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
| `--docs PATH,PATH` | Add specific files, globs, or directories to context |
| `--diff-files` | Full content of files changed on the branch |
| `--diff` | Raw `git diff` output |
| `-i` | Interactive mode |
| `-b` | Batch/headless mode |

---

## Build Features

Design, implement, gate, ship.

```bash
lf op wt create auth-feature       # create worktree
lf design: add OAuth login         # discuss approach
lf implement                       # build it
lf gate                            # ship-ready check
lf op pr                           # open PR
```

### Steps chain

| Step | What it does |
|------|--------------|
| `design` | Explore the problem, write spec to `scratch/<branch>.md` |
| `implement` | Read spec, build it |
| `gate` | Ship-ready check: tests, quality, PR description |
| `qa` | Thorough quality assessment of the current branch |
| `lint` | Run ruff, fix issues |

### How steps chain

| Step | Reads | Writes |
|------|-------|--------|
| design | — | `scratch/<branch>.md` |
| implement | `scratch/<branch>.md` | code |
| gate | code, tests | code, PR description |
| qa | code | findings and fixes on branch |

### Named flows

Chain steps manually, or use a named flow:

```bash
lf design && lf implement && lf gate    # manual chain
lf build                                # full design → code → demo/review → deploy loop
```

Flows automate the handoffs. `build` runs kickoff → review-design → loop(code → xor(demo, code-review), exit: gate) → deploy.

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
lf op pr      # create or update PR
lf op land    # submit to merge queue
```

---

## Scale with Waves

Ready to automate? Waves run your workflows continuously.

`lf` steps are manual building blocks. A wave is a named agent that runs them for you — reading a roadmap, dispatching workers to build each item, watching their PRs, and looping.

Author `wave/shipper/GOAL.md` (the body is the goal prompt; optional frontmatter sets machine config such as `workers:`, `crons:`, and `pm:`), then run the agent:

```bash
lf wave shipper
```

The wave agent coordinates; it dispatches a **worker** per task (`lf build "…" --wave shipper --dispatch`) and folds each shipped PR into memory. Today CI failures and pushes to main land in the wave's thread as attributed `lf chat` notifications, so the demo path stays live while webhook coordination moves toward durable facts plus explicit commands.

**Loopflow** (macOS) is the native wave experience — monitor progress, browse flows, review PRs. Requires `lfd`.

Sessions are plain tmux:

```bash
tmux ls               # live agent sessions
tmux attach -t <name> # attach to one; agent output lives here
```

Stop a wave with Ctrl-C in its `lf wave` session.

### Browse flows

1. Open **Flows** in Loopflow.
2. Expand `build` → `build`.
3. Click `gate` to see every parent flow that reaches it.

`lfd` serves the same resolved catalog at `/v0/catalog?repo=/path/to/repo`, including builtin definitions and any `.lf/flows/*.yaml` or `.lf/steps/*.md` overrides in the repo.

You can draft wave content with `lf design` locally, or write it by hand. Once `wave/` files exist, `lf wave <name>` runs them and Loopflow picks them up.

[Wave Authoring Guide →](wave-authoring.md) · [Waves Reference →](waves.md)

---

## Go Remote

Run agents while you sleep. SSH into a server, install Loopflow, and run native
`lf`/`lfd` there.

```bash
mkdir -p ~/.lf
cat > ~/.lf/lfd.yaml <<'YAML'
mode: native
YAML
lfd install
```

Remote Loopflow/Cadenza is future work; for now, use SSH as the remote control
surface.

Auth connects your providers:

```bash
lf op auth github    # connect GitHub
lf op auth claude    # connect Claude
lf op auth linear     # connect Linear with OAuth
lf op auth status    # check connections
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

[`lf` commands](lf.md) · [`lf op` commands](lfop.md) · [`lfd` commands](lfd.md) · [Configuration](config.md) · [Wave Authoring](wave-authoring.md) · [Waves](waves.md)
