# Skip Binary Files in Context

**What to build:** Automatic detection and skipping of binary files when gathering context and diffs.

## Problem

When gathering context with directory or glob patterns (e.g., `lf implement -x .`), binary files cause `UnicodeDecodeError`:

```
UnicodeDecodeError: 'utf-8' codec can't decode byte 0x89 in position 0: invalid start byte
```

The `0x89` byte is the PNG magic header. Currently, `gather_files()` calls `path.read_text()` on all files without checking if they're text.

## Data Structures

No new types needed. Add a detection function:

```python
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
```

The null-byte check is the standard heuristic used by git, file(1), and most editors.

## APIs

### `is_binary(path: Path) -> bool`

Returns True if the file should be skipped. Checks extension first (fast), then sniffs content (slower but catches unlisted types).

### Changes to `gather_file()`

```python
def gather_file(path: Path, repo_root: Path, exclude: Optional[list[str]] = None) -> tuple[Path, str] | None:
    """Gather a single file if it exists, isn't ignored, and isn't binary."""
    if not path.exists():
        return None
    if not path.is_file():
        return None
    if is_ignored(path, repo_root, exclude):
        return None
    if is_binary(path):          # NEW
        return None
    return (path, path.read_text())
```

### Changes to `gather_diff()`

Git already handles binary files in diffs (shows "Binary files X and Y differ"). No code changes needed—git's output is safe.

Verified by checking git diff output:
> Git handles binary files fine (shows "Binary files differ").

## Constraints

1. **Extension list is conservative.** Only truly binary formats are listed. Text formats like `.svg` (XML), `.lock` (TOML/JSON), and `.json` are NOT in the list—they're valid context.

2. **Null-byte detection is the fallback.** Unlisted extensions get content-sniffed. The null-byte heuristic matches what git and file(1) use.

3. **Match existing patterns.** Follow the `is_ignored()` pattern—check at the `gather_file()` level so all callers benefit.

## Done When

```bash
# This should work without errors:
cd ~/src/lf/cadenza
uv run python -c "
from loopflow.files import gather_files
from pathlib import Path
files = gather_files(['.'], Path.cwd())
print(f'Got {len(files)} files')
for p, _ in files[:5]:
    print(f'  {p.name}')
"

# Should see text files only, no UnicodeDecodeError
```

And tests pass:
```bash
uv run pytest tests/test_files.py -v
```
