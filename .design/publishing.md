# scripts/publish.py

## What to build

A repo-local script to build, publish to PyPI, and update the local installation—mostly deterministic Python with one LLM call for release notes.

## Implementation notes

Changed from `lfops publish` to `scripts/publish.py` since publishing is repo-specific, not a general loopflow feature. Also reorganized dev scripts:

- `scripts/publish.py` — publish loopflow to PyPI
- `Maestro/dev` — Swift build commands (build, test, run, xcode, release)

## Data structures

```python
# In llm_http.py
class ReleaseNotes(BaseModel):
    summary: str      # 2-3 sentences
    changes: list[str]  # bullet points
```

## Key functions

```python
# In llm_http.py
def generate_release_notes(repo_root: Path, old_version: str, new_version: str) -> ReleaseNotes:
    """Generate release notes from commits since last tag via API."""

# In scripts/publish.py
def main() -> int:
    # argparse: bump (patch/minor/major), --dry-run, --skip-tests, --force
```

## Release notes prompt

Added `src/loopflow/builtins/release_notes.txt` — outcome-focused style like draft_commit.

## Usage

```bash
./scripts/publish.py                    # patch bump (default)
./scripts/publish.py minor              # minor bump
./scripts/publish.py --dry-run          # preview without executing
./scripts/publish.py --force            # bypass main branch check (for testing)
./scripts/publish.py --skip-tests       # skip test run
```

## Done when

```bash
# Help shows options
./scripts/publish.py --help

# Dry run works
./scripts/publish.py --dry-run --force
```
