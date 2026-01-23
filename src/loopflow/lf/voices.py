"""Voice file loading for agent judgment and perspective."""

from dataclasses import dataclass
from enum import Enum
from pathlib import Path

from loopflow.lf.goals import _parse_frontmatter

# Path to bundled builtin voice templates
_VOICES_TEMPLATES_DIR = Path(__file__).parent.parent / "templates" / "voices"

# Builtin mode voices (decide what to work on)
_BUILTIN_MODES = {"adaptive", "roadmap", "build", "simplify"}


class VoiceKind(Enum):
    """Whether a voice is a role (how to judge) or mode (what to decide)."""

    ROLE = "role"
    MODE = "mode"


@dataclass
class Voice:
    """A parsed voice file."""

    name: str
    content: str
    area: list[str]  # Default pathset
    pipeline: str  # Default pipeline
    kind: VoiceKind = VoiceKind.ROLE  # Default to role


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


def load_voice(repo: Path, voice_name: str) -> Voice | None:
    """Load and parse a voice file.

    Checks in order:
    1. voices/{name}.md (user-defined)
    2. templates/voices/{name}.md (builtin fallback)

    Returns None if voice file doesn't exist.
    """
    if not voice_name:
        return None

    # Check user-defined voice first
    voice_path = repo / ".lf" / "voices" / f"{voice_name}.md"
    if not voice_path.exists():
        # Fall back to builtin templates
        builtin_path = _get_builtin_voice(voice_name)
        if builtin_path:
            voice_path = builtin_path
        else:
            return None

    text = voice_path.read_text()
    frontmatter, content = _parse_frontmatter(text)

    # Parse area as list
    area = frontmatter.get("area", [])
    if isinstance(area, str):
        area = [a.strip() for a in area.split(",") if a.strip()]

    # Determine kind
    kind = _detect_voice_kind(voice_name, frontmatter, content)

    return Voice(
        name=voice_name,
        content=content,
        area=area,
        pipeline=frontmatter.get("pipeline", "ship"),
        kind=kind,
    )


def load_voice_content(repo: Path, voice_name: str) -> str | None:
    """Load just the voice file content."""
    voice = load_voice(repo, voice_name)
    return voice.content if voice else None


def list_voices(repo: Path) -> list[str]:
    """List available voice names in a repo (including builtins)."""
    voices = set()

    # User-defined voices
    voices_dir = repo / ".lf" / "voices"
    if voices_dir.exists():
        voices.update(p.stem for p in voices_dir.glob("*.md"))

    # Builtin voices
    voices.update(list_builtin_voices())

    return sorted(voices)


def voice_exists(repo: Path, voice_name: str) -> bool:
    """Check if a voice file exists (user-defined or builtin)."""
    if not voice_name:
        return False
    # Check user-defined voice
    voice_path = repo / ".lf" / "voices" / f"{voice_name}.md"
    if voice_path.exists():
        return True
    # Check builtin voice
    return _get_builtin_voice(voice_name) is not None


# Voice kind detection and composition


def _detect_voice_kind(name: str, frontmatter: dict, content: str) -> VoiceKind:
    """Infer kind from frontmatter or content heuristics."""
    # Explicit frontmatter takes precedence
    if "kind" in frontmatter:
        kind_str = frontmatter["kind"].lower()
        if kind_str == "mode":
            return VoiceKind.MODE
        return VoiceKind.ROLE

    # Builtin modes
    if name in _BUILTIN_MODES:
        return VoiceKind.MODE

    # Heuristic: if content talks about deciding what to do, it's a mode
    mode_patterns = [
        "## Decision",
        "decide what mode",
        "deciding what to",
        "roadmap/",
        "status: approved",
        "status: proposed",
    ]
    for pattern in mode_patterns:
        if pattern in content:
            return VoiceKind.MODE

    return VoiceKind.ROLE


def needs_adaptive(voices: list[Voice]) -> bool:
    """True if no mode voice present—adaptive should be injected."""
    return not any(voice.kind == VoiceKind.MODE for voice in voices)


def resolve_voices(repo: Path, voice_names: list[str]) -> list[Voice]:
    """Load and resolve voice names to Voice objects."""
    voices = []
    for name in voice_names:
        voice = load_voice(repo, name)
        if voice:
            voices.append(voice)
    return voices


def build_effective_voices(repo: Path, voice_names: list[str]) -> list[Voice]:
    """Build final voice list, injecting adaptive if needed.

    - If voice_names is empty → [adaptive]
    - If only roles → [adaptive] + roles
    - If any mode present → voices as-is (no adaptive injection)
    """
    voices = resolve_voices(repo, voice_names)

    if needs_adaptive(voices):
        adaptive = load_voice(repo, "adaptive")
        if adaptive:
            voices = [adaptive] + voices

    return voices


def render_voices(voices: list[Voice]) -> str:
    """Combine voices into single prompt. Modes first, then roles."""
    # Sort: modes first, then roles
    modes = [voice for voice in voices if voice.kind == VoiceKind.MODE]
    roles = [voice for voice in voices if voice.kind == VoiceKind.ROLE]
    ordered = modes + roles

    parts = []
    for voice in ordered:
        tag = "mode" if voice.kind == VoiceKind.MODE else "voice"
        parts.append(f"<lf:{tag}:{voice.name}>\n{voice.content}\n</lf:{tag}:{voice.name}>")

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
