# Loopflow

Arrange and conduct an agent orchestra.

## The Model

| Atom | What it does |
|------|--------------|
| **Step** | Runs a prompt with assembled context |
| **Flow** | Chains steps together |
| **Goal** | Shapes judgment and intent |
| **Area** | Focuses on part of the codebase |
| **Stimulus** | When to run: once, loop, watch, cron |

An agent is **area × goal × flow × stimulus**.

## Steps

```bash
lf debug -c
```

Assembles context (docs, style guides, branch diff) and runs the prompt. `-c` adds your clipboard.

```bash
lf review                 # run review.md
lf implement: add auth    # pass arguments
lf debug -i               # interactive (you guide)
lf polish -a              # autonomous (runs to completion)
```

Steps live in `.lf/steps/`.

## Flows

```bash
lf flow ship              # design → implement → polish
```

Or chain manually:

```bash
lf design: add user auth && lf implement && lf polish && lfops pr
```

Define flows in `.lf/flows/ship.py`:

```python
def flow():
    return Flow("design", "implement", "polish")
```

## Agents

```bash
lfd loop ship src/
```

Runs the `ship` flow on `src/` continuously, creating PRs until stopped.

```bash
lfd loop ship src/ -g product-engineer  # add a goal
lfd subscribe ship docs/ -g designer    # activate when docs/ changes
lfd status                              # see all agents
```

Goals compose. The first sets intent. Additional goals add perspective.

```bash
lf review -g designer                     # design quality focus
lf review -g product-engineer,designer    # product focus + design perspective
```

## Install

```bash
uv tool install loopflow
```

Built-in steps and flows included. `lf init` sets up Claude Code and preferences.

[Documentation →](docs/index.md)

## Integrations

**Coding Agents**
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — Anthropic's coding agent (default)
- [Codex CLI](https://github.com/openai/codex) — OpenAI's coding agent
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — Google's coding agent

**Tools**
- [worktrunk](https://github.com/loopflowstudio/worktrunk) — git worktree management (`wt` commands)

**Skill Libraries**
- [superpowers](https://github.com/obra/superpowers) — prompt library (`lf sp:<skill>`)
- [SkillRegistry](https://skillregistry.io/) — remote skill directory (`lf sr:<skill>`)
- [rams](https://rams.ai) — accessibility and visual design review

## Requirements

- macOS
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [Gemini CLI](https://github.com/google-gemini/gemini-cli)

## License

MIT
