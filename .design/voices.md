# Voices

**What to build:** A system for defining reusable personas (system prompts) that get prepended to tasks.

## User Intent

"Multiple personas - different system prompts/personalities the agent can use."

## Data Structures

```python
@dataclass
class Voice:
    name: str
    content: str  # The persona/system prompt text

def load_voice(name: str, repo_root: Path) -> Voice:
    """Load voice from .lf/voices/{name}.md"""
    ...
```

Voices are markdown files in `.lf/voices/`:

```
.lf/
  voices/
    architect.md
    reviewer.md
    concise.md
```

Example voice file (`.lf/voices/architect.md`):

```markdown
Bring an architect's perspective. Focus on what only an architect would catch:

- How does this fit the larger system?
- What interfaces will other code need?

Don't do more than your share. Leave the details to other passes.
```

## Configuration

### Global default voice

```yaml
# .lf/config.yaml
voice: architect
```

### Per-task voice override

```yaml
# .lf/implement.lf
---
voice: concise
---
Implement the feature...
```

### CLI override

```bash
lf implement --voice architect
lf implement --voice architect,concise   # multiple voices, comma-separated
```

Priority: CLI > frontmatter > config > none

Multiple voices are concatenated in order, separated by blank lines.

## Key Functions

```python
# voices.py
def load_voice(name: str, repo_root: Path) -> Voice:
    """Load voice from .lf/voices/{name}.md. Raise VoiceNotFoundError if not found."""

def parse_voice_arg(voice_arg: str | None) -> list[str]:
    """Parse 'a,b,c' into ['a', 'b', 'c']. Returns [] if None or empty."""

class VoiceNotFoundError(Exception):
    """Raised when a voice file doesn't exist."""
```

```python
# frontmatter.py - add to TaskConfig
@dataclass
class TaskConfig:
    interactive: bool | None = None
    include: list[str] | None = None
    exclude: list[str] | None = None
    model: str | None = None
    voice: list[str] | None = None  # NEW - list of voice names
```

```python
# config.py - add to Config
class Config(BaseModel):
    agent_model: str | None = None
    voice: list[str] | None = None  # NEW - global default voices
    # ... existing fields
```

Frontmatter supports both string and list:

```yaml
# Single voice
---
voice: architect
---

# Multiple voices
---
voice: [architect, concise]
---
```

```python
# context.py - modify gather_prompt_components
def gather_prompt_components(..., voices: list[str] | None = None) -> PromptComponents:
    """Add voices parameter, load and store combined voice content."""
```

```python
# context.py - modify format_prompt
def format_prompt(components: PromptComponents) -> str:
    """Prepend voice content before task if present."""
```

## Prompt Assembly

Voice content is prepended to the task section:

```
The task.

<lf:voices>
<lf:voice:architect>
Bring an architect's perspective. Focus on what only an architect would catch...
</lf:voice:architect>

<lf:voice:concise>
Be concise. One sentence where possible.
</lf:voice:concise>
</lf:voices>

<lf:task:implement>
Implement the feature...
</lf:task:implement>
```

Single voice omits the outer `<lf:voices>` wrapper. No voices = no voice section.

## Constraints

- **Voice files are plain markdown.** No frontmatter, no templates, no special syntax. Just the persona text.
- **Voices don't affect context gathering.** They're pure prompt injection, not configuration.
- **Empty voice = no voice.** If `voice:` is set to empty string or null, skip voice injection.
- **Voice not found = error.** Don't silently ignore missing voices.

## Files to Change

1. **New:** `src/loopflow/voices.py` - Voice loading
2. **Edit:** `src/loopflow/config.py` - Add `voice` field to Config
3. **Edit:** `src/loopflow/frontmatter.py` - Add `voice` field to TaskConfig, update resolution
4. **Edit:** `src/loopflow/context.py` - Add voice to PromptComponents, update format_prompt
5. **Edit:** `src/loopflow/cli/run.py` - Add `--voice` CLI option

## Done When

```bash
# Create voices
mkdir -p .lf/voices
echo "Be concise. One sentence where possible." > .lf/voices/concise.md
echo "Bring an architect's perspective. Focus on system-level concerns." > .lf/voices/architect.md

# Single voice
lf : "What is Python?" --voice concise -c
# Output shows <lf:voice:concise>...</lf:voice:concise>

# Multiple voices
lf : "What is Python?" --voice architect,concise -c
# Output shows <lf:voices> wrapper with both voices inside
```

The `-c` (copy) flag shows the assembled prompt. Verify voices appear before the task.
