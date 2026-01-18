"""Design artifact helpers."""

import shutil
from pathlib import Path


def gather_design_docs(repo_root: Path) -> list[tuple[Path, str]]:
    """Gather design docs from .design/ for prompt context."""
    design_dir = repo_root / ".design"
    if not design_dir.is_dir():
        return []

    docs = []
    for path in sorted(design_dir.rglob("*.md")):
        if path.is_file():
            docs.append((path, path.read_text()))
    return docs


def has_design_artifacts(repo_root: Path) -> bool:
    """Return True when .design contains any files or folders."""
    design_dir = repo_root / ".design"
    if not design_dir.exists():
        return False
    return any(design_dir.iterdir())


def clear_design_artifacts(repo_root: Path) -> bool:
    """Remove .design contents while keeping the folder."""
    design_dir = repo_root / ".design"
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
