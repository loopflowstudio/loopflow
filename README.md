# Loopflow

Loopflow helps you maintain flow and craft using coding agents (Claude Code, Codex, Gemini CLI) at high scale.

Loopflow helps you create and run **Waves**. Waves are chains of coding agents working together in pre-defined ways.  

Waves are first built manually through more interactive exploration. Eventually waves become autonomous through looping, scheduling, and watching for changes.

## Waves

Waves are objects with 4 primary fields.

| Field | Usage | Form |
|-------|------|------|
| **Area** | Scope and context | pathset |
| **Flow** | Process followed / steps taken | sequence of prompts |
| **Direction** | Defines success, quality, and aesthetics | prompt |
| **Stimulus** | Watch, loop, or cron | mode |

## Steps

```bash
lf debug -c    # paste an error, watch it fix
lf design      # interactive design session
```

Steps are prompts that run coding agents. Add your own in `.lf/steps/`.

| Built-in Step | What it does |
|------|--------------|
| `debug` | Fix an error |
| `design` | Interactive design session |
| `design-doc` | Pick roadmap item, write design |
| `implement` | Build from a design doc |
| `iterate` | Read review, write design to address it |
| `reduce` | Simplify touched code |
| `reduce-big` | Strategic simplification analysis |
| `expand` | Extend working code ambitiously |
| `explore` | Investigate the codebase |
| `review` | Thorough area review |
| `polish` | Ship-ready code and reviewer-friendly docs |
| `polish-big` | Strategic polish analysis |
| `roadmap` | Reflect on learnings → forward direction |
| `5whys` | Root cause analysis |

## Flows

```bash
lf design && lf implement && lf polish    # chain steps manually
lf flow ship                              # or use a named flow
```

Steps chain into flows. Flows feed into waves.

| Flow | Steps |
|------|-------|
| `ship` | implement → reduce → polish |
| `pair` | design → ship |
| `grind` | review → iterate → ship → expand → polish |
| `research` | explore → design → roadmap |
| `ship-roadmap` | design-doc → ship |
| `plan-reduce` | fork(reduce-big×3) → roadmap |
| `plan-review` | fork(review×3) → roadmap |
| `plan-polish` | fork(polish-big×3) → roadmap |
| `debug-big` | debug → 5whys → ship |

### Forks

Forks run a step in parallel with different directions, then synthesize the results.

```bash
lf flow plan-review    # runs review 3x with different perspectives
```

`plan-review` forks `review` across infra-engineer, designer, and product-engineer directions, then feeds the combined analysis into `roadmap`.

## Playing in the Waves

Once you have played with chaining steps into flows, you're ready to start running waves.

```bash
lfd create engbot --area src/ --direction product-engineer --flow ship
```

Runs the `ship` flow on `src/` continuously using the `product-engineer` direction, creating PRs until stopped.

```bash
lfd loop engbot      # keep shipping continuously
lfd subscribe ship designs/ -d designer   # or only ship when new designs arrive
```

You can compose multiple directions to add additional nuance or perspectives.

```bash
lf review -d designer,product-engineer
lf review -d ceo
```

## Install

```bash
uv tool install loopflow
```

Built-in steps and flows included. `lf init` sets up your coding agent and preferences.

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
