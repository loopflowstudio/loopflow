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
| Run autonomous waves | Author `wave/<name>/GOAL.md`, open it in Loopflow (macOS) |
| Steer and inspect from terminal | `lf home start <name>` → `lf chat --steer` / `lf status` |
| Run on another machine | Set the wave's `home:` and run `lf home start <name>` ([Go Remote](#go-remote)) |

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
| `prompt` | Author or audit a skill, direction, Wave goal, or inline prompt |
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

Flows automate steps within one bounded pass. `build` runs kickoff →
review-design → implement → compress → lint → demo/review → gate. Use `ship` or
`deploy` for the explicit delivery workflow. Repetition belongs to Wave,
Project, and Task runtimes.

### Custom skills

Author one from intent:

```bash
lf prompt: create a dependency-audit skill
```

Or add one directly in `.lf/skills/`:

```markdown
# .lf/skills/audit.md
Check this branch for security issues.
Focus on input validation and auth boundaries.
```

```bash
lf audit    # runs your custom skill
```

The [Prompt Authoring guide](https://loopflow.studio/docs/prompts) covers prompt
contracts, evidence loops, Wave goals, and directions.

### Shipping

```bash
lf pr publish   # push + create or update PR (no browser)
lf pr open      # publish, then open the PR for review
lf pr land      # arm auto-merge; GitHub merges after required checks pass
```

---

## Scale with Waves

Ready to automate? Waves remain available continuously and run a complete flow
when chat, child observations, crons, or a heartbeat wake them.

`lf` skills are manual building blocks. A Wave is a named agent that reads its
Linear Projects and tasks, starts durable Task Sessions, and supervises their
results.

Author `wave/shipper/GOAL.md` (the body is the goal prompt; optional
frontmatter sets machine config such as `crons:` and `pm:`), then open it in
**Loopflow** (macOS) — the home for running waves. Select the repository and
the Wave to get its persistent conversation beside the Linear-backed
Project → Task work map; the app starts the Wave's resident process when
needed. From the CLI, `lf home start shipper` does the same start.

The Wave creates or selects a Linear task, starts it with `lf task run
<issue-id>`, and stays steerable while the Task Session works in its immutable
worktree. CI failures and review feedback return to the same session; linked
events land in the Wave thread.

Detached processes use named tmux sessions for process lifetime and read-only
inspection:

```bash
tmux ls               # live agent sessions
tmux attach -r -t <name> # inspect one; never mutate the session directly
```

Use `lf queue`, then `lf work feedback <kind> <id>`, for work that
needs you. Stop a running Wave with `lf stop <name>`.

Use `lf prompt: draft wave/shipper/GOAL.md` to author the loop contract. Use
`lf design` to explore an uncertain operating context, or write it by hand.
Once `wave/` files exist, `lf wave <name>` runs them and Loopflow picks them up.

[Waves →](waves.md) · [Conducting →](conducting.md)

---

## Go Remote

Run agents while you sleep. A wave's **Home** — set in `GOAL.md` frontmatter —
is where its work executes:

```yaml
home: ssh://jack@mini.local
```

```bash
lf home probe shipper    # reachable? stopped? running?
lf home start shipper    # idempotently start the Wave on its Home
```

Project and Task launches inherit the Home. There is no machine-wide service
to install and nothing to register: the remote host needs `lf` and SSH.
Credentials are resolved on your machine and forwarded per-invocation with
`lf ssh` — they live only as long as the remote process, so the remote host
stays a stateless compute surface.

Auth connects your providers locally:

```bash
lf auth github    # connect GitHub
lf auth claude    # connect Claude
lf auth linear    # connect Linear with OAuth
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

Status bar shows wave state: `[lf: main]` or `[lf: 3 waves | engbot]`. Customize with `@loopflow_status_format` (variables: `#{status}`, `#{branch}`, `#{skill}`, `#{waves}`, `#{wave}`):

```bash
# .tmux.conf
set -g @loopflow_status_format '[lf: #{status}]'    # default
```

| Key | Action |
|-----|--------|
| `prefix+l r` | Run skill/wave |
| `prefix+l s` | Stop |
| `prefix+l o` | Open logs |
| `prefix+l p` | Open PR |
| `prefix+l n` | New worktree |
| `prefix+l d` | Land PR |
| `prefix+l w` | Pick wave/worktree |
| `prefix+l L` | Pick layout |
| `prefix+l ?` | Help |

Works without `lf` installed — status shows a placeholder and keybindings explain themselves.

Two built-in layouts: `lf-dev` (editor + agent + shell), `lf-swarm` (monitor + 3 worktree workers).

---

## Reference

[`lf` commands](lf.md) · [Authoring](authoring.md) · [Configuration](config.md) · [Waves](waves.md) · [The Agent API](agent-api.md)
