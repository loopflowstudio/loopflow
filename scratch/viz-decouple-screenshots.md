---
status: done
phase: 1
---

# Decouple screenshots from publish

Screenshot generation removed from `_release()` in `publish.py`. Screenshots are now their own concern.

## What changed

1. **`scripts/publish.py`** — removed `skip_screenshots` parameter from `_release()`, `patch()`, `minor()`, `major()`. The `_generate_screenshots()` helper and `screenshots` subcommand are retained.

2. **`.lf/steps/screenshots.md`** — new step. Runs `generate_screenshots.py`, checks for changes, commits if any.

3. **`roadmap/viz/README.md`** — 01-decouple-screenshots marked done, item file deleted.

## Two paths remain

- `publish.py screenshots` — manual convenience, no commit
- `lf screenshots` — intended workflow, generates + commits

## Key decisions

**Step, not ops command.** Screenshots are a Python script + commit. Steps chain naturally (`lf screenshots && lf ux-review`).

**Keep `publish.py screenshots`.** Zero-cost convenience for manual runs. 8 lines, no maintenance burden.

**Commit in the step, not the script.** `generate_screenshots.py` stays a pure generation tool.
