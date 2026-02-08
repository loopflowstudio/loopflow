---
status: todo
phase: 1
---

# Decouple screenshots from publish

Screenshot generation runs during `_release()` in `publish.py`, but publishing no longer submits PRs. Screenshots get left dirty on main.

## Build

- Remove screenshot generation from `_release()` (the `skip_screenshots` param and `_generate_screenshots()` call)
- Keep `publish.py screenshots` as a convenience alias — it already delegates to `generate_screenshots.py`
- Add an `lf screenshots` step that runs the script and commits results

## Current state

`_release()` calls `_generate_screenshots()` unless `--skip-screenshots` is passed. The `publish.py screenshots` subcommand also calls `_generate_screenshots()` independently. These are redundant paths.

`generate_screenshots.py` works standalone. It reads `scripts/screenshots.yaml`, finds/builds Concerto, and writes to `docs/screenshots/`.

## Done when

`lf screenshots` generates fresh screenshots and commits them. Publishing no longer touches screenshots.
