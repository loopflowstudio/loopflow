# Review: Decouple screenshots from publish

## What was implemented

Removed screenshot generation from the `_release()` flow in `publish.py` and added `.lf/steps/screenshots.md` as the new entry point for generating and committing screenshots. Marked roadmap item 01-decouple-screenshots as done.

Three changes:

1. **`scripts/publish.py`** — removed `skip_screenshots` parameter from `_release()`, `patch()`, `minor()`, and `major()`. Removed the conditional screenshot generation block from the release flow. The `_generate_screenshots()` helper and `screenshots` subcommand are retained.

2. **`.lf/steps/screenshots.md`** — new step prompt. Runs `generate_screenshots.py`, checks for changes, commits if any.

3. **`roadmap/viz/README.md`** — 01-decouple-screenshots struck through, link removed, item file deleted.

## Key choices

**Step over ops command.** Screenshots are a Python script + commit — no Rust needed. Steps chain naturally (`lf screenshots && lf ux-review`), ops commands don't.

**Keep `publish.py screenshots`.** Zero-cost convenience for manual runs without the commit step. 8 lines, no maintenance burden.

**Commit in the step, not the script.** `generate_screenshots.py` stays a pure generation tool. The agent writes a meaningful commit message.

## How it fits together

`_release()` → `_generate_screenshots()` path is deleted. Two paths remain:
- `publish.py screenshots` → `_generate_screenshots()` (manual, no commit)
- `lf screenshots` → agent runs `generate_screenshots.py` + commits (intended workflow)

## Risks and bottlenecks

None. This is a pure removal of coupling. Both remaining paths already worked before this change.

## What's not included

- Screenshot coverage gaps (roadmap item 02)
- Persona subdivision
- New manifest entries
