# Get Started

## Install

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
```

Default install location is `~/.local/bin`. Override with `LF_INSTALL_DIR=/path`.

### Before you install

| You need | For |
|---|---|
| macOS or Linux | Everything |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [OpenCode](https://github.com/anomalyco/opencode), signed in | Every run. Loopflow uses the one you have; these have their own cost |
| `git`, and a project in a git repository | Every run |
| [GitHub CLI](https://cli.github.com) (`gh`), signed in | Pull requests |
| A [Linear](https://linear.app) workspace | Tasks and Waves |

A single prompt (`lf debug -c`, `lf : "..."`) needs only the first three.

### Setup Paths

| I want to... | Start here |
|---|---|
| Try loopflow from terminal | `lf init` |
| Run autonomous waves | Author `wave/<name>/GOAL.md`, open it in Loopflow (macOS) |
| Steer and inspect from terminal | `lf --wave <name> wave/operate` → `lf wave status` |
| Run on another machine | `lf ssh <home-id> --wave <name> wave/operate` ([Go Remote](#go-remote)) |

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
provider starts, Loopflow reserves the AgentSession and captures its input in
this Home. The store must be writable even for unbound work. Native history
retains provider outcomes and usage; Exec records the command result. Add repo
docs explicitly with `--docs` and changed file bodies with `--diff files`.

`LOOPFLOW.md` ships as default operating guidance for every run; opt out with `--no-loopflow`.

Or try the demo repo:

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -c                            # fix it
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
| `--diff files` | Full content of files changed on the branch |
| `--diff patch` | Raw `git diff` output |
| `--mode interactive` | Interactive mode |
| `--mode batch` | Batch/headless mode |

---

## Build Features

Start from a Linear task; Loopflow creates and retains its worktree.

```bash
lf task create --run --wave <wave> --title "add OAuth login"
lf task status <issue-id>
lf comment <issue-id> "support passkeys too"
lf wait <issue-id> --until terminal
```

### Skills chain

| Skill | What it does |
|------|--------------|
| `prompt` | Author or audit a skill, Wave goal, or inline prompt |
| `design` | Explore the problem, write spec to `scratch/<branch>.md` |
| `implement` | Read spec, build it |
| `compress` | Simplify the implementation without changing behavior |
| `gate` | Verify the branch for shipping: tests, static checks and docs |
| `qa` | Thorough quality assessment of the current branch |

### How steps chain

| Skill | Reads | Writes |
|------|-------|--------|
| design | — | `scratch/<branch>.md` |
| implement | `scratch/<branch>.md` | code |
| gate | code, tests | code, docs and proof |
| qa | code | findings and fixes on branch |

### Named flows

Chain skills manually, or use a named flow (a flow is a sequence of steps; each step names a skill, an op, or a subflow):

```bash
lf incident                            # unbreak → 5whys → launch-plan
lf code                                # implement → compress; local changes
lf feature                             # kickoff → design review → pursue → queue → land
lf ship                                # gate → land and complete the Task
```

Use bare names for both skills and Flows: `lf debug`, `lf code`, `lf incident`.
`lf flow incident` explicitly selects the Flow when a name also names a skill
or CLI command; the prefix is otherwise optional.

Flow YAML owns step order, interactive reviews (`human: true`), and explicit
backward edges. A Flow with backward edges is a loopflow; it runs with or without
a Task. `ship` and `deploy` supply explicitly selected delivery steps.

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
lf audit                               # runs your custom skill
```

The [Authoring guide](authoring.md) covers prompt contracts, evidence loops,
and Wave goals.

### Shipping

```bash
lf pr open                             # push + create or update a draft, then open its page
lf pr publish                          # push + create or update PR and mark ready (no browser)
lf submit                              # prepare the exact head; you click merge
lf land                                 # watch CI, repair failures, and finish merged
```

Use the same delivery verbs for Task and non-Task branches. They act on the
branch and Task PR record when present; they do not require a live Task worker.

---

## Scale with Waves

```bash
lf --wave shipper wave/operate "Review the release blockers"
lf wave status shipper
lf ps --json
```

Author `wave/shipper/GOAL.md` with an objective and operating guidance. Optional
`crons:` schedules recurring commands; `pm:` connects shared planning. Each
`wave/operate` invocation reviews the plan and takes a bounded next action.
Tasks own implementation in stable worktrees; `lf pr land` watches CI and repair
through merge.

Open the repository in Loopflow on macOS to read Waves, Tasks and conversations.
`lf session list` finds conversations, Asks and Flow reviews; open one with
`lf session connect <session-id>`. Completing it returns its saved feedback.

[Waves →](waves.md) · [Conducting →](conducting.md)

## Go Remote

Run agents while you sleep. A Home is a stable machine identity with local
planning, process, journal, and conversation history; its SSH route can change.
Bootstrap the remote identity once:

```bash
lf ssh jack@mini.local home id --json
lf observe <home-id> ssh://jack@mini.local
lf wave list --json
lf wave place <wave-id> <home-id>    # record origin-side planning state
lf ssh <home-id> --wave shipper wave/operate
```

The target Home proves its identity before running the command and keeps the
resulting execution locally.

Reads follow the same rule: `lf monitor list`, `lf usage`, `lf wave list`, and `lf wave status`
read the executing Home. Prefix the command with `lf ssh <home-id>` to read
another Home. Loopflow does not silently aggregate or replicate execution records.

Foreground `lf ssh` commands can choose from subscription accounts installed on
the origin and target. A detached process sheds forwarded credentials
and uses authority installed on its own machine. See
[Subscription Management](subscriptions.md#use-subscriptions-over-ssh).

Auth connects your providers locally:

```bash
lf account connect github    # connect GitHub
lf account connect claude    # connect Claude
lf account connect linear    # connect Linear with OAuth
lf account    # refresh managed identity and usage
```

---

## Reference

[`lf` commands](lf.md) · [Authoring](authoring.md) · [Configuration](config.md) · [Waves](waves.md) · [The Agent API](agent-api.md)
