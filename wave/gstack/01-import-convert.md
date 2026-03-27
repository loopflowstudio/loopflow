# Stage 1: Import and convert

Clone garrytan/gstack, write a converter that transforms SKILL.md files into loopflow steps, and produce `.lf/workstyles/gstack/` with converted steps. Extract gstack's voice into a reusable direction instead of a workstyle-local voice file.

## What to build

**Python converter** (`python/loopflow/workstyle/convert.py`):
- Parse gstack SKILL.md format (YAML frontmatter + preamble bash block + voice + instructions)
- Strip preamble bash blocks (telemetry, update checks, session tracking, onboarding)
- Strip setup sections (env verification, data download, onboarding flows) — loopflow owns setup
- Extract the voice section once and turn it into the `gstack` direction
- Add an `openclaw` direction from OpenClaw's `SOUL.md`
- Keep skill-specific instructions as step content
- Map gstack frontmatter (`name`, `version`, `benefits-from`, `allowed-tools`) to loopflow step frontmatter
- Write converted steps to `.lf/workstyles/gstack/steps/<name>.md`

**General policy: strip setup from imported workstyles.** Branch creation, environment checks, tool verification, data preparation — these are loopflow's responsibility, standardized across all workstyles. Workstyles bring methodology, not scaffolding.

**Workstyle directory structure**:
```
.lf/workstyles/gstack/
  workstyle.yaml          # metadata: source repo, last sync, prefix
  steps/
    office-hours.md
    ceo-review.md
    design-review.md
    eng-review.md
    autoplan.md
    review.md
    investigate.md
    cso.md
    qa.md
    ship.md
    retro.md
    ...
```

**Discovery wiring** (`discovery.rs`):
- Add `SkillSourceKind::Workstyle`
- Discover workstyles from `.lf/workstyles/*/`
- Steps available as `gstack:office-hours`, `gstack:review`, etc.

**Direction wiring**:
- Add built-in direction `gstack`
- Add built-in direction `openclaw`
- Both are usable anywhere with `-d`

## Data structures

```python
@dataclass
class GstackSkill:
    name: str
    version: str
    description: str
    allowed_tools: list[str]
    benefits_from: list[str]
    preamble: str           # stripped during conversion
    voice: str              # extracted and reshaped into the gstack direction
    instructions: str       # becomes the step content

@dataclass
class WorkstyleManifest:
    name: str
    source_repo: str
    source_ref: str
    last_sync: str
    last_commit: str
    step_prefix: str
    steps: list[str]
```

```rust
// In discovery.rs
pub enum SkillSourceKind {
    Directory,
    SingleFile,
    Npx,
    Workstyle,  // new
}
```

## Done when

1. `lf gstack:office-hours` runs and shows the office-hours prompt content
2. `lf --list` shows gstack steps under a "gstack" source
3. All 29 SKILL.md files convert without errors
4. `lf implement -d gstack` and `lf implement -d openclaw` both resolve
5. `cargo test` and `uv run pytest` pass
