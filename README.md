# Loopflow

Loopflow helps you maintain flow and craft using coding agents (Claude Code, Codex, Gemini CLI, OpenCode) at high scale.

Loopflow helps you create and run **Waves**. Waves are chains of coding agents working together in pre-defined ways.  

Waves are first built manually through more interactive exploration. Eventually waves become autonomous through looping, scheduling, and watching for changes.

## Waves

Waves are objects with 4 primary fields.

| Field | Usage | Form |
|-------|------|------|
| **Area** | Scope and context | pathset |
| **Flow** | Process followed / steps taken | sequence of prompts |
| **Direction** | Defines success, quality, and aesthetics | prompt |
| **Stimulus** | Watch, loop, cron, or listen | mode |

```yaml
# wave/designer/designer.yaml
flow: build
stimulus:
  kind: listen
  source: infra
  source_repo: /Users/jack/src/other-repo # optional
```

## Steps

```bash
lf debug -c    # paste an error, watch it fix
lf design      # interactive design session
lf npx:explain-code   # fetch from npx skills ecosystem and run
```

Steps are prompts that run coding agents. Add your own in `.lf/steps/`.

### Planning steps (`plan/`)

| Step | What it does |
|------|--------------|
| `research` | Map the territory — architecture, complexity, quality, potential |
| `reduce` | Find simplification opportunities |
| `polish` | Find polish priorities |
| `expand` | Find expansion opportunities |
| `iterate` | Read research, write design to address it |
| `ingest` | Pick wave item, move to scratch/ |
| `kickoff` | Elaborate design — alternatives, research, imagine success/failure |
| `5whys` | Root cause analysis on a bug fix |

### Code steps (`code/`)

| Step | What it does |
|------|--------------|
| `debug` | Fix an error |
| `ci-fix` | Fix failing CI checks for the current PR |
| `implement` | Build from a design doc |
| `compress` | Simplify touched code |
| `gate` | Ship-ready code and reviewer-friendly docs |

### Interactive steps (`interactive/`)

| Step | What it does |
|------|--------------|
| `design` | Interactive design session |
| `explore` | Investigate the codebase |
| `review` | Walk through the diff, evaluate model and decisions |
| `review-design` | Walk through the design doc, evaluate before implementation |
| `refine` | Refine existing work |

### Scan steps (`scan/`)

| Step | What it does |
|------|--------------|
| `scan/scan-report` | Scan deps and APIs for vulnerabilities, staleness, breaking changes |
| `scan/scan-plan` | Turn scan report into actionable design doc |

### Ops steps (`ops/`)

| Step | What it does |
|------|--------------|
| `update-wave` | Create, update, or delete wave state |
| `split-wave` | Split a wave into smaller independent waves |
| `synthesize` | Combine multiple perspectives into one |
| `validate` | Validate flows, steps, and directions |
| `release` | Run the full release workflow (notes, PR, tag, status) |

## Flows

```bash
lf design && lf implement && lf gate    # chain steps manually
lf flow build                           # or use a named flow
```

Steps chain into flows. Flows feed into waves.

### Code flows (`code/`)

| Flow | Steps |
|------|-------|
| `build` | implement → compress → lint → gate → update-wave |
| `design-and-ship` | design → implement → reduce → polish |
| `ship` | design → build → review |
| `pair` | design → build |
| `grind` | research → iterate → build → gate |
| `incident` | debug → 5whys → build |
| `start` | ingest → kickoff |
| `ship-wave` | start → build |
| `ship-roadmap` | ingest → kickoff → review-design → build → review |

### Plan flows (`plan/`)

| Flow | Steps |
|------|-------|
| `wave-reduce` | fork(reduce×3) → update-wave |
| `wave-polish` | fork(polish×3) → update-wave |
| `wave-expand` | fork(expand×3) → update-wave |

### Scan flows (`scan/`)

| Flow | Steps |
|------|-------|
| `scan` | scan/scan-report → scan/scan-plan → build |

### Forks

Forks run a step in parallel with different directions, then synthesize the results.

```bash
lf flow wave-reduce    # runs reduce 3x with different perspectives
```

