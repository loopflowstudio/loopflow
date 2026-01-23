"""Design artifact helpers."""

import shutil
from pathlib import Path

from loopflow.lf.voices import load_voice_content


def gather_design_docs(repo_root: Path) -> list[tuple[Path, str]]:
    """Gather design docs from scratch/ for prompt context."""
    design_dir = repo_root / "scratch"
    if not design_dir.is_dir():
        return []

    docs = []
    for path in sorted(design_dir.rglob("*.md")):
        if path.is_file():
            docs.append((path, path.read_text()))
    return docs


def gather_internal_docs(repo_root: Path) -> list[tuple[Path, str]]:
    """Gather internal docs from roadmap/ for prompt context.

    roadmap/ contains forward-looking internal documentation:
    architecture, decisions, context for agents. Unlike scratch/
    (ephemeral per-PR), roadmap/ persists across merges.
    """
    docs_dir = repo_root / "roadmap"
    if not docs_dir.is_dir():
        return []

    docs = []
    for path in sorted(docs_dir.rglob("*.md")):
        if path.is_file():
            docs.append((path, path.read_text()))
    return docs


def _area_parent_paths(area: str) -> list[str]:
    """Return all parent paths for an area.

    For area="a/b/c", returns ["a", "a/b", "a/b/c"].
    """
    parts = area.strip("/").split("/")
    paths = []
    for i in range(len(parts)):
        paths.append("/".join(parts[: i + 1]))
    return paths


def gather_area_docs(repo_root: Path, area: str) -> list[tuple[Path, str]]:
    """Gather docs from area and all parent areas.

    For area="a/b/c", includes:
    - a/*.md and a/roadmap/**/*.md
    - a/b/*.md and a/b/roadmap/**/*.md
    - a/b/c/*.md and a/b/c/roadmap/**/*.md
    """
    docs = []
    seen = set()

    for parent in _area_parent_paths(area):
        parent_dir = repo_root / parent

        # Direct .md files in the area directory
        if parent_dir.is_dir():
            for path in sorted(parent_dir.glob("*.md")):
                if path.is_file() and path not in seen:
                    seen.add(path)
                    docs.append((path, path.read_text()))

        # Area-specific roadmap
        roadmap_dir = parent_dir / "roadmap"
        if roadmap_dir.is_dir():
            for path in sorted(roadmap_dir.rglob("*.md")):
                if path.is_file() and path not in seen:
                    seen.add(path)
                    docs.append((path, path.read_text()))

    return docs


def load_voice(voice: str | Path, repo_root: Path) -> str | None:
    """Load voice content from .lf/voices/{name}.md or a direct path."""
    voice_str = str(voice)

    # If it's just a name (no path separator), use voice loading
    if "/" not in voice_str and "\\" not in voice_str:
        return load_voice_content(repo_root, voice_str)

    # It's a path, resolve relative to repo root
    voice_path = repo_root / voice_str
    if voice_path.exists() and voice_path.is_file():
        return voice_path.read_text()
    return None


def has_design_artifacts(repo_root: Path) -> bool:
    """Return True when scratch/ contains any files or folders."""
    design_dir = repo_root / "scratch"
    if not design_dir.exists():
        return False
    return any(design_dir.iterdir())


def clear_design_artifacts(repo_root: Path) -> bool:
    """Remove scratch/ contents while keeping the folder."""
    design_dir = repo_root / "scratch"
    if design_dir.exists() and not design_dir.is_dir():
        design_dir.unlink()
        design_dir.mkdir(exist_ok=True)
        return True

    if not design_dir.exists():
        return False

    removed = False
    for path in list(design_dir.iterdir()):
        removed = True
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()

    design_dir.mkdir(exist_ok=True)
    return removed
