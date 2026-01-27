"""File library gathering for LLM context."""

from dataclasses import dataclass, field
from functools import lru_cache
from pathlib import Path
from typing import Optional

import pathspec

# =============================================================================
# Markdown file lookup helpers (used by context.py and directions.py)
# =============================================================================


def find_md_in_dir(base_dir: Path, name: str) -> Path | None:
    """Find a .md file by name in base_dir or its subdirectories.

    Supports both 'name' and 'folder/name' formats.
    If just 'name', searches subdirectories for unique match.
    """
    if not base_dir.exists():
        return None

    # Try exact path first (with or without folder prefix)
    exact = base_dir / f"{name}.md"
    if exact.exists():
        return exact

    # Search all .md files for matching stem
    matches = []
    for p in base_dir.rglob("*.md"):
        if p.stem == name:
            matches.append(p)

    if len(matches) == 1:
        return matches[0]
    return None  # Ambiguous or not found


def list_md_grouped(base_dir: Path) -> dict[str, list[str]]:
    """List all .md files grouped by parent folder.

    Returns dict mapping folder name (or '' for root) to list of stems.
    """
    if not base_dir.exists():
        return {}

    grouped: dict[str, list[str]] = {}
    for p in base_dir.rglob("*.md"):
        rel = p.relative_to(base_dir)
        if len(rel.parts) == 1:
            folder = ""
        else:
            folder = str(rel.parent)
        grouped.setdefault(folder, []).append(p.stem)

    # Sort within each group
    for folder in grouped:
        grouped[folder] = sorted(grouped[folder])
    return grouped


# =============================================================================
# File gathering for LLM context
# =============================================================================


@dataclass
class GatherResult:
    """Result of gathering files for context."""

    text_files: list[tuple[Path, str]] = field(default_factory=list)
    image_files: list[Path] = field(default_factory=list)


# Image extensions (tracked separately, not embedded in text)
_IMAGE_EXTENSIONS: set[str] = {
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".webp",
    ".bmp",
    ".tiff",
    ".ico",
}

# Known binary extensions (skip without reading)
_BINARY_EXTENSIONS: set[str] = _IMAGE_EXTENSIONS | {
    # Archives
    ".zip",
    ".tar",
    ".gz",
    ".bz2",
    ".7z",
    ".rar",
    # Executables/libraries
    ".exe",
    ".dll",
    ".so",
    ".dylib",
    ".o",
    ".a",
    # Documents
    ".pdf",
    ".doc",
    ".docx",
    ".xls",
    ".xlsx",
    ".ppt",
    ".pptx",
    # Media
    ".mp3",
    ".mp4",
    ".wav",
    ".avi",
    ".mov",
    ".mkv",
    # Fonts
    ".ttf",
    ".otf",
    ".woff",
    ".woff2",
    ".eot",
    # Other
    ".pyc",
    ".class",
    ".sqlite",
    ".db",
}


def is_image(path: Path) -> bool:
    """Check if file is an image by extension."""
    return path.suffix.lower() in _IMAGE_EXTENSIONS


def is_binary(path: Path) -> bool:
    """Check if file is binary by extension or content sniffing."""
    # Fast path: check extension
    if path.suffix.lower() in _BINARY_EXTENSIONS:
        return True

    # Slow path: read first bytes and check for null bytes
    try:
        with open(path, "rb") as f:
            chunk = f.read(8192)
            return b"\x00" in chunk
    except (OSError, IOError):
        return True  # Can't read = skip it


@lru_cache(maxsize=64)
def _load_gitignore(gitignore_path: Path) -> pathspec.PathSpec | None:
    """Load and parse a .gitignore file."""
    if not gitignore_path.exists():
        return None
    patterns = gitignore_path.read_text().splitlines()
    return pathspec.PathSpec.from_lines("gitwildmatch", patterns)


def _compile_exclude_patterns(patterns: list[str], repo_root: Path) -> set[Path]:
    """Pre-compute all paths matching exclude patterns.

    Runs glob once per pattern instead of once per file, giving O(patterns)
    instead of O(patterns * files) glob operations.
    """
    excluded: set[Path] = set()
    for pattern in patterns:
        excluded.update(repo_root.glob(pattern))
    return excluded


def _is_excluded_by_paths(path: Path, excluded: set[Path]) -> bool:
    if path in excluded:
        return True
    for parent in path.parents:
        if parent in excluded:
            return True
    return False


def _is_ignored(
    path: Path,
    repo_root: Path,
    excluded_paths: set[Path] | None,
) -> bool:
    """Check if path should be excluded from context.

    Excludes .git, .lf (prompt config), gitignored paths, and paths matching
    exclude patterns. Patterns use Path.glob semantics (*.md = root only,
    **/*.md = recursive).
    """
    rel_path = path.relative_to(repo_root)

    # Always ignore .git directory
    if ".git" in rel_path.parts:
        return True

    # Always ignore .lf directory at repo root (prompt configuration, not context)
    if rel_path.parts and rel_path.parts[0] == ".lf":
        return True

    # Check pre-compiled exclude patterns.
    if excluded_paths is not None and _is_excluded_by_paths(path, excluded_paths):
        return True

    # Check .gitignore files from repo root down to path's parent
    # Each .gitignore matches paths relative to its own directory
    parts = rel_path.parts
    for i in range(len(parts)):
        current_dir = repo_root / Path(*parts[:i]) if i > 0 else repo_root
        gitignore = current_dir / ".gitignore"
        spec = _load_gitignore(gitignore)
        if spec:
            # Path relative to this .gitignore's directory
            rel_from_here = Path(*parts[i:])
            if spec.match_file(str(rel_from_here)):
                return True

    return False


