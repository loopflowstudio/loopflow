# DMG Publishing to loopflow.studio

## What was built

Extended the publish script to build the Maestro DMG and upload it to Cloudflare R2 for download.

## CLI interface

```bash
# Full release (PyPI + DMG)
python scripts/publish.py patch

# DMG only (no PyPI, no version bump)
python scripts/publish.py --dmg-only

# Skip DMG during release
python scripts/publish.py patch --skip-dmg

# Dry run shows all steps
python scripts/publish.py patch --dry-run
python scripts/publish.py --dmg-only --dry-run
```

The `--dmg-only` flag:
- Builds Maestro DMG using current version from `__init__.py`
- Uploads to R2
- Skips: tests, version bump, git commit/tag, PyPI publish
- Useful for: fixing DMG-only bugs, re-uploading after failed upload, testing

## Key functions

```python
# src/loopflow/publish.py

R2_PUBLIC_URL = "https://downloads.loopflow.studio"

def build_dmg(repo_root: Path) -> tuple[bool, str]:
    """Build Maestro DMG. Returns (success, output)."""
    # Runs: cd Maestro && ./dev release

def get_dmg_path(repo_root: Path) -> Path:
    """Get path to built DMG."""
    # Returns: repo_root / "Maestro" / "dist" / "LoopflowMaestro.dmg"

def upload_dmg(dmg_path: Path, version: str) -> tuple[bool, str]:
    """Upload DMG to Cloudflare R2. Returns (success, output)."""
    # Uploads to: LoopflowMaestro-{version}.dmg
    # Also uploads to: LoopflowMaestro-latest.dmg
```

## Infrastructure requirements

1. **Cloudflare R2 bucket**: `loopflow-downloads` (or set via `R2_BUCKET_NAME`)
2. **Custom domain**: `downloads.loopflow.studio` pointing to R2 bucket
3. **Credentials**: Environment variables required when running publish:
   - `R2_ACCOUNT_ID`
   - `R2_ACCESS_KEY_ID`
   - `R2_SECRET_ACCESS_KEY`
   - `R2_BUCKET_NAME` (optional, defaults to `loopflow-downloads`)

## What's left

- Website `/download` page showing both Maestro and CLI install options (separate repo: loopflowstudio)
- Initial R2 bucket setup and domain configuration

## Testing

```bash
# Test dry run
python scripts/publish.py --dmg-only --dry-run

# Expected output:
# Current version: 0.6.2
# Would build Maestro DMG
# Would upload DMG to https://downloads.loopflow.studio/LoopflowMaestro-0.6.2.dmg
# Would upload DMG to https://downloads.loopflow.studio/LoopflowMaestro-latest.dmg
```
