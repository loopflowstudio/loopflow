# gstack integration + workstyle infrastructure

## What to build

Import gstack's prompt sequence as loopflow flows and steps, with tooling to sync from the upstream repo. This is the first external workstyle — the concrete work that proves the workstyle abstraction.

## The three launch workstyles

| Name | Philosophy | Source | Status |
|------|-----------|--------|--------|
| **lfjack** | Design → implement → gate → ship. Interactive, craft-oriented. | Builtin (current default) | Already exists |
| **vsm** | VSM governance. Scan → assess → mutate across five system levels. | Builtin | Already exists (as vsm/ flows) |
| **gstack** | Sprint factory. Think → plan → build → review → test → ship → reflect. | github:garrytan/gstack | **Needs building** |

## gstack → loopflow mapping

gstack has 28 SKILL.md files. They map to loopflow steps. The sprint sequence maps to flows.

### Step mapping

| gstack skill | loopflow step name | Phase |
|---|---|---|
| office-hours | `gstack:office-hours` | think |
| plan-ceo-review | `gstack:ceo-review` | plan |
| plan-design-review | `gstack:design-review` | plan |
| plan-eng-review | `gstack:eng-review` | plan |
| autoplan | `gstack:autoplan` | plan |
| design-consultation | `gstack:design-consultation` | plan |
| review | `gstack:review` | review |
| investigate | `gstack:investigate` | review |
| design-review | `gstack:design-review-live` | review |
| codex | `gstack:codex` | review |
| cso | `gstack:cso` | review |
| qa | `gstack:qa` | test |
| qa-only | `gstack:qa-only` | test |
| benchmark | `gstack:benchmark` | test |
| canary | `gstack:canary` | test |
| ship | `gstack:ship` | ship |
| land-and-deploy | `gstack:land-and-deploy` | ship |
| document-release | `gstack:document-release` | ship |
| retro | `gstack:retro` | reflect |

Utility skills (careful, freeze, guard, unfreeze, browse, setup-*) become hooks or config, not flow steps.

### Flow mapping

```yaml
# .lf/flows/gstack-sprint.yaml
# Full sprint: think → plan → build → review → test → ship → reflect
- gstack:office-hours
- xor:
    router: gstack:office-hours
    paths:
      autoplan:
        step: gstack:autoplan
        description: "Auto-plan with minimal interaction"
      manual:
        flow: gstack-plan-manual
        description: "Interactive planning with CEO/design/eng reviews"
- implement
- gstack:review
- gstack:qa
- gstack:ship
- gstack:retro
```

```yaml
# .lf/flows/gstack-plan-manual.yaml
- gstack:ceo-review
- gstack:design-review
- gstack:eng-review
```

```yaml
# .lf/flows/gstack-review.yaml
# Deep review: code + security + optional cross-model
- gstack:review
- gstack:cso
- gstack:codex
```

## Sync tooling

### `lf ops workstyle sync`

Pulls SKILL.md files from a git repo, strips gstack-specific preamble (telemetry, update checks, onboarding), converts to loopflow step format, and writes to a local cache.

```
garrytan/gstack repo
  └─ office-hours/SKILL.md
  └─ review/SKILL.md
  └─ ...
       │
       ▼  lf ops workstyle sync gstack
       │
.lf/workstyles/gstack/
  └─ workstyle.yaml          # metadata + sync config
  └─ steps/
      └─ office-hours.md     # converted step
      └─ review.md           # converted step
      └─ ...
  └─ flows/
      └─ sprint.yaml         # the main flow
      └─ plan-manual.yaml
      └─ review.yaml
```

### Conversion: SKILL.md → loopflow step

gstack SKILL.md files have:
1. YAML frontmatter (name, version, description, allowed-tools, benefits-from)
2. A preamble bash block (telemetry, update checks, session tracking)
3. Voice section (shared across all skills — the "gstack voice")
4. Skill-specific instructions

The converter:
- **Keeps**: frontmatter (mapped to loopflow fields), skill-specific instructions, **voice section**
- **Strips**: preamble bash block, telemetry, update checks, onboarding flows
- **Maps**: `benefits-from` → loopflow step dependencies, `allowed-tools` → step config

The gstack voice is preserved — it's part of the workstyle identity. When a gstack flow is active, gstack's voice takes priority over loopflow's default voice. The voice section from SKILL.md files gets extracted into `.lf/workstyles/gstack/voice.md` and injected at runtime the same way `.lf/voice.md` works today.

