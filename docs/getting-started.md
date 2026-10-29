# Get Started

The first hour: install, one prompt, one Skill, one Task that comes back as a
pull request, and the conversation you pick back up to land it.

## Install

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init                          # checks your agent and logins, then names one command to try
```

`lf` lands in `~/.local/bin`; set `LF_INSTALL_DIR=/path` to put it elsewhere.
You need `git`, a project in a git repository, and one of Claude Code, Codex,
OpenCode or Pi (1.3+) signed in. Pull requests also need `gh`, signed in.

## 1. Launch an agent

```bash
lf : "fix the typo in README"            # on whichever of your logins has room
lf -a codex : "fix the typo in README"   # the same prompt on Codex
```

Run on `main`, and lf first carries your local commits and uncommitted files
into a sibling worktree, so the conversation cannot dirty `main`. Bare `lf`
opens the same general-purpose conversation without a prompt.

## 2. Run a Skill

Copy an error, run one command.

```bash
lf debug -c                     # paste the clipboard; it finds and fixes the cause
lf --context debug -c           # print what the agent would receive, launch nothing
```

`-c` pastes your clipboard. lf assembles the rest: operating guidance, your
`scratch/` notes, and any docs you name.

```bash
lf gate --docs 'docs/*.md'      # add files, globs or directories
lf gate --diff files            # add the bodies of files changed on the branch
```

Try it on the demo repo:

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug; copy the error
lf debug -c                      # fix it
```

`debug`, `design`, `implement`, `gate` and `qa` are built in. Your own Skill is a
Markdown file:

```markdown
<!-- .lf/skills/audit.md -->
Check this branch for security issues.
Focus on input validation and auth boundaries.
```

```bash
lf audit                         # runs your Skill by name
lf audit: focus on the login form
```

## 3. Hand off a Task

A Task gets its own worktree, branch and pull request. Start one and walk away:

```bash
lf task create --title "add a divide-by-zero error"   # prints the Task ID
lf task run <ID> code                                 # its own worktree; publishes a PR, then waits for you
lf comment <ID> "raise ValueError, not ZeroDivisionError"   # reaches the run before its next step
```

`code` is the built-in Graph: build until it holds and publish the PR, stop for
you to try the change, then deliver it and stop again for you to check it on
`main`. Name it once; the Task keeps it, and each `lf task run <ID>` continues
from where it stopped. Set `graph: code` in `.lf/config.yaml` and every new Task
takes it up. `lf -b task run <ID>` runs straight through both stops with an agent
standing in; `auto` has no stops: with `graph: auto` in
`.lf/wave/<wave>/config.yaml`, that Wave's Tasks run from request to merged PR
without you.

While it runs:

```bash
lf task status <ID>              # where it is, and why
lf ps                            # live lf call trees: each Graph, its steps, their agents
```

## 4. Pick up the conversation

When the PR is published, the Task waits at its demo: you try the change, in the
Task's own conversation.

```bash
lf resume --task <ID>            # from any terminal, on whatever agent it ran
```

Ask for changes there. Close the window; the conversation is still there next
week. After a crash or reboot, `lf resume --all` brings back every conversation
it interrupted.

## 5. Land it

```bash
lf ship <ID>                     # the remaining steps, then merge; repairs failing CI until it lands
lf ship <ID> --stop              # withdraw it; disarms the auto-merge it armed
```

`lf ship` stands in for you at each remaining interactive step and runs `deliver`: `gate`,
then `lf land --wait-and-fix`. It returns once the Task is shipping (`-b` waits);
`lf task status` follows it. The Task completes when the PR merges, and its
worktree is cleaned up later by `lf wt prune`.

Outside a Task, the same verbs work on any branch:

```bash
lf land                          # request auto-merge and return
lf land --wait-and-fix           # stay until it merges, fixing CI on the way
```

## Keep it current

```bash
lf install                       # the latest published release, from any directory
lf install --main                # the newest green main build (needs gh auth)
lf schedule                      # check at login and weekly (macOS); also daily, hourly, 5min
lf doctor                        # installation, store and process health
```

Prefer cargo? `cargo install --git https://github.com/loopflowstudio/loopflow --bin lf`.
The Mac app, [`Loopflow-latest.dmg`](https://downloads.loopflow.studio/Loopflow-latest.dmg),
bundles `lf`.

| Flag | What it does |
|---|---|
| `-c` | Paste the clipboard into the prompt |
| `--docs PATH,…` | Add files, globs or directories |
| `--diff files` / `--diff patch` | Add changed file bodies / the raw diff |
| `--context` | Preview the assembled input; launch nothing |
| `-a HARNESS[:MODEL]` | Run on claude, codex, opencode or pi |
| `-i` / `-b` | Interactive / headless, returning when the work ends |

Next: [Work](work.md): many Tasks, their status, and Waves that keep a longer
objective moving.
