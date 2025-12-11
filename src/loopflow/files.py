"""File library gathering for LLM context."""

from functools import lru_cache
from pathlib import Path
from typing import Optional

import pathspec


# Known binary extensions (skip without reading)
_BINARY_EXTENSIONS: set[str] = {
    # Images
    ".png", ".jpg", ".jpeg", ".gif", ".ico", ".webp", ".bmp", ".tiff",
    # Archives
    ".zip", ".tar", ".gz", ".bz2", ".7z", ".rar",
    # Executables/libraries
    ".exe", ".dll", ".so", ".dylib", ".o", ".a",
    # Documents
    ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
    # Media
    ".mp3", ".mp4", ".wav", ".avi", ".mov", ".mkv",
    # Fonts
    ".ttf", ".otf", ".woff", ".woff2", ".eot",
    # Other
    ".pyc", ".class", ".sqlite", ".db",
}


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


def _matches_glob_pattern(path: Path, pattern: str, repo_root: Path) -> bool:
    """Check if path matches a glob pattern using Path.glob semantics."""
    matching_paths = set(repo_root.glob(pattern))
    return path in matching_paths


def is_ignored(path: Path, repo_root: Path, exclude: Optional[list[str]] = None) -> bool:
    """Check if path should be excluded from context.

    Excludes .git, .lf (prompt config), gitignored paths, and paths matching exclude patterns.
    Exclude patterns use Path.glob semantics (*.md = root only, **/*.md = recursive).
    """
    rel_path = path.relative_to(repo_root)

    # Always ignore .git directory
    if ".git" in rel_path.parts:
        return True

    # Always ignore .lf directory at repo root (prompt configuration, not context)
    if rel_path.parts and rel_path.parts[0] == ".lf":
        return True

    # Check exclude patterns (Path.glob style)
    if exclude:
        for pattern in exclude:
            if _matches_glob_pattern(path, pattern, repo_root):
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


def gather_docs(path: Path, repo_root: Path, exclude: Optional[list[str]] = None) -> list[tuple[Path, str]]:
    """Gather .md files from path up to repo root.

    If path is a file, starts from its parent directory.
    Returns docs in root-to-leaf order, with files sorted alphabetically within each directory.
    """
    docs_by_dir: list[list[tuple[Path, str]]] = []
    current = path.parent if path.is_file() else path

    while current >= repo_root:
        dir_docs = []
        for md_file in sorted(current.glob("*.md")):
            if md_file.is_file() and not is_ignored(md_file, repo_root, exclude):
                dir_docs.append((md_file, md_file.read_text()))
        if dir_docs:
            docs_by_dir.append(dir_docs)
        current = current.parent

    # Reverse directory order (root first), flatten
    result = []
    for dir_docs in reversed(docs_by_dir):
        result.extend(dir_docs)
    return result


def gather_file(path: Path, repo_root: Path, exclude: Optional[list[str]] = None) -> tuple[Path, str] | None:
    """Gather a single file if it exists, isn't ignored, and isn't binary."""
    if not path.exists():
        return None
    if not path.is_file():
        return None
    if is_ignored(path, repo_root, exclude):
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


def gather_files(paths: list[str], repo_root: Path, exclude: Optional[list[str]] = None) -> list[tuple[Path, str]]:
    """Gather files and their parent READMEs.

    Returns list of (path, content) tuples, deduplicated and ordered.
    """
    seen: set[Path] = set()
    results: list[tuple[Path, str]] = []

    for path_str in paths:
        expanded = _expand_path(path_str, repo_root)

        for path in expanded:
            # Gather parent documentation first
            for doc_path, content in gather_docs(path, repo_root, exclude):
                if doc_path not in seen:
                    seen.add(doc_path)
                    results.append((doc_path, content))

            # Gather the file itself
            file_result = gather_file(path, repo_root, exclude)
            if file_result and file_result[0] not in seen:
                seen.add(file_result[0])
                results.append(file_result)

    return results


def format_files(files: list[tuple[Path, str]], repo_root: Path) -> str:
    """Format files with unique delimiters for unambiguous parsing."""
    if not files:
        return ""

    parts = []
    for path, content in files:
        relative = path.relative_to(repo_root)
        parts.append(f"<lf:file path=\"{relative}\">\n{content}\n</lf:file>")

    body = "\n\n".join(parts)
    return f"Reference files for this task. Includes parent documentation for context.\n\n<lf:files>\n{body}\n</lf:files>"