```python
def convert_gstack_skill(skill_md: str) -> str:
    """Convert a gstack SKILL.md to a loopflow step .md"""
    frontmatter, body = split_frontmatter(skill_md)

    # Map gstack frontmatter to loopflow frontmatter
    lf_frontmatter = {
        "interactive": True,  # most gstack skills are interactive
        "source": f"gstack:{frontmatter.get('name', 'unknown')}",
    }

    # Strip preamble bash block (telemetry, updates, onboarding)
    body = strip_preamble_block(body)

    # Keep voice section — it's the gstack identity
    # (extracted separately into workstyle voice.md during first sync)

    return format_step(lf_frontmatter, body)
```

### workstyle.yaml

```yaml
# .lf/workstyles/gstack/workstyle.yaml
name: gstack
description: "Garry Tan's sprint factory — think, plan, build, review, test, ship, reflect"
source:
  repo: garrytan/gstack
  ref: main                    # branch/tag to sync from
  last_sync: 2026-03-27T12:00:00Z
  last_commit: abc1234

# What this workstyle provides
steps:
  prefix: gstack               # steps are available as gstack:office-hours, etc.
  path: steps/                  # relative to workstyle dir

flows:
  - sprint                     # main flow
  - plan-manual
  - review

# Optional: config this workstyle sets
config:
  direction: []                # gstack doesn't prescribe directions
```

### Sync behavior

- `lf ops workstyle sync gstack` — pull latest from repo, convert, write
- `lf ops workstyle sync --all` — sync all workstyles with remote sources
- `lf ops workstyle diff gstack` — show what changed upstream since last sync
- Sync is explicit, not automatic. User controls when prompts update.

## Discovery integration

The step discovery chain (`discovery.rs`) already supports external sources. A workstyle is a new `SkillSourceKind`:

```rust
pub enum SkillSourceKind {
    Directory,
    SingleFile,
    Npx,
    Workstyle,   // new
}
```

Workstyle steps are discovered from `.lf/workstyles/<name>/steps/` and prefixed with the workstyle name. Workstyle flows are discovered from `.lf/workstyles/<name>/flows/`.

## Grand vision: workstyles as platform

A workstyle bundles:

| Layer | What it provides | gstack | vsm | lfjack |
|-------|-----------------|--------|-----|--------|
| **Steps** | The actual prompts | 28 SKILL.md files | s2-scan through s5-assess | design, implement, gate |
| **Flows** | Step sequences | sprint, plan-manual, review | govern-*, garden | build, code, pair, deploy |
| **Voice** | Personality and tone | gstack builder voice (prioritized) | — | loopflow default |
| **Directions** | Quality/aesthetic lens | — | care, clarity | craft, ux |
| **Config** | Harness, model, tools | — | — | opus default |
| **Hooks** | Behavioral constraints | careful, freeze, guard | — | — |
| **Task provider** | Where work comes from | — | wave items | wave items, notion, linear |

`lf init --workstyle gstack` would:
1. Clone/sync the prompt source
2. Install flows, steps, and voice
3. Set default config
4. Configure hooks

### Loopflowhub (future)

Open-source registry for sharing workstyles. Anyone publishes their setup — prompts, flows, voice, config — and anyone can install and adapt it. Like a package manager for how you work with coding agents.

```bash
lf workstyle install loopflowhub:gstack     # install from registry
lf workstyle install github:someone/setup   # install from git
lf workstyle publish                         # publish your workstyle
lf workstyle search "security"               # find workstyles
```

The three launch workstyles (lfjack, vsm, gstack) prove the abstraction works before opening it up.

But that's the grand version. The quick win is: import gstack's prompts, create the flows, build the sync tool.

## Done when

1. `lf gstack:office-hours` runs the office-hours prompt (from synced source)
2. `lf gstack-sprint` runs the full sprint flow
3. `lf ops workstyle sync gstack` pulls latest from garrytan/gstack
4. The sync tool strips preamble/telemetry, keeps prompt content
5. Existing loopflow steps (implement, gate) work inside gstack flows

## Implementation

This is wave `gstack`. See `wave/gstack/` for staged roadmap:

1. **01-import-convert** — Clone gstack, write converter, produce `.lf/workstyles/gstack/` with steps and voice
2. **02-flows** — Create gstack flow YAMLs, wire into flow discovery, voice resolution
3. **03-sync** — `lf ops workstyle sync` command, cache management, diff preview

lfjack and vsm workstyle packaging are separate future waves.
