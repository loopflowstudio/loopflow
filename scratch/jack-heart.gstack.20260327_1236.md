# Stage 1: Import and convert gstack

## Problem

Loopflow needs to run prompts from external creators. gstack is the first — 29 SKILL.md files with a sprint methodology baked in. Today there's no way to import external prompt sets, no concept of a workstyle directory, and no prefix-based step discovery for local subdirectories.

The converter must strip gstack's infrastructure (telemetry, session tracking, update checks, onboarding flows) while preserving the methodology. gstack's voice should not be tied to the workstyle runtime — it should become a reusable direction. gstack publishes generated SKILL.md files (resolved from `.tmpl` templates) — we convert those, not the templates.

Advancing wave goals: "Run any gstack prompt as a loopflow step: `lf gstack:office-hours`" and "Use gstack's voice anywhere via a direction."

## Approach

Three pieces, delivered together.

### 1. Python converter (`python/loopflow/workstyle/convert.py`)

A standalone script that reads a cloned gstack repo and writes loopflow-native output to `.lf/workstyles/gstack/`.

**Parsing strategy**: gstack SKILL.md files follow a consistent pattern:
1. YAML frontmatter (`---` delimited)
2. Auto-generated comment (`<!-- AUTO-GENERATED ... -->`)
3. `## Preamble (run first)` — a bash block + prose (telemetry, session tracking, upgrade checks, onboarding prompts)
4. Voice section — either short (tier 1) or full Garry Tan persona (tier 2+)
5. Skill-specific instructions (the actual methodology)

The converter:
- Parses YAML frontmatter with `pyyaml` — extracts `name`, `version`, `description`, `allowed-tools`, `benefits-from`, `preamble-tier`
- Strips everything between `## Preamble` and the skill's main `# /skill-name:` heading
- Extracts the full voice section (tier 2+) once from the first skill that has it, converts it into a built-in direction prompt
- Keeps everything after the skill heading as step content
- Writes loopflow step frontmatter: maps `allowed-tools` → `tools`, `benefits-from` → `after` (ordering hint), `description` from frontmatter
- Preserves imported metadata even where loopflow doesn't use it yet; stage 1 treats these fields as carried-through data, not committed runtime semantics

**What gets stripped (resolved content, not template vars)**:
- The preamble bash block (update checks, session management, telemetry, repo mode detection)
- Onboarding prose (Boil the Lake intro, telemetry prompt, proactive behavior prompt)
- Upgrade flow instructions
- Contributor mode handling
- Completion status protocol + telemetry footer
- Repo ownership / search-before-building sections (loopflow has its own)
- `## Voice` section from each skill (extracted into a shared direction, not kept in each step)
- Base branch detection bash blocks (`{{BASE_BRANCH_DETECT}}` resolved content)
- Test bootstrap/setup bash blocks (loopflow owns test infrastructure)

**What gets kept**:
- The skill's actual methodology (everything after the `# /skill-name:` heading, minus voice)
- Browser setup content — kept but marked with `requires: [browser]` in frontmatter so runtime can warn
- Design methodology, QA methodology, review methodology sections
- Any resolved `{{BENEFITS_FROM}}` content (maps to step ordering)

**Template placeholders**: Not relevant. We convert the generated SKILL.md files where all `{{PLACEHOLDER}}` tokens are already resolved. The converter never sees template syntax.

**Browser-dependent skills** (browse, qa, qa-only, benchmark, canary, connect-chrome, setup-browser-cookies): Convert normally but add `requires: [browser]` to frontmatter.

### 2. Workstyle directory structure

```
.lf/workstyles/gstack/
  workstyle.yaml
  steps/
    office-hours.md
    autoplan.md
    ceo-review.md
    design-review.md
    eng-review.md
    review.md
    cso.md
    investigate.md
    qa.md
    qa-only.md
    ship.md
    retro.md
    codex.md
    design-consultation.md
    document-release.md
    land-and-deploy.md
    browse.md
    benchmark.md
    canary.md
    careful.md
    freeze.md
    unfreeze.md
    guard.md
    gstack-upgrade.md
    connect-chrome.md
    setup-browser-cookies.md
    setup-deploy.md
    gstack.md
```

`workstyle.yaml`:
```yaml
name: gstack
description: "Garry Tan's sprint factory"
source:
  repo: garrytan/gstack
  ref: main
  last_commit: <sha>
  last_sync: <iso8601>
prefix: gstack
```

### 3. Built-in directions for imported voice/soul

