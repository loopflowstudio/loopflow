---
requires: none
produces: docs/screenshots/*.png
---
Generate fresh Concerto screenshots and commit results.

## Workflow

1. Run the screenshot generator:
   ```bash
   uv run python scripts/generate_screenshots.py
   ```

2. Check what changed:
   ```bash
   git diff --stat docs/screenshots/
   ```

3. If screenshots changed, commit them:
   ```bash
   git add docs/screenshots/
   git commit -m "screenshots: refresh docs/screenshots/"
   ```

4. If nothing changed, report that screenshots are up to date.
