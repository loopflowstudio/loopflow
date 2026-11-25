# Get Started

The first hour: install, one prompt, one Skill, one Task that comes back as a
pull request, and the conversation you pick back up to land it.

## Install

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init                          # checks your harnesses and logins, then suggests a first command
npx skills add loopflowstudio/loopflow --skill loopflow -g   # optional: teach your coding agent to drive lf
```

`lf` installs into `~/.local/bin` (override with `LF_INSTALL_DIR=/path`).
You need `git`, a Git repository, and at least one signed-in coding agent:
Claude Code, Codex, OpenCode, Pi (1.3+), or Antigravity. Pull requests also need `gh` signed
in.

Prefer Cargo? `cargo install --git https://github.com/loopflowstudio/loopflow --bin lf`.
On macOS, [`Loopflow-latest.dmg`](https://downloads.loopflow.studio/Loopflow-latest.dmg)
bundles `lf` and its local server with Loopflow Desktop.

## 1. Launch an agent

```bash
lf : "fix the typo in README"            # uses your default agent
lf -a codex : "fix the typo in README"   # run the same prompt on Codex
```

If you run `lf` on `main`, it first moves your local commits and uncommitted
files into a sibling worktree so the conversation cannot dirty `main`. Bare `lf`
opens a general-purpose conversation with no initial prompt.

If you have more than one harness installed, save a default or an automatic
failover order:

```bash
lf set --global agents claude,codex      # start on Claude; move to Codex on usage limits
```

## 2. Run a Skill

Copy an error message to your clipboard and run one command:

```bash
lf debug -c                     # pastes your clipboard, finds the cause, and fixes it
lf --context debug -c           # preview what the agent would receive without launching
```

Loopflow assembles the operating contract, your `scratch/` notes, and any extra
context you request:

```bash
lf gate --docs 'docs/*.md'      # include files, globs, or directories
lf gate --diff files            # include paths of files changed on this branch
```

Try it on the demo repository:

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py   # see the bug; copy the error
lf debug -c                     # fix it
```

`debug`, `design`, `implement`, `qa`, `gate`, and more are built in. Your own
Skill is a plain Markdown file in `.lf/skills/`:

```markdown
<!-- .lf/skills/audit.md -->
Check this branch for security issues.
Focus on input validation and auth boundaries.
```

```bash
lf audit                        # run it by name
lf audit: focus on the login form
```

## 3. Hand off a Task

A Task gets its own Git worktree, branch, persistent conversation, workflow,
and pull request. Start one and keep working on something else:

```bash
lf task create --title "add a divide-by-zero error"       # prints the Task ID
lf task run <ID> code                                     # creates worktree, builds, publishes PR, then pauses for review
lf steer <ID> "raise ValueError, not ZeroDivisionError"   # redirects the run before its next step
```

`code` is a built-in Graph: it plans, implements in reviewable commits, runs
`qa`, publishes the PR, pauses for your review, then delivers and pauses once
more to verify on `main`. A Task remembers its Graph, so later `lf task run <ID>`
calls continue from where it stopped.

- Set `graph: code` in `.lf/config.yaml` to make it the repo default.
- Run `lf -b task run <ID>` to run straight through interactive stops headless.
- Put `auto: on` beside `graph:` in `.lf/wave/<wave>/config.yaml` to take that
  Wave's Tasks from request to merged PR autonomously; `lf task auto off <ID>`
  takes one back.

Check progress anytime:

```bash
lf task status <ID>              # current step, PR status, and what it needs
lf session list                  # live conversations, OS-confirmed
```

## 4. Pick up the conversation

When the PR is ready, the Task pauses at `ship-review` in its own conversation.
It builds one comprehension page (`scratch/ship-review.html`) covering what was
built, how it works on real diffs, the key data structures and APIs, the
trade-offs going forward, and findings to fix here, file as a Task, or drop.

```bash
lf page open scratch/ship-review.html   # open in your browser or Desktop Files; highlight, choose, and press ⌘↩ to steer
lf resume --task <ID>                   # open the conversation from any terminal, on whichever harness it ran
```

Highlight text on the page to leave notes, pick choices on findings, and press
`⌘↩` to steer the Task directly—or talk in the conversation. Close the terminal
and come back next week: the conversation persists across window closes,
`lf install` updates, crashes, and reboots.

## 5. Land it

```bash
lf task auto on <ID>             # runs remaining steps, then merges; repairs failing CI until it lands
lf task auto off <ID>            # wait at the next interactive step and disarm auto-merge
```

`lf task auto on` turns auto on and runs the Task: an agent stands in at each
remaining interactive step, then `deliver` runs `reflect` (folds your feedback
into skill improvements), `gate` (publishes the PR so CI starts, then reviews
while CI runs), and `lf land --wait-and-fix`. It returns once the run starts,
like `lf task run` (`-b` waits until merge). The Task completes when its PR merges, and
`lf wt prune` cleans up finished worktrees later.

The same landing commands work on any branch outside a Task:

```bash
lf land                          # request auto-merge and return
lf land --wait-and-fix           # stay until merged, repairing CI failures along the way
```

## Keep Loopflow updated

```bash
lf install                       # update to the latest stable build
lf install --preview             # follow every release as it ships
lf install --dev                 # follow green main builds (needs gh auth)
lf schedule weekly               # change auto-update cadence (daily by default; also hourly, 5min)
lf doctor                        # check installation, store, and process health
```

`lf install` also keeps the local client server (used by Loopflow Desktop and
held Task conversations) in sync without interrupting running agents. Check it
with `lf server status`, or see [Connect clients](server.md).

## Common flags

| Flag | What it does |
|---|---|
| `-c` | Append clipboard contents to the prompt |
| `--docs PATH,…` | Include files, globs, or directories |
| `--diff files` / `--diff patch` | Reference changed file paths / raw diff |
| `--context` | Preview assembled input without launching |
| `-a HARNESS[:MODEL]` | Run on `claude`, `codex`, `opencode`, `pi`, or `agy` (Antigravity) |
| `-i` / `-b` | Interactive / headless (waits until finished) |

Next: [Work](work.md): parallel Tasks, steering, stacked PRs, team trackers,
and Waves.
