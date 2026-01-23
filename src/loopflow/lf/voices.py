"""Voice file loading for agent judgment and perspective."""

import re
from dataclasses import dataclass
from pathlib import Path

_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)

# Path to bundled builtin voice templates
_VOICES_TEMPLATES_DIR = Path(__file__).parent.parent / "templates" / "voices"


def _parse_frontmatter(text: str) -> tuple[dict, str]:
    """Parse YAML frontmatter from markdown text.

    Returns (frontmatter_dict, body_content).
    """
    match = _FRONTMATTER_PATTERN.match(text)
    if not match:
        return {}, text

    frontmatter_text = match.group(1)
    body = text[match.end() :].strip()

    # Simple YAML parsing (no external dependency)
    result: dict = {}
    current_key = None

    for line in frontmatter_text.split("\n"):
        line = line.rstrip()
        if not line or line.startswith("#"):
            continue

        # List item continuation
        if line.startswith("  - ") and current_key:
            if current_key not in result:
                result[current_key] = []
            result[current_key].append(line[4:].strip())
            continue

        if ":" in line:
            key, _, value = line.partition(":")
            key = key.strip()
            value = value.strip()
            current_key = key

            if not value:
                continue

            # Inline list: [a, b, c]
            if value.startswith("[") and value.endswith("]"):
                items = value[1:-1].split(",")
                result[key] = [item.strip() for item in items if item.strip()]
            elif value.lower() in ("true", "yes"):
                result[key] = True
            elif value.lower() in ("false", "no"):
                result[key] = False
            elif value.isdigit():
                result[key] = int(value)
            else:
                result[key] = value

    return result, body


@dataclass
class Voice:
    """A parsed voice file.

    Voices define *how* to approach work (judgment, style, perspective).
    They don't specify *where* (area) or *what* (flow) - those are separate dimensions.
    """

    name: str
    content: str


class VoiceNotFoundError(Exception):
    """Raised when a voice file doesn't exist."""

    pass


def _get_builtin_voice(name: str) -> Path | None:
    """Return path to bundled voice template if it exists."""
    builtin = _VOICES_TEMPLATES_DIR / f"{name}.md"
    return builtin if builtin.exists() else None


def list_builtin_voices() -> list[str]:
    """Return names of all builtin voices."""
    if not _VOICES_TEMPLATES_DIR.exists():
        return []
    return sorted(p.stem for p in _VOICES_TEMPLATES_DIR.glob("*.md"))


def load_voice(repo: Path | None, voice_name: str) -> Voice | None:
    """Load and parse a voice file.

    Checks in order:
    1. .lf/voices/{name}.md (repo)
    2. ~/.lf/voices/{name}.md (global)
    3. templates/voices/{name}.md (builtin)

    Returns None if voice file doesn't exist.
    """
    if not voice_name:
        return None

    # Check repo voice first
    voice_path = None
    if repo:
        repo_voice = repo / ".lf" / "voices" / f"{voice_name}.md"
        if repo_voice.exists():
            voice_path = repo_voice

    # Check global voice
    if not voice_path:
        global_voice = Path.home() / ".lf" / "voices" / f"{voice_name}.md"
        if global_voice.exists():
            voice_path = global_voice

    # Fall back to builtin templates
    if not voice_path:
        builtin_path = _get_builtin_voice(voice_name)
        if builtin_path:
            voice_path = builtin_path
        else:
            return None

    text = voice_path.read_text()
    _frontmatter, content = _parse_frontmatter(text)

    return Voice(name=voice_name, content=content)


def load_voice_content(repo: Path, voice_name: str) -> str | None:
    """Load just the voice file content."""
    voice = load_voice(repo, voice_name)
    return voice.content if voice else None


def list_voices(repo: Path | None) -> list[str]:
    """List available voice names (repo, global, and builtin)."""
    voices = set()

    # Repo voices
    if repo:
        repo_voices_dir = repo / ".lf" / "voices"
        if repo_voices_dir.exists():
            voices.update(p.stem for p in repo_voices_dir.glob("*.md"))

    # Global voices
    global_voices_dir = Path.home() / ".lf" / "voices"
    if global_voices_dir.exists():
        voices.update(p.stem for p in global_voices_dir.glob("*.md"))

    # Builtin voices
    voices.update(list_builtin_voices())

    return sorted(voices)


def voice_exists(repo: Path | None, voice_name: str) -> bool:
    """Check if a voice file exists (repo, global, or builtin)."""
    if not voice_name:
        return False
    # Check repo voice
    if repo:
        repo_voice = repo / ".lf" / "voices" / f"{voice_name}.md"
        if repo_voice.exists():
            return True
    # Check global voice
    global_voice = Path.home() / ".lf" / "voices" / f"{voice_name}.md"
    if global_voice.exists():
        return True
    # Check builtin voice
    return _get_builtin_voice(voice_name) is not None


def resolve_voices(repo: Path, voice_names: list[str]) -> list[Voice]:
    """Load and resolve voice names to Voice objects."""
    voices = []
    for name in voice_names:
        voice = load_voice(repo, name)
        if voice:
            voices.append(voice)
    return voices


def render_voices(voices: list[Voice]) -> str:
    """Combine voices into single prompt."""
    parts = [
        f"<lf:voice:{voice.name}>\n{voice.content}\n</lf:voice:{voice.name}>" for voice in voices
    ]
    return "\n\n".join(parts)


def parse_voice_arg(voice_arg: str | None) -> list[str]:
    """Parse 'a,b,c' into ['a', 'b', 'c']. Returns [] if None or empty."""
    if not voice_arg:
        return []
    return [v.strip() for v in voice_arg.split(",") if v.strip()]


def format_voice_section(voice_names: list[str] | None, repo_root: Path) -> str | None:
    """Load voices and format as XML section for prompt."""
    if not voice_names:
        return None

    loaded = []
    for name in voice_names:
        voice = load_voice(repo_root, name)
        if voice:
            loaded.append(voice)

    if not loaded:
        return None

    parts = [
        f"<lf:voice:{voice.name}>\n{voice.content}\n</lf:voice:{voice.name}>" for voice in loaded
    ]

    if len(parts) == 1:
        return parts[0]
    return "<lf:voices>\n" + "\n\n".join(parts) + "\n</lf:voices>"
