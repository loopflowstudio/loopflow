"""Design artifact helpers."""

import shutil
from pathlib import Path

from loopflow.lf.directions import load_direction_content


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
    """Gather internal docs from reports/ for prompt context.

    reports/ contains reference material: research, analysis, decisions.
    Unlike scratch/ (ephemeral per-PR), reports/ persists across merges.
    """
    docs_dir = repo_root / "reports"
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


def gather_area(repo_root: Path, area: str) -> list[tuple[Path, str]]:
    """Gather all content from an area (scope of responsibility).

    For area="src/api", includes:
    - All files in src/api/ (code, docs, everything)
    - reports/src/api/* (reports for this area)

    This is what the agent is responsible for and needs to know.
    """
    files = []
    seen = set()
    area_dir = repo_root / area

    # All files in the area directory
    if area_dir.is_dir():
        for path in sorted(area_dir.rglob("*")):
            if path.is_file() and path not in seen:
                # Skip binary files, only include text
                try:
                    content = path.read_text()
                    seen.add(path)
                    files.append((path, content))
                except (UnicodeDecodeError, PermissionError):
                    pass  # Skip binary or unreadable files

    # Area-specific reports (reports/<area>/)
    reports_dir = repo_root / "reports" / area
    if reports_dir.is_dir():
        for path in sorted(reports_dir.rglob("*")):
            if path.is_file() and path not in seen:
                try:
                    content = path.read_text()
                    seen.add(path)
                    files.append((path, content))
                except (UnicodeDecodeError, PermissionError):
                    pass  # Skip binary or unreadable files

    return files


def gather_ancestral_docs(repo_root: Path, area: str) -> list[tuple[Path, str]]:
    """Gather docs and reports from parent paths of an area.

    For area="a/b/c", includes:
    - a/*.md, a/b/*.md (parent docs)
    - reports/a/*, reports/a/b/* (parent reports)
    - NOT a/b/c/* or reports/a/b/c/* (that's in the area itself)
    """
    docs = []
    seen = set()
    parents = _area_parent_paths(area)[:-1]  # Exclude the area itself

    # Paths to exclude (the area itself and deeper)
    area_reports = repo_root / "reports" / area.strip("/")

    for parent in parents:
        # Parent directory docs
        parent_dir = repo_root / parent
        if parent_dir.is_dir():
            for path in sorted(parent_dir.glob("*.md")):
                if path.is_file() and path not in seen:
                    seen.add(path)
                    docs.append((path, path.read_text()))

        # Parent reports (all files, not just .md)
        reports_dir = repo_root / "reports" / parent
        if reports_dir.is_dir():
            for path in sorted(reports_dir.rglob("*")):
                # Skip files under the area's reports (handled by gather_area)
                if _is_under(path, area_reports):
                    continue
                if path.is_file() and path not in seen:
                    try:
                        content = path.read_text()
                        seen.add(path)
                        docs.append((path, content))
                    except (UnicodeDecodeError, PermissionError):
                        pass  # Skip binary or unreadable files

    return docs


def _is_under(path: Path, parent: Path) -> bool:
    """Check if path is under parent directory."""
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def load_direction(direction: str | Path, repo_root: Path) -> str | None:
    """Load direction content from .lf/directions/{name}.md or a direct path."""
    direction_str = str(direction)

    # If it's just a name (no path separator), use direction loading
    if "/" not in direction_str and "\\" not in direction_str:
        return load_direction_content(repo_root, direction_str)

    # It's a path, resolve relative to repo root
    direction_path = repo_root / direction_str
    if direction_path.exists() and direction_path.is_file():
        return direction_path.read_text()
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
