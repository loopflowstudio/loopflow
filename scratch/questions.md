# Concerto Wave: Ingest Blocked

## Situation

Phase 1 (Polish - macOS local) is marked "In progress" in `roadmap/concerto/README.md`, but there are no Phase 1 items in the backlog. All items in `roadmap/concerto/` are Phase 2 or Phase 3.

## Options

1. **Phase 1 is complete** — Update README to mark Phase 1 done, then pick from Phase 2
2. **Phase 1 items are missing** — Add Phase 1 work items to the backlog
3. **Phase 1 work lives elsewhere** — Items like `reports/cli/ux-polish.md` may be intended as Phase 1 work but aren't in the roadmap structure

## What's needed

Clarification on whether Phase 1 work is complete or what items should be added to the Phase 1 backlog.

# Screenshot pipeline blocked (2026-01-31)

## What happened

- `uv run python scripts/generate_screenshots.py` timed out after 5 minutes.
- `uv run lf ux-review --direction conductor --area docs/screenshots/` timed out after 2 minutes.

## Likely causes

- Concerto build + launch may be slow or waiting on Xcode build prompts.
- Screen Recording permission is required for Swift screenshots.
- The UX review step likely expects screenshot files that were not generated.

## What I need

- Confirmation on whether to rerun with longer timeouts or on a machine with screen recording enabled.
- If the screenshot pipeline must be skipped for now, should we accept the placeholder Phase 1 item and proceed?
