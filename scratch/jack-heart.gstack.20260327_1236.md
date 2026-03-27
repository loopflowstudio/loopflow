# Stage 1: Import and convert gstack

Clone garrytan/gstack, write a converter that transforms SKILL.md files into loopflow steps, and wire them into discovery.

## What to build

**Python converter** (`python/loopflow/workstyle/convert.py`):
- Parse gstack SKILL.md format (YAML frontmatter + preamble bash block + voice + instructions)
- Strip preamble bash blocks (telemetry, update checks, session tracking, onboarding)
- Strip setup sections (env verification, data download, onboarding flows) — loopflow owns setup
- Keep voice section — extract once into `.lf/voice/gstack.md`
- Keep skill-specific instructions as step content
- Map gstack frontmatter (`name`, `version`, `benefits-from`, `allowed-tools`) to loopflow step frontmatter
- Write converted steps to `.lf/steps/gstack/<name>.md`

**Output structure** (native loopflow locations):
```
.lf/steps/gstack/
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
.lf/voice/
  gstack.md               # gstack voice (extracted from shared section)
```

**Discovery wiring** (`discovery.rs`):
- Discover steps from `.lf/steps/<prefix>/` subdirectories
- Steps available as `gstack:office-hours`, `gstack:review`, etc.
- Voice from `.lf/voice/gstack.md` injected when running gstack-prefixed steps

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
    voice: str              # extracted to voice file
    instructions: str       # becomes the step content
```

```rust
// discovery.rs — steps in subdirectories are prefixed
// .lf/steps/gstack/office-hours.md → gstack:office-hours
```

## Constraints

- The converter must handle gstack's template placeholders (`{{PREAMBLE}}`, `{{BROWSE_SETUP}}`, etc.) — either resolve them or strip them cleanly
- SKILL.md files that depend on the browser daemon (qa, browse, benchmark, canary) should convert but warn at runtime that browser features aren't available
- `benefits-from` in gstack frontmatter declares step dependencies — map to loopflow step ordering hints

## Done when

1. `lf gstack:office-hours` runs and shows the office-hours prompt content
2. `lf --list` shows gstack steps under a "gstack" source
3. All 28 SKILL.md files convert without errors
4. Voice is extracted to `.lf/voice/gstack.md`
5. `cargo test` and `uv run pytest` pass
