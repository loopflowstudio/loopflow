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
| Steer and inspect from terminal | `lf start <name>` → `lf chat --steer` / `lf status` |
| Run on another machine | `lf ssh <home-id> start <name>` ([Go Remote](#go-remote)) |

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

The `-c` flag pastes your clipboard. `lf` assembles context—operating guidance,
scratch notes, and clipboard—and passes it to the coding agent. Before the
provider starts, Loopflow publishes one immutable Run manifest under this
Home's `$LF_HOME/runs/`; conversation, usage, and terminal evidence accumulate
there without becoming a lock on the repository or planning system. Add repo
docs explicitly with `--docs` and changed file bodies with `--diff-files`.

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
lf task start <linear-project-id> "add OAuth login"
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
| `compress` | Simplify the implementation without changing behavior |
| `gate` | Verify the branch for shipping: tests, static checks, docs, PR description |
| `qa` | Thorough quality assessment of the current branch |

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
lf design                                # review one exact design artifact
lf launch-plan                           # keep the core here; launch independent Tasks
lf build                                 # one code → reviewable Task slice
lf ship                                  # final Task gate → learnings → land
```

Flows automate skills within one bounded pass. Their YAML owns ordering and
human gates; `ship` and `deploy` own the ordinary delivery steps. Repetition
belongs to Wave, Project, and Task runtimes.

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

The [Authoring guide](authoring.md) covers prompt contracts, evidence loops,
Wave goals, and directions.

### Shipping

```bash
lf pr publish   # push + create or update PR (no browser)
lf pr open      # publish, then open the PR for review
lf pr submit    # prepare the exact head; you click merge
lf pr arm       # arm exact-head auto-merge and return
lf pr land      # watch, repair CI, and return after GitHub merges
```

Use the same delivery verbs for Task and non-Task branches. They act on the
branch and Task PR record when present; they do not require end-to-end
controller state.

---

## Scale with Waves

Ready to automate? Waves remain available continuously and choose another
bounded pass when chat, child observations, crons, or a heartbeat wake them.

`lf` skills are manual building blocks. A Wave is a named agent that reads its
Linear Projects and tasks, starts durable Tasks, and supervises their
results.

Author `wave/shipper/GOAL.md` (the body is the goal prompt; optional
frontmatter sets machine config such as `owner:`, `home:`, `crons:`, and `pm:`), then open it in
**Loopflow** (macOS) — the home for running waves. Select the repository and
the Wave to get its persistent conversation beside the Linear-backed
Project → Task work map; the app starts the Wave's resident process when
needed. From the CLI, `lf start shipper` does the same start.

The Wave creates or selects a Linear task, starts it with `lf task run
<issue-id>`, and stays steerable while the Task runs in its stable
worktree. `lf pr land` keeps the PR under one durable watcher through CI repair
and merge; review feedback returns to the same Task and linked events land in
the Wave thread.

Detached processes use named tmux sessions for process lifetime and read-only
inspection:

```bash
tmux ls               # live agent sessions
tmux attach -r -t <name> # inspect one; never mutate the session directly
```

Use `lf session list`, then `lf session open <session-id>`, for every unresolved
interactive Run, Ask, or Task FlowStep. Finish it with the kind-specific action
shown in the [Sessions lifecycle](../README.md#sessions).

Use `lf prompt: draft wave/shipper/GOAL.md` to author the loop contract. Use
`lf design` to explore an uncertain operating context, or write it by hand.
Once `wave/` files exist, `lf wave <name>` runs them and Loopflow picks them up.

[Waves →](waves.md) · [Conducting →](conducting.md)

---

## Go Remote

Run agents while you sleep. A Home is a stable machine identity with local
planning, process, journal, and Run evidence; its SSH route can change.
Bootstrap the remote identity once:

```bash
lf ssh jack@mini.local home id --json
lf home observe <home-id> ssh://jack@mini.local
lf ls --json
lf work place wave <wave-id> <home-id>    # record origin-side planning state
lf ssh <home-id> start shipper
```

`lf start shipper` always starts shipper on the machine executing that command.
`lf ssh <home-id> start shipper` executes the same operation on the named Home,
which proves its identity before changing lifecycle state. One Home keeper
serves every Wave running there.

Reads follow the same rule: `lf runs`, `lf usage`, `lf ls`, and `lf status`
read the executing Home. Prefix the command with `lf ssh <home-id>` to read
another Home. Loopflow does not silently aggregate or replicate Run records.

Foreground `lf ssh` commands can choose from subscription accounts installed on
the origin and target. A resident that outlives SSH sheds forwarded credentials
and uses authority installed on its own machine. See
[Subscription Management](subscriptions.md#use-subscriptions-over-ssh).

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
