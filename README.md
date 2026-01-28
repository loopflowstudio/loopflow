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

### Planning steps (`plan/`)

| Step | What it does |
|------|--------------|
| `review` | Research an area — architecture, complexity, quality, potential |
| `reduce` | Find simplification opportunities |
| `polish` | Find polish priorities |
| `expand` | Find expansion opportunities |
| `iterate` | Read review, write design to address it |
| `ingest` | Pick roadmap item, move to scratch/ |
| `kickoff` | Elaborate design — alternatives, research, imagine success/failure |
| `roadmap` | Synthesize analysis into a roadmap proposal |
| `5whys` | Root cause analysis on a bug fix |

### Code steps (`code/`)

| Step | What it does |
|------|--------------|
| `debug` | Fix an error |
| `implement` | Build from a design doc |
| `compress` | Simplify touched code |
| `gate` | Ship-ready code and reviewer-friendly docs |

### Interactive steps (`interactive/`)

| Step | What it does |
|------|--------------|
| `design` | Interactive design session |
| `explore` | Investigate the codebase |
| `refine` | Refine existing work |

### Ops steps (`ops/`)

| Step | What it does |
|------|--------------|
| `consolidate` | Reorganize scratch/ for roadmap |
| `add-to-roadmap` | Promote from scratch/ to roadmap/ |
| `synthesize` | Combine multiple perspectives into one |
| `validate` | Validate flows, steps, and directions |

## Flows

```bash
lf design && lf implement && lf gate    # chain steps manually
lf flow ship                            # or use a named flow
```

Steps chain into flows. Flows feed into waves.

### Code flows (`code/`)

| Flow | Steps |
|------|-------|
| `ship` | implement → compress → gate → consolidate |
| `design-and-ship` | design → implement → reduce → polish |
| `pair` | design → ship |
| `grind` | review → iterate → ship → gate |
| `incident` | debug → 5whys → ship |
| `start` | ingest → kickoff |
| `ship-roadmap` | start → ship |

### Plan flows (`plan/`)

| Flow | Steps |
|------|-------|
| `roadmap-reduce` | review → fork(reduce×3) → publish |
| `roadmap-polish` | review → fork(polish×3) → publish |
| `roadmap-expand` | review → fork(expand×3) → publish |
| `research` | explore → review → publish |
| `publish` | consolidate → add-to-roadmap |

### Forks

Forks run a step in parallel with different directions, then synthesize the results.

```bash
lf flow roadmap-reduce    # runs reduce 3x with different perspectives
```

`roadmap-reduce` forks `reduce` across infra-engineer, designer, and product-engineer directions, then synthesizes and publishes to roadmap.

## Playing in the Waves

Once you have played with chaining steps into flows, you're ready to start running waves.

```bash
lfd create engbot                         # create a wave
lfd area engbot src/                      # set area
lfd direction engbot product-engineer     # set direction
lfd flow engbot ship                      # set flow
lfd loop engbot                           # start looping
```

Runs the `ship` flow on `src/` continuously using the `product-engineer` direction, creating PRs until stopped.

```bash
lfd watch ship designs/ -d designer       # ship when designs/ changes on main
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
