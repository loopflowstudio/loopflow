---
layout: default
title: Loopflow
---

# Loopflow

Orchestrate waves of autonomous work.

## Try it

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -c                       # fix it
```

## Query lfd

```bash
uv tool install loopflow
lfq                  # status overview
lfq list             # list waves
lfq logs engbot      # tail agent output
```

## Python API

```bash
uv pip install loopflow
```

```python
import loopflow.api as loopflow

loopflow.waves()
loopflow.create_wave("engbot", repo=".", flow="build", direction=["clarity"])
loopflow.run_wave("engbot")
```

---

## Why Flows?

Steps are atomic. Flows are how work actually gets done.

**Linear flows** chain steps with automatic commits:
```
design → implement → polish
```

**Parallel flows** branch and join:
```
design ──┬──> impl-api ──┬──> integrate
         └──> impl-ui ───┘
```

**Fork** explores multiple approaches and synthesizes:
```
Fork ──┬──> impl (infra) ──┐
       ├──> impl (ux)    ──┼──> synthesize
       └──> impl (ceo)   ──┘
```

The synthesizer doesn't just pick a winner—it documents why approaches differed.

---

## The Model

| Atom | What it does | File |
|------|--------------|------|
| **Step** | Runs a prompt with assembled context | `.lf/steps/*.md` |
| **Flow** | Chains steps together | `.lf/flows/*.py` |
| **Direction** | Shapes judgment and intent | `.lf/directions/*.md` |
| **Area** | Focuses on part of the codebase | path argument |
| **Stimulus** | When to run: once, loop, watch, cron | command |

A wave is **area × direction × flow × stimulus**.

Area is the path you pass—not a file. It scopes what the wave sees and changes.

| Stimulus | Runs when |
|----------|-----------|
| **Once** | Single run (one-shot) |
| **Loop** | Continuously until stopped |
| **Watch** | Area changes on main |
| **Cron** | On schedule |

---

## Step

A markdown file that tells the coding agent what to do.

```markdown
# .lf/steps/review.md

Review the code on this branch. Check for:
- Correctness
- Test coverage
- Style guide compliance

Fix any issues you find.
```

```bash
lf review                     # run the step
lf review: focus on auth      # pass arguments
```

Steps run to completion. Built-ins: `debug`, `design`, `implement`, `polish`, `review`, `lint`.

**Where to put steps:** `.lf/steps/` is canonical. Symlink for Claude Code compatibility:

```bash
ln -s ../.lf/steps .claude/commands
```

---

## Flow

Chains steps together with commits between them.

```yaml
# .lf/flows/build.yaml
- implement
- compress
- gate
- update-wave
```

```bash
lf --flow build
```

Or chain manually:

```bash
lf design: add auth && lf implement && lf polish
```

---

## Direction

Shapes how the coding agent judges and responds.

```markdown
# .lf/directions/ux.md

Optimize for user experience quality: visibility, feedback, consistency.

## Success

A design doc in scratch/ that another engineer could implement from.
```

```bash
lf review --direction ux
lf review --direction ux,clarity    # stack multiple
```

Directions compose. A `ux` direction sets user-facing intent. A `clarity`
direction adds code-model rigor. Stack them to get both.

---

## Area

The path you pass to `lfq`/`loopflow` when configuring waves. Scopes what the wave works on.

```bash
lfq create shipper .
python - <<'PY'
import loopflow.api as loopflow

loopflow.update_wave("shipper", flow="build", area=["src/api/"])
loopflow.run_wave("shipper")
PY
```

Combined with flow and direction, area defines the wave:

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.update_wave("shipper", flow="build", area=["src/api/"], direction=["clarity"])
loopflow.run_wave("shipper")
PY
```

---

## Where Files Live

```
.lf/                      # Repo config and extensions
  config.yaml             # Model, context defaults
  steps/                  # Step prompts (preferred)
  directions/             # Judgment and intent
  flows/                  # Flow definitions
.claude/commands/         # Steps (Claude Code compatible)
scratch/                  # PR scratchpad (cleared on merge)
wave/                     # Wave plans (persists)
~/.lf/                    # Global config and steps
```

### scratch/ vs wave/

| | scratch/ | wave/ |
|---|---|---|
| **Lifespan** | Dies with the PR | Lives forever |
| **Purpose** | Current work | Forward-looking plans |
| **Location** | Root only | Root + per-folder |
| **Example** | "Add auth" spec | "How auth should work long-term" |

`wave/` can exist at any level:
- Root `wave/` is auto-included (part of lfdocs)
- `src/api/wave/` holds API-specific plans
- Including an area via `--area` auto-includes its nested `wave/` folders

### What's Auto-Included

Every step sees: `README.md`, `STYLE.md`, `CLAUDE.md`, `scratch/`, `wave/`, and files touched by your branch.

---

## Next

[Waves →](waves.md)

## Reference

[`lf` commands](lf.md) · [`lf ops` commands](lfops.md) · [`lfd` commands](lfd.md) · [Configuration](config.md)