def _gather_docs(
    path: Path,
    repo_root: Path,
    excluded_paths: set[Path] | None,
) -> list[tuple[Path, str]]:
    """Gather .md files from path up to repo root.

    If path is a file, starts from its parent directory.
    Returns docs in root-to-leaf order, sorted alphabetically within each dir.
    """
    docs_by_dir: list[list[tuple[Path, str]]] = []
    current = path.parent if path.is_file() else path

    while current >= repo_root:
        dir_docs = []
        for md_file in sorted(current.glob("*.md")):
            if md_file.is_file() and not _is_ignored(md_file, repo_root, excluded_paths):
                dir_docs.append((md_file, md_file.read_text()))
        if dir_docs:
            docs_by_dir.append(dir_docs)
        current = current.parent

    # Reverse directory order (root first), flatten
    result = []
    for dir_docs in reversed(docs_by_dir):
        result.extend(dir_docs)
    return result


def gather_docs(
    path: Path,
    repo_root: Path,
    exclude: Optional[list[str]] = None,
) -> list[tuple[Path, str]]:
    excluded_paths = _compile_exclude_patterns(exclude, repo_root) if exclude else None
    return _gather_docs(path, repo_root, excluded_paths)


def _gather_file(
    path: Path,
    repo_root: Path,
    excluded_paths: set[Path] | None,
) -> tuple[Path, str] | None:
    """Gather a single file if it exists, isn't ignored, and isn't binary."""
    if not path.exists():
        return None
    if not path.is_file():
        return None
    if _is_ignored(path, repo_root, excluded_paths):
        return None
    if is_binary(path):
        return None
    return (path, path.read_text())


def _expand_path(path_str: str, repo_root: Path) -> list[Path]:
    """Expand a path string to a list of file paths.

    Handles:
    - Regular files: returns [path]
    - Directories: returns all files recursively
    - Glob patterns (* or **): returns matching files
    """
    # Check for glob patterns
    if "*" in path_str:
        return sorted(repo_root.glob(path_str))

    path = (repo_root / path_str).resolve()

    if path.is_file():
        return [path]

    if path.is_dir():
        return sorted(path.rglob("*"))

    return []


def gather_files(
    paths: list[str], repo_root: Path, exclude: Optional[list[str]] = None
) -> GatherResult:
    """Gather files and their parent READMEs.

    Returns GatherResult with text files (path, content) and image file paths.
    Images are tracked separately since they can't be embedded in text prompts.
    """
    seen: set[Path] = set()
    text_files: list[tuple[Path, str]] = []
    image_files: list[Path] = []

    # Pre-compile exclude patterns once for the entire gather operation
    excluded_paths = _compile_exclude_patterns(exclude, repo_root) if exclude else None

    for path_str in paths:
        expanded = _expand_path(path_str, repo_root)

        for path in expanded:
            if path in seen:
                continue

            # Check if this is an image
            if (
                path.is_file()
                and is_image(path)
                and not _is_ignored(path, repo_root, excluded_paths)
            ):
                seen.add(path)
                image_files.append(path)
                continue

            # Gather parent documentation first
            for doc_path, content in _gather_docs(path, repo_root, excluded_paths):
                if doc_path not in seen:
                    seen.add(doc_path)
                    text_files.append((doc_path, content))

            # Gather the file itself (skips binary including images)
            file_result = _gather_file(path, repo_root, excluded_paths)
            if file_result and file_result[0] not in seen:
                seen.add(file_result[0])
                text_files.append(file_result)

    return GatherResult(text_files=text_files, image_files=image_files)


def format_files(files: list[tuple[Path, str]], repo_root: Path) -> str:
    """Format files with unique delimiters for unambiguous parsing."""
    if not files:
        return ""

    parts = []
    for path, content in files:
        relative = path.relative_to(repo_root)
        parts.append(f'<lf:file path="{relative}">\n{content}\n</lf:file>')

    body = "\n\n".join(parts)
    header = "Reference files for this task. Includes parent documentation for context."
    return f"{header}\n\n<lf:files>\n{body}\n</lf:files>"


def format_image_references(images: list[Path], repo_root: Path) -> str:
    """Format image file references for the prompt.

    Images can't be embedded in text, but we tell the agent where they are
    so it can read them using its file tools.
    """
    if not images:
        return ""

    lines = ["The following images are available. Use your Read tool to view them:"]
    for img in images:
        # Use relative path for repo files, absolute for external (e.g., clipboard)
        try:
            display_path = img.relative_to(repo_root)
        except ValueError:
            display_path = img
        lines.append(f"- {display_path}")

    return "<lf:images>\n" + "\n".join(lines) + "\n</lf:images>"