`wave-reduce` forks `reduce` across infra, ux, and ceo directions, then reconciles results with `update-wave`.

## Playing in the Waves

Once you have played with chaining steps into flows, you're ready to start running waves.

```bash
lfq create engbot .                # create a wave
lfq run engbot                            # start a wave
```

Configure flow/area/direction with `loopflow.update_wave(...)`, then start it with `loopflow.run_wave(...)`.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.update_wave("engbot", flow="build", direction=["ux"], area=["designs/"])
loopflow.run_wave("engbot")
PY
```

You can compose multiple directions to add additional nuance or perspectives.

```bash
lf research -d ux,clarity
lf research -d ceo
```

## Install

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
```

Default install location is `~/.local/bin`. Override with `LF_INSTALL_DIR=/path`.

Built-in steps and flows included. `lf init` sets up your coding agent and preferences.

```bash
cargo install --git https://github.com/loopflowstudio/loopflow --bin lf --bin lfd
```
Install the Rust binaries directly with cargo.

## Query lfd (lfq)

```bash
uv tool install loopflow
lfq                  # status overview
lfq list             # list waves
lfq logs engbot      # tail agent output
lfq auth status      # provider auth status (GitHub / Claude / Codex / OpenCode Zen)
lfq auth github      # connect GitHub in your browser
lfq auth claude      # connect Claude in your browser
lfq auth codex       # connect Codex in your browser
lfq auth disconnect github
```

`uv tool install loopflow` installs the Python CLI (`lfq`) and Python API only.  
Use the install script or cargo to install `lf` and `lfd`.

## Python API

```bash
uv pip install loopflow
```

```python
import loopflow.api as loopflow

loopflow.waves()
loopflow.create_wave("engbot", repo=".", flow="build", direction=["clarity"])
loopflow.create_wave("ux", repo=".", flow="build", direction=["ux"], area=["docs/"])
loopflow.create_wave("infra", repo=".", flow="grind", direction=["infra"], area=["rust/"])
loopflow.add_stimulus("ux", kind="listen", source_wave_id="infra")
loopflow.run_wave("ux")
```

```python
import loopflow.api as loopflow

chord = loopflow.create_chord("frontend")
waves = loopflow.waves()
loopflow.add_chord_member(chord.id, waves[0].id)
loopflow.list_chord_members(chord.id)
loopflow.list_wave_chords(waves[0].id)
loopflow.remove_chord_member(chord.id, waves[0].id)
```

[Documentation →](docs/index.md)

## tmux Plugin

```bash
# Add to .tmux.conf
set -g @plugin 'loopflowstudio/loopflow.tmux'
run '~/.tmux/plugins/tpm/tpm'
```

Status bar shows wave state: `[lf: main]` or `[lf: 3 waves | engbot]`. Keybindings start with `prefix+l`:

| Key | Action |
|-----|--------|
| `r` | Run step/wave |
| `s` | Stop |
| `o` | Open logs |
| `p` | Open PR |
| `n` | Next iteration |
| `d` | Land PR |
| `u` | Start/bootstrap |
| `w` | Pick wave/worktree |
| `L` | Pick layout |
| `?` | Help |

Two built-in layouts: `lf-dev` (editor + agent + shell), `lf-swarm` (monitor + 3 worktree workers).

Works without `lf` or `lfq` installed — status shows placeholder, keybindings display clear messages.

## Integrations

**Coding Agents**
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — Anthropic's coding agent (default)
- [Codex CLI](https://github.com/openai/codex) — OpenAI's coding agent
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — Google's coding agent
- [OpenCode](https://github.com/anomalyco/opencode) — Open source coding agent

**Skill Libraries**
- [npx skills](https://github.com/vercel-labs/skills) — install/search skills (`lf npx:<skill>`)
- [superpowers](https://github.com/obra/superpowers) — prompt library (`lf sp:<skill>`)
- [SkillRegistry](https://skillregistry.io/) — remote skill directory (`lf sr:<skill>`)
- [rams](https://rams.ai) — accessibility and visual design review

## Requirements

- macOS
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), [Gemini CLI](https://github.com/google-gemini/gemini-cli), or [OpenCode](https://github.com/anomalyco/opencode)

## License

MIT
