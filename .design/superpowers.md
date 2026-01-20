# Superpowers Integration

Import external skill libraries (starting with [obra/superpowers](https://github.com/obra/superpowers)) and run them through loopflow's universal launcher.

## Philosophy

Loopflow implements best practices for working with AI agents:

- **Write prompts down.** In your codebase, versioned, reviewed.
- **Assemble context explicitly.** Know what the agent sees.
- **Chain tasks with quality gates.** Commits between steps, not vibes.

There's no magic in the prompts themselves. The value is the discipline—having them written, iterating on them, sharing them across your team.

You shouldn't have to write everything from scratch. Good skills exist. Superpowers has them. Other libraries will emerge. Loopflow lets you use any skill, with any agent, while keeping your own prompts alongside.

## What to build

`lf sp:<skill>` runs skills from superpowers with loopflow's context assembly and model selection.

## User's words

> "lf lets you use all the skills you can"

> "if you want to use some lf stuff and some brainstorm stuff, great, lets help you do that"

## Data structures

```python
@dataclass
class SkillSource:
    """External skill library."""
    name: str           # "superpowers"
    prefix: str         # "sp"
    path: Path          # ~/.superpowers or local clone
    skills: list[str]   # discovered skill names

@dataclass
class Skill:
    """A skill from an external source."""
    name: str           # "brainstorm"
    source: str         # "superpowers"
    prompt_path: Path   # path to SKILL.md or equivalent
```

## Key functions

```python
def discover_skill_sources(
    config_sources: list[SkillSourceConfig] | None,
    repo_root: Path | None,
) -> list[SkillSource]:
    """Find configured skill libraries."""

def find_skill(name: str, sources: list[SkillSource]) -> Skill | None:
    """Resolve 'sp:brainstorm' to a Skill."""

def load_skill_prompt(skill: Skill) -> str:
    """Extract prompt content from skill definition."""

def list_all_skills(sources: list[SkillSource]) -> list[tuple[str, str]]:
    """Return all skills as (prefixed_name, source_name) tuples."""
```

## Config

```yaml
# .lf/config.yaml
skill_sources:
  - name: superpowers
    prefix: sp
    path: ~/.superpowers  # or: git: obra/superpowers
```

Auto-detection fallback: if `~/.superpowers` or `./superpowers` exists, register it with prefix `sp`.

## Invocation

```bash
lf sp:brainstorm                 # run superpowers brainstorm skill
lf sp:write-plan -m codex        # with different model
lf sp:execute-plan -i            # interactive mode
lf sp:brainstorm: auth feature   # with args
```

Skills get loopflow's full context assembly (docs, diff, files, clipboard).

## Skill discovery

Superpowers structure:
```
superpowers/
  skills/
    brainstorming/
      SKILL.md           # main prompt
    writing-plans/
      SKILL.md
    ...
```

Discovery:
1. Scan `skills/` directory
2. Each subdirectory with `SKILL.md` is a skill
3. Normalize names:
   - Remove trailing `-ing` suffix: `brainstorming` → `brainstorm`
   - Remove trailing `-s` suffix: `writing-plans` → `writing-plan`
   - Replace underscores with hyphens
   - Special case: `test-driven-development` → `tdd`

## Prompt extraction

`SKILL.md` files contain the full skill definition. Load as the task prompt, same as `.claude/commands/*.md`.

If the skill references other files (e.g., `REFERENCES.md`), include them as additional context.

## UI changes

**Maestro:** Task selector should show external skills:
- Group by source: "Loopflow" section, "Superpowers" section
- Badge showing source prefix
- Same run flow as native tasks

## Documentation updates

All public-facing docs need updates to reflect skill sources.

**README.md** (done):
- "Works With" section listing ecosystem tools (worktrunk, superpowers)

**docs/index.md** (done):
- "Works With" section mirroring README

**docs/vision.md** (done):
- "Best Practices, Not Magic" section explaining the philosophy

**docs/config.md:**
- Add `skill_sources` configuration option
- Document auto-detection behavior
- Example config with superpowers

**docs/lf.md:**
- Document `lf <prefix>:<skill>` syntax
- Add `--list` flag showing external skills
- Examples with `lf sp:brainstorm`

**docs/builtins.md:**
- Note that external skills appear alongside built-ins
- Explain prefix namespacing

**docs/getting-started.md:**
- Mention ecosystem tools in "Next Steps" or similar
- Optional: quick example of using external skill

**docs/patterns.md:**
- Add pattern for mixing loopflow tasks with external skills
- Example workflow: `lf sp:brainstorm` → `lf implement` → `lf sp:execute-plan`

## Constraints

- **Don't reimplement superpowers.** We're a launcher, not a fork. Their skills, our context.
- **Prefix required for external skills.** `lf brainstorm` is ambiguous if user has both. `lf sp:brainstorm` is explicit.
- **No auto-triggering.** Superpowers skills auto-activate in Claude Code. In loopflow, invocation is explicit.
- **Skill files are read-only.** We don't modify external skill libraries.

## Done when

**Functional:**
```bash
# Configure superpowers
cat >> .lf/config.yaml << 'EOF'
skill_sources:
  - name: superpowers
    prefix: sp
    path: ~/.superpowers
EOF

# List available skills
lf --list
# Shows: design, implement, review, sp:brainstorm, sp:write-plan, sp:execute-plan

# Run a superpowers skill
lf sp:brainstorm: add user auth
# Assembles loopflow context + superpowers prompt, runs with configured model

# Run with different model
lf sp:write-plan -m codex
# Same skill, Codex backend
```

**Documentation:**
- [x] docs/config.md has `skill_sources` option documented
- [x] docs/lf.md has `<prefix>:<skill>` syntax documented
- [x] docs/builtins.md mentions external skills
- [x] docs/patterns.md has mixed workflow example

**UI:**
- [x] Maestro task selector shows external skills grouped by source

## Future

- `lf skill add <git-url>` to install skill libraries
- Marketplace discovery (list popular skill sources)
- Skill aliasing: `lf alias brainstorm sp:brainstorm`
