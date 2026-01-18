"""Voice loading for prompt personas."""

from dataclasses import dataclass
from pathlib import Path


@dataclass
class Voice:
    name: str
    content: str


class VoiceNotFoundError(Exception):
    """Raised when a voice file doesn't exist."""
    pass


def load_voice(name: str, repo_root: Path) -> Voice:
    """Load voice from .lf/voices/{name}.md. Raise if not found."""
    voice_path = repo_root / ".lf" / "voices" / f"{name}.md"
    if not voice_path.exists():
        voices_dir = repo_root / ".lf" / "voices"
        available = sorted(p.stem for p in voices_dir.glob("*.md")) if voices_dir.exists() else []
        if available:
            raise VoiceNotFoundError(
                f"Voice '{name}' not found. Available: {', '.join(available)}"
            )
        raise VoiceNotFoundError(
            f"Voice '{name}' not found. Create it at: {voice_path}"
        )
    return Voice(name=name, content=voice_path.read_text().strip())


def parse_voice_arg(voice_arg: str | None) -> list[str]:
    """Parse 'a,b,c' into ['a', 'b', 'c']. Returns [] if None or empty."""
    if not voice_arg:
        return []
    return [v.strip() for v in voice_arg.split(",") if v.strip()]
