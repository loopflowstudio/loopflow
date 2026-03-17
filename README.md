# Loopflow

Loopflow helps you maintain flow and craft using coding agents (Claude Code, Codex, OpenCode) at high scale.

Loopflow helps you create and run **Waves**. Waves are chains of coding agents working together in pre-defined ways.  

Waves are first built manually through more interactive exploration. Eventually waves become autonomous through looping, scheduling, and watching for changes.

## Waves

A wave is **area × direction × flow**.

| Field | Usage | Form |
|-------|------|------|
| **Area** | Scope and context | pathset |
| **Flow** | Process followed / steps taken | sequence of prompts |
| **Direction** | Defines success, quality, and aesthetics | prompt |

```yaml
# wave/designer/designer.yaml
flow: build
mode: loop
direction:
  - ux
area:
  - designs/
triggers:
  - signal: wave
    source_wave_id: infra
    flow: build
```

### Modes

The wave's `mode` controls its execution pattern.

| Mode | Behavior | Example |
|------|----------|---------|
| **manual** | Single run | Ship one feature, run one audit |
| **loop** | Continuous until stopped | Work through a backlog, grind PRs |
| **cron** | On a schedule | Daily QA pass, weekly dependency scan |

### Triggers

A trigger pairs a signal (what changed) with a flow (what to run). Triggers are a list — multiple triggers of the same signal are fine.

| Signal | What changed | Default flow |
|--------|--------------|--------------|
| **repo** | Paths changed on main | `integrate` |
| **wave** | Another wave completed | `build` |
| **ci_failure** | CI failed on a wave PR | `ci-fix` |

Every new wave ships with two default triggers: `repo` (whole repo → integrate) and `ci_failure` → `ci-fix`. These don't need to be declared in the YAML.

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
| `integrate-upstream` | Adapt wave code after rebasing onto main |
| `implement` | Build from a design doc |
| `compress` | Simplify touched code |
| `gate` | Ship-ready code and reviewer-friendly docs |
| `qa` | Thorough quality assessment of the current branch |
| `triage` | Assess QA findings, separate blocking from polish |

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
| `release-notes` | Write narrative `RELEASE_NOTES.md` from release context |
| `pr` | Generate PR title/body and call `lf ops pr --title --body` |

## Flows

```bash
lf design && lf implement && lf gate    # chain steps manually
lf build                                # or use a named flow
```

Steps chain into flows. Flows feed into waves.

Flows can include mechanical ops items directly:

```yaml
- implement
- gate
- ops: land --create-pr
```

### Code flows (`code/`)

| Flow | Steps |
|------|-------|
| `build` | implement → compress → lint → gate → update-wave |
| `design-and-ship` | design → implement → reduce → polish |
| `ship` | design → build → review → land |
| `pair` | design → build |
| `grind` | research → iterate → build → gate |
| `integrate` | rebase → integrate-upstream |
| `incident` | debug → 5whys → build |
| `start` | ingest → kickoff |
| `ship-wave` | start → build |
| `ship-roadmap` | ingest → kickoff → review-design → build → review → land |
| `qa-deploy` | qa → triage → branch(fix: qa-fix, deploy: deploy) |
| `qa-fix` | implement → compress → lint → gate |
| `deploy` | gate → update-wave |

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
lf wave-reduce    # runs reduce 3x with different perspectives
```

`wave-reduce` forks `reduce` across infra, ux, and ceo directions, then reconciles results with `update-wave`.

### Branches

Branches route a flow based on an agent's assessment of the current state.

```yaml
# flow: qa-deploy
- qa
- triage
- branch:
    paths:
      fix:
        flow: qa-fix
        description: "Blocking issues found, fix before deploy"
      deploy:
        flow: deploy
        description: "Clean enough to ship"
```

The branch construct runs a routing agent that reads scratch/ and chooses a path. The selected sub-flow runs inline. Combined with cron scheduling and loop iteration, this enables cycles like daily QA → fix → deploy.

## Playing in the Waves

Once you have played with chaining steps into flows, you're ready to ride some waves.

```bash
lfq create engbot .                # create a wave
lfq run engbot                            # ride a wave
```

Configure flow/area/direction with `loopflow.update_wave(...)`, then ride it with `loopflow.run_wave(...)`.

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

First install guides you through connecting Claude, GitHub, and optional providers. Use `--no-interactive` to skip (CI, Docker, scripted installs).

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
lfq show engbot      # show wave details
lfq logs engbot      # tail agent output
lfq stop engbot      # stop a running wave
lfq delete engbot    # remove wave and history
lfq usage            # token usage summary (group by wave)
lfq usage --wave engbot  # usage for one wave (group by step)
lfq providers        # list providers with auth status and models
lfq auth status      # provider auth status (GitHub / Claude / Codex / OpenCode Zen)
lfq auth github      # connect GitHub in your browser
lfq auth claude      # connect Claude in your browser
lfq auth codex       # connect Codex in your browser
lfq auth zen         # connect OpenCode Zen in your browser
lfq auth disconnect github
lfq token revoke abc123   # revoke connection tokens by hash prefix
lfq token revoke --all    # revoke all connection tokens
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
loopflow.add_trigger("ux", signal="wave", source_wave_id="infra")
loopflow.run_wave("ux")
```

```python
import loopflow.api as loopflow

loopflow.create_wave("redesign", repo=".")
redesign = loopflow.wave("redesign")
print(redesign.primary_flow)
print(redesign.area)
```

[Documentation →](docs/index.md)

## tmux Plugin

```bash
# Add to .tmux.conf
set -g @plugin 'loopflowstudio/loopflow.tmux'
run '~/.tmux/plugins/tpm/tpm'
```

Status bar shows wave state: `[lf: main]` or `[lf: 3 waves | engbot]`. Customize the format:

```bash
# .tmux.conf
set -g @loopflow_status_format '⚡#{status}'       # change wrapper
set -g @loopflow_status_format '[#{branch}]'        # branch only
set -g @loopflow_status_format '[lf: #{status}]'    # default
```

Variables: `#{status}` (computed text), `#{branch}`, `#{step}`, `#{waves}`, `#{wave}`.

Keybindings start with `prefix+l`:

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
- [OpenCode](https://github.com/anomalyco/opencode) — Open source coding agent

**Skill Libraries**
- [npx skills](https://github.com/vercel-labs/skills) — install/search skills (`lf npx:<skill>`)
- [superpowers](https://github.com/obra/superpowers) — prompt library (`lf sp:<skill>`)
- [SkillRegistry](https://skillregistry.io/) — remote skill directory (`lf sr:<skill>`)
- [rams](https://rams.ai) — accessibility and visual design review

## Requirements

- macOS or Linux
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [OpenCode](https://github.com/anomalyco/opencode)
- Concerto (visual app) is macOS-only

## License

MIT
