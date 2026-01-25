# Loopflow

Arrange and conduct an agent orchestra.

## Agents

|  | What it does |
|------|--------------|
| **Flow** | Pipeline of steps (design -> implement -> reduce -> polish) |
| **Goal** | Defines success, tone, and quality |
| **Area** | Subsection of codebase passsed in as context and scope for changes |
| **Stimulus** | How agents get run: cron, file changes, or loops |

An agent is configured by these dimensions: **area × goal × flow × stimulus**.

## Steps

Steps are the building blocks of flows and can also be used for small, atomic changes.

```bash
lf debug -c
```

Thris runs the `debug` prompt, loading the clipboard (`-c`) as context on what to debug. 

## Flows

Steps can be chained together to make flows. You can try it manually on the commandline:

```bash
lf review                 # run review.md
lf implement: add auth    # pass arguments
lf debug -i               # interactive (you guide)
lf polish -a              # autonomous (runs to completion)
```

But you can also define pre-configured flows to be reused by agents.

```bash
lf flow ship: add user auth
```

### Built-in Flows

| Flow | What it does |
|------|--------------|
| `ship` | Design, implement, simplify, polish — full feature workflow |
| `roadmap` | Strategic planning with multiple perspectives |


## Running Agents

Once you have played with chaining steps into flows, you're ready to start playing with agents.

```bash
lfd loop ship src/
```

Runs the `ship` flow on `src/` continuously, creating PRs until stopped.

```bash
lfd loop ship src/ -g product-engineer  # add a goal
lfd subscribe ship docs/ -g designer    # activate when docs/ changes
lfd status                              # see all agents
```

You can compose multiple goals to add additional nuance or perspectives.

```bash
lf review -g designer,product-engineer
lf review -g ceo    
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
