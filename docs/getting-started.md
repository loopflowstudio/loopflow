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
| Run autonomous waves | `lf init` → `lf wave <name>` |
| Use Wave Chat (macOS) | Download Loopflow and open a repository |
| Run on another machine | SSH into the host and run `lf wave <name>` there |

---

## Try It

Copy an error, run one command.

```bash
lf debug -c
```

```
Tokens: 8,247

system         4,892 ████
skill           1,854 █
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

Start from a Linear task; Loopflow creates and retains its worktree.

```bash
lf task start "add OAuth login" --project <linear-project-id>
lf task status <issue-id>
lf task steer <issue-id> "support passkeys too"
lf task wait <issue-id> --until terminal
```

### Skills chain

| Skill | What it does |
|------|--------------|
| `design` | Explore the problem, write spec to `scratch/<branch>.md` |
| `implement` | Read spec, build it |
| `gate` | Ship-ready check: tests, quality, PR description |
| `qa` | Thorough quality assessment of the current branch |
| `lint` | Run ruff, fix issues |

### How steps chain

| Skill | Reads | Writes |
|------|-------|--------|
| design | — | `scratch/<branch>.md` |
| implement | `scratch/<branch>.md` | code |
| gate | code, tests | code, PR description |
| qa | code | findings and fixes on branch |

### Named flows

Chain skills manually, or use a named flow (a flow is a sequence of steps; each step names a skill, an op, or a subflow):

```bash
lf design && lf implement && lf gate    # manual chain
lf build                                # one design → code → demo/review → gate pass
```

Flows automate handoffs within one bounded pass. `build` runs kickoff →
review-design → implement → compress → lint → demo/review → gate. Use `ship` or
`deploy` for the explicit delivery workflow. Repetition belongs to Wave,
Project, and Task runtimes.

### Custom skills

Add your own in `.lf/skills/`:

```markdown
# .lf/skills/audit.md
Check this branch for security issues.
Focus on input validation and auth boundaries.
```

```bash
lf audit    # runs your custom skill
```

### Shipping

```bash
lf pr publish   # push + create or update PR (no browser)
lf pr open      # publish, then present an ordinary PR in GitHub
lf pr land      # land an ordinary non-Task PR
```

---

## Scale with Waves

Ready to automate? Waves remain available continuously and run a complete flow
when chat, child observations, crons, or a heartbeat wake them.

`lf` skills are manual building blocks. A Wave is a named agent that reads its
Linear Projects and tasks, starts durable Task Sessions, and supervises their
results.

Author `wave/shipper/GOAL.md` (the body is the goal prompt; optional frontmatter sets machine config such as `crons:` and `pm:`), then run the agent:

```bash
lf wave shipper
```

The Wave creates or selects a Linear task, starts it with `lf task run
<issue-id>`, and stays steerable while the Task Session works in its immutable
worktree. CI failures and review feedback return to the same session; linked
events land in the Wave thread.

**Loopflow** (macOS) is the native Wave experience. Select a repository and a
Wave to open its persistent conversation beside the Linear-backed Project →
Task work map. The app queries local state through its bundled `lf` and starts
the selected Wave's `lf wave` process when needed.

Detached processes use named tmux sessions:

```bash
tmux ls               # live agent sessions
tmux attach -r -t <name> # inspect one; never mutate the session directly
```

Use `lf project attach <project>` or `lf task attach <issue>` for a writable,
audited control prompt. Stop a foreground Wave with Ctrl-C or run
`lf stop <name>`.

You can draft wave content with `lf design` locally, or write it by hand. Once `wave/` files exist, `lf wave <name>` runs them and Loopflow picks them up.

[Wave Authoring Guide →](wave-authoring.md) · [Waves Reference →](waves.md)

---

## Go Remote

Run agents while you sleep. SSH into a server, install Loopflow, and run
`lf wave <name>` there. The Wave process owns its listener and resident loop;
there is no machine-wide service to install.

Remote Loopflow/Cadenza is future work; for now, use SSH as the remote control
surface.

Auth connects your providers:

```bash
lf auth github    # connect GitHub
lf auth claude    # connect Claude
lf auth linear     # connect Linear with OAuth
lf auth status    # check connections
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
| `prefix+l r` | Run skill/wave |
| `prefix+l s` | Stop |
| `prefix+l o` | Open logs |
| `prefix+l w` | Pick wave/worktree |
| `prefix+l L` | Pick layout |

Two built-in layouts: `lf-dev` (editor + agent + shell), `lf-swarm` (monitor + 3 worktree workers).

---

## Reference

[`lf` commands](lf.md) · [`lf` operations](ops.md) · [Configuration](config.md) · [Wave Authoring](wave-authoring.md) · [Waves](waves.md)
