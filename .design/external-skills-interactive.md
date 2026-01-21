# External Skills Default to Interactive

External skills (superpowers `sp:`, SkillRegistry `sr:`) should default to interactive mode.

## What to build

When running skills from external sources, default to interactive mode instead of auto mode. Users haven't written these prompts themselves—they benefit from the ability to guide and course-correct.

User quote: "I think special skills should be default interactive? like superpowers and skillsregistry seem likely to be best done interactively"

## Data structures

Add `is_external_skill` to `StepFile` in `frontmatter.py`:

```python
@dataclass
class StepFile:
    name: str
    content: str
    config: StepConfig = field(default_factory=StepConfig)
    is_external_skill: bool = False  # New field
```

## Key functions

**frontmatter.py** — Add parameter to resolution:

```python
def resolve_step_config(
    step_name: str,
    global_config,
    frontmatter: StepConfig,
    cli_interactive: bool | None,
    cli_auto: bool | None,
    cli_model: str | None,
    cli_context: list[str] | None,
    cli_voice: list[str] | None = None,
    is_external_skill: bool = False,  # New parameter
) -> ResolvedStepConfig:
    """Merge configs: CLI > frontmatter > global > skill-source-default > defaults."""
```

Resolution chain for `interactive`:
1. CLI flags (`-i`/`-a`) — highest priority
2. Frontmatter in skill file (`interactive: true/false`)
3. Global config `interactive` list
4. **External skill default** (new) — `True` if `is_external_skill`
5. Step defaults — `False`

**context.py:414-416** — Mark external skills in `gather_step()`:

```python
if skill:
    content = load_skill_prompt(skill)
    step_file = parse_step_file(name, content)
    step_file.is_external_skill = True  # Mark it
    return step_file
```

**run.py:390-399** — Pass flag through:

```python
resolved = resolve_step_config(
    step_name=step,
    global_config=config,
    frontmatter=frontmatter,
    cli_interactive=True if interactive else None,
    cli_auto=True if auto else None,
    cli_model=model,
    cli_context=list(path) if path else None,
    cli_voice=cli_voices or None,
    is_external_skill=step_file.is_external_skill if step_file else False,
)
```

## Constraints

- CLI `-a`/`--auto` must still override to auto mode
- Frontmatter `interactive: false` must still work
- Local steps (`.claude/commands/`, `.lf/`) keep current behavior (default auto)
- Built-in steps keep current behavior (default auto)

## Done when

```bash
# External skill defaults to interactive
lf sp:brainstorm 2>&1 | head -5
# Should show interactive mode launching

# CLI override works
lf sp:brainstorm -a 2>&1 | head -5
# Should show auto mode

# Local step still defaults to auto
lf review 2>&1 | head -5
# Should show auto mode
```

Verify in logs or by observing the session behavior—interactive mode allows interruption, auto mode runs to completion.