Treat imported "voice" documents as directions rather than prompt-wrapper state.

- Add a built-in direction `gstack` from the extracted gstack voice
- Add a built-in direction `openclaw` from OpenClaw's `SOUL.md`
- These directions should be usable anywhere: `lf implement -d gstack`, `lf review -d openclaw`
- Workstyle flows can opt into them explicitly with `direction: [gstack]` rather than relying on hidden runtime voice switching

This makes imported style composable instead of ambient. A gstack step can run with or without the gstack direction. A loopflow-native step can also borrow that direction when useful.

### 4. Rust discovery wiring (`discovery.rs`)

Add `SkillSourceKind::Workstyle`. Scan `.lf/workstyles/*/` at discovery time, same priority band as superpowers and npx.

In `discover_skill_sources`, after config sources and before npx:
- Read `.lf/workstyles/` directory
- For each subdirectory, list `.md` files in `steps/`
- Create a `SkillSource` with prefix = directory name, kind = Workstyle

`find_skill` already handles `prefix:name` → look up source by prefix → load from source path. Adding the workstyle source makes `gstack:office-hours` resolve to `.lf/workstyles/gstack/steps/office-hours.md` with zero changes to `find_skill`.

**`lf --list`**: `list_all_steps` already iterates skill sources. Workstyle sources appear automatically.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Native locations (`.lf/steps/gstack/`, builtin direction for gstack voice) | Simpler | Steps become harder to sync/remove as a bundle. Keep workstyle steps bundled; extract voice separately as a reusable direction. |
| Convert `.tmpl` files + resolve placeholders ourselves | Access to "source" format | 31 placeholder resolvers, many pulling from bash scripts. The generated SKILL.md is the stable interface. |
| Rust converter instead of Python | Single language | Python is better for text parsing/YAML. The converter is a build tool, not runtime. |
| Workstyle-specific voice injection | Makes gstack flows feel automatic | Couples style to source lookup and makes voice unusable elsewhere. Direction fits the model better. |
| Import as `.agents/skills/` (npx-style) | Zero discovery changes | Wrong abstraction. Npx skills are single-file, no manifest, no sync. |

## Key decisions

**Workstyle directory over native locations.** The workstyle directory *is* a native format — it's where loopflow will look for workstyles. Discovery finds them via `SkillSourceKind::Workstyle`. The directory keeps the bundle atomic for sync (stage 3).

**Convert generated SKILL.md, not templates.** The generated files are the stable contract. Template placeholders and resolvers are gstack internals that change frequently.

**Strip by section heading, not content matching.** The preamble follows a consistent structure: `## Preamble` through the next `# /skill-name:` heading. Heading boundaries are robust to content changes.

**Voice is a direction, not a hidden prompt layer.** gstack's extracted voice becomes a built-in direction (`gstack`). OpenClaw's `SOUL.md` becomes another built-in direction (`openclaw`). Flows and users opt into these directions explicitly.

**Preserve metadata now; interpret later.** Keep fields like `tools`, `after`, and browser requirements in the converted output, but don't contort stage 1 runtime around them. The exact shape can evolve once real workstyles put pressure on it.

**29 skills, not 28.** The wave item says 28, but gstack has 29 (including the root `gstack` meta-skill). Convert all 29.

**Rename `plan-*` → drop `plan-` prefix.** `plan-ceo-review` becomes `gstack:ceo-review`. The `plan-` prefix is gstack-internal organization that adds noise under a prefix namespace.

## Scope

In scope:
- Python converter: parse SKILL.md, strip infrastructure, extract voice, write steps
- `SkillSourceKind::Workstyle` in discovery.rs
- Built-in direction prompts for gstack voice and OpenClaw soul
- Tests for converter (Python) and discovery (Rust)
- Running the converter to produce the initial `.lf/workstyles/gstack/`

Out of scope (per wave README "Not here"):
- Browser daemon integration
- Cross-agent compatibility (codex/gemini)
- Sync CLI commands (stage 3)
- Flow definitions (stage 2)
- loopflowhub registry

## Done when

1. `lf gstack:office-hours` runs and shows the office-hours prompt content
2. `lf --list` shows gstack steps under a "gstack" source
3. All 29 SKILL.md files convert without errors
4. `lf implement -d gstack` and `lf implement -d openclaw` both resolve built-in directions
5. `cargo test` and `uv run pytest` pass
6. Converted steps contain methodology content, zero telemetry/session/onboarding machinery
