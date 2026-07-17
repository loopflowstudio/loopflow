# Loopflow

Loopflow creates and runs **Waves** — persistent agents that work toward an
outcome. Write a Wave's goal once; it coordinates Linear-backed Projects and
Tasks, remembers what it learns, and keeps one steerable conversation beside
the live work map.

Everything is one binary. `lf` is the command humans type *and* the API agents
call to launch, steer, and observe other agents. There is no centralized
server: state lives in a local SQLite store and append-only journals, shared
truth lives in Linear and GitHub, and remote machines are reached over plain
SSH.

## The Model

| Atom | What it does | Where it lives |
|------|--------------|----------------|
| **Skill** | Runs a prompt with assembled context | `.lf/skills/*.md` |
| **Flow** | Chains skills together | `.lf/flows/*.yaml` |
| **Wave** | Durable operating context with memory, cadence, chat, and project selection | `wave/<name>/` |
| **Goal** | A wave's intent and loop prompt | `wave/<name>/GOAL.md` |
| **Memory** | What a wave remembers between loops | `wave/<name>/MEMORY.md` |
| **Project** | Measured bet inside exactly one wave | Linear, via `lf pm` |
| **Task** | Concrete work that advances a project; owns the only delivery worktree | Linear, via `lf pm` |
| **Direction** | Shapes judgment and intent | `.lf/directions/*.md` |
| **Home** | Where a wave's work executes — an owner plus a location, local or `ssh://` | `GOAL.md` frontmatter |

A wave is a named agent with a goal. Everything that defines its durable
operating context — goal, memory, crons, Home — is authored in the repo and
reviewed like code. Projects and Tasks live in Linear. Only a Task Session
owns a worktree; that is where every file change happens.

## Try it

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
lf debug -c        # copy an error to the clipboard, watch it fix
```

Then author a Wave and let it run. **Loopflow** (macOS) is the home for
running waves — open the repository and start it there — or start it from the
CLI:

```bash
# author wave/engbot/GOAL.md, then:
lf home start engbot      # idempotently start the Wave on its Home
lf chat --steer "ship the parser fix first"
lf status engbot          # its live Project → Task hierarchy
lf stop engbot
```

## The Journey

| Where you are | What to read |
|---|---|
| Just installed, want to try it | [Get Started](getting-started.md) |
| Automating with a persistent agent | [Waves](waves.md) |
| Writing skills, flows, and goals | [Authoring](authoring.md) |
| Writing an agent that drives other agents | [The Agent API](agent-api.md) |
| Watching and steering many agents | [Conducting](conducting.md) |
| Understanding how it works with no server | [Architecture](architecture.md) |
| Looking up a command | [`lf` reference](lf.md) |

## Skills, flows, directions

A skill is a markdown file that tells the coding agent what to do:

```markdown
# .lf/skills/audit.md

Audit auth changes on this branch.
Check for missing validation, confusing errors, gaps in tests.
Fix any issues you find.
```

```bash
lf audit                      # run the skill
lf audit: focus on auth       # pass arguments
lf : "fix the typo in README" # or skip the file entirely
```

A flow chains skills with commits between them:

```yaml
# .lf/flows/ship-api.yaml
- implement
- lint
- gate
```

```bash
lf ship-api
```

A direction shapes how the agent judges:

```bash
lf gate --direction ux            # optimize for user experience
lf gate --direction ux,clarity    # stack intents
```

Built-ins cover the common ground: `debug`, `design`, `implement`, `gate`,
`qa`, `lint`, the `build` flow, and more. Repo skills in `.lf/skills/`
override and extend them. [Authoring](authoring.md) covers writing each of
these well.

## Context

Every skill sees your agent doc (`AGENTS.md` / `CLAUDE.md`), `LOOPFLOW.md`,
`scratch/`, and `wave/`. Nothing else is auto-injected — pull in more
explicitly:

```bash
lf gate --docs VISUAL_DESIGN.md      # one doc
lf gate --docs 'docs/*.md'           # a glob
lf gate --diff-files                 # bodies of files changed on the branch
lf debug -c                          # the clipboard
```

## Where files live

```
.lf/                      # Repo config and extensions
  config.yaml             # Model, context defaults
  skills/                 # Skill prompts
  directions/             # Judgment and intent
  flows/                  # Flow definitions
scratch/                  # PR scratchpad (cleared on merge)
wave/                     # Wave goals and memory (persists)
~/.lf/                    # Global config, skills, and the local store
```

`scratch/` dies with the PR; `wave/` lives forever. Design docs go in
`scratch/`, forward-looking plans in `wave/`.

## Next

[Get Started →](getting-started.md) · [Waves →](waves.md) · [The Agent API →](agent-api.md) · [Conducting →](conducting.md)

## Reference

[`lf` commands](lf.md) · [Authoring](authoring.md) · [Configuration](config.md) · [Architecture](architecture.md) · [Troubleshooting](troubleshooting.md)
