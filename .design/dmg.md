# DMG Publishing to loopflow.studio

## What to build

Extend the publish script to build the Maestro DMG and upload it to loopflow.studio for download, with a new download page showing both the CLI (pip) and GUI (DMG) install options.

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

## Data structures

```python
@dataclass
class DMGBuildResult:
    path: Path           # Local path to built DMG
    version: str         # Version string (matches Python package)
    size_bytes: int      # File size for display
```

No new config structures needed—DMG publishing is part of the standard release flow.

## Key functions

```python
# src/loopflow/publish.py

def build_dmg(repo_root: Path) -> tuple[bool, str]:
    """Build Maestro DMG. Returns (success, output)."""
    # Runs: cd Maestro && ./dev release
    ...

def upload_dmg(dmg_path: Path, version: str) -> tuple[bool, str]:
    """Upload DMG to loopflow.studio static hosting via Cloudflare R2."""
    # Uses boto3 to upload to R2 bucket
    # Target: downloads/LoopflowMaestro-{version}.dmg
    # Also updates downloads/LoopflowMaestro-latest.dmg
    ...
```

```python
# scripts/publish.py additions

# After PyPI publish, before "Installed locally":
# Step 7.5: Build and upload DMG
print("Building Maestro DMG...")
success, output = build_dmg(ROOT)
if not success:
    print("DMG build failed (continuing with PyPI release):", file=sys.stderr)
    print(output, file=sys.stderr)
else:
    print("Uploading DMG...")
    dmg_path = ROOT / "Maestro" / "dist" / "LoopflowMaestro.dmg"
    success, output = upload_dmg(dmg_path, new_version)
    if not success:
        print("DMG upload failed:", file=sys.stderr)
        print(output, file=sys.stderr)
    else:
        print(f"DMG uploaded: https://loopflow.studio/downloads/LoopflowMaestro-{new_version}.dmg")
```

## Website changes

Update `/download` route in `loopflowstudio/website/main.py`:

```python
@rt("/download")
def get():
    return (
        Title("Install Loopflow"),
        Navbar(),
        Main(
            Section(
                Div(
                    Img(src="/static/logo.png", alt="Loopflow", cls="hero-logo"),
                    H1("Install"),

                    # Maestro GUI app
                    Div(
                        H2("Maestro", cls="install-heading"),
                        P("Visual interface for managing prompts and worktrees.", cls="install-desc"),
                        A("Download for macOS", href="/downloads/LoopflowMaestro-latest.dmg", cls="btn btn-primary"),
                        P("Apple Silicon · macOS 15+", cls="system-req"),
                        cls="install-option",
                    ),

                    # CLI
                    Div(
                        H2("CLI", cls="install-heading"),
                        P("Run prompts from the terminal.", cls="install-desc"),
                        Pre(Code("pip install loopflow"), cls="install-code"),
                        P("macOS · Python 3.11+", cls="system-req"),
                        cls="install-option",
                    ),

                    cls="container",
                ),
                cls="hero download-hero",
            ),
        ),
        SiteFooter(),
    )
```

Add route to serve DMG downloads via Cloudflare R2:

```python
@rt("/downloads/{fname:path}")
async def downloads(fname: str):
    """Redirect to Cloudflare R2 for DMG downloads."""
    return RedirectResponse(
        f"https://downloads.loopflow.studio/{fname}",
        status_code=302
    )
```

## Infrastructure setup

1. **Cloudflare R2 bucket**: Create `loopflow-downloads` bucket
2. **Custom domain**: `downloads.loopflow.studio` → R2 bucket (Cloudflare handles this)
3. **Credentials**: R2 API token with write access, stored in environment:
   - `R2_ACCOUNT_ID`
   - `R2_ACCESS_KEY_ID`
   - `R2_SECRET_ACCESS_KEY`
   - `R2_BUCKET_NAME=loopflow-downloads`

## Constraints

- **DMG build requires macOS**: The publish script must run on macOS (already true—loopflow is macOS-only)
- **R2 credentials**: Must be available in environment when running publish
- **Swift toolchain**: Requires Swift 6.0+ for Maestro build
- **Versioning**: DMG version must match Python package version (single source of truth in `__init__.py`)

## Done when

```bash
# On main branch, with all checks passing:
python scripts/publish.py patch --dry-run
```

Shows:
```
Would bump version: 0.6.2 → 0.6.3 (patch)
Would run tests
Would generate release notes
Would commit: release: v0.6.3
Would tag: v0.6.3
Would build package
Would publish to PyPI
Would build Maestro DMG
Would upload DMG to https://loopflow.studio/downloads/LoopflowMaestro-0.6.3.dmg
Would install locally
```

After a real release:
1. `https://loopflow.studio/download` shows both Maestro DMG and CLI options
2. DMG download link works: `https://loopflow.studio/downloads/LoopflowMaestro-latest.dmg`
3. DMG installs and runs on macOS 15+ Apple Silicon

## Testing

### 1. Test DMG build locally

```bash
# Build DMG without uploading
cd Maestro && ./dev release

# Verify output
ls -la dist/LoopflowMaestro.dmg
open dist/LoopflowMaestro.dmg  # Mount and inspect
```

### 2. Test R2 upload (dry run)

```bash
# Set credentials (get from Cloudflare dashboard)
export R2_ACCOUNT_ID="..."
export R2_ACCESS_KEY_ID="..."
export R2_SECRET_ACCESS_KEY="..."
export R2_BUCKET_NAME="loopflow-downloads"

# Dry run DMG-only publish
python scripts/publish.py --dmg-only --dry-run
```

Expected output:
```
Current version: 0.6.2
Would build Maestro DMG
Would upload DMG to https://downloads.loopflow.studio/LoopflowMaestro-0.6.2.dmg
Would upload DMG to https://downloads.loopflow.studio/LoopflowMaestro-latest.dmg
```

### 3. Test upload to R2 (real)

```bash
# First upload—use a test version or current version
python scripts/publish.py --dmg-only

# Verify
curl -I https://downloads.loopflow.studio/LoopflowMaestro-latest.dmg
```

### 4. Test website locally

```bash
cd ../loopflowstudio/website
python dev.py serve

# Open http://localhost:5001/download
# Verify both install options appear
# Click DMG download link (will fail until R2 is set up)
```

### 5. End-to-end test

```bash
# After R2 is configured and website deployed:
curl -L -o /tmp/test.dmg https://loopflow.studio/downloads/LoopflowMaestro-latest.dmg
hdiutil attach /tmp/test.dmg
ls "/Volumes/Loopflow Maestro/"
# Should show: "Loopflow Maestro.app" and "Applications" symlink
hdiutil detach "/Volumes/Loopflow Maestro"
```
