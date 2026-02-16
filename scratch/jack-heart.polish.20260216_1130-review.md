# Branch Review: jack-heart.polish.20260216_1130

## What was implemented

This branch tightens CLI/docs parity and command discoverability across `lf` and `lfq`.

- Updated `lf` docs to match actual flags/subcommands (`-a/--area`, `-w/--wave`, `-b/--batch`, flow invocation syntax, config-only lfdocs behavior).
- Removed docs for non-existent `lf ops` commands (`add`, `version`, `summarize`) and removed stale references in config/troubleshooting docs.
- Added missing builtin steps to `lf --list` metadata and descriptions (`compress`, `gate`, `5whys`, `ingest`, `kickoff`, `wave-plan`, `add-to-wave`, `consolidate`, `synthesize`, `update-wave`, `validate`).
- Added help text for previously undocumented `lf ops`, `lf ops wt`, and `lf ops shell` subcommands and key positional args.
- Added actionable error hints in Rust/Python CLIs (`lf --list`, `lf ops doctor`, `lfq list`, `.lf/logs/`).
- Added CLI toggles for branch diff context (`--diff-files/--no-diff-files`, `--diff/--no-diff`) and wired them into prompt context gathering.
- Added `lfq` command help descriptions for discoverability.

## Key choices

- **Chose docs cleanup over phantom command implementation** for `lf ops add/version/summarize` to avoid inventing unsupported behavior.
- **Added CLI diff toggles in code** (instead of only removing docs) because underlying context plumbing already existed and this improves command-line ergonomics.
- **Kept changes scoped to polish priorities**: no unrelated workflow rewrites or behavioral shifts outside docs/help/error UX.

## How it fits together

`lf` argument parsing now understands diff-context toggles and passes them through `Cli` into `run::build_prompt()`, where they override config defaults for `GatherContextOpts`. Discovery/listing uses expanded builtin metadata so `lf --list` reflects all shipped builtin steps. Documentation was updated to align with actual clap surfaces and current command routing.

## Risks and bottlenecks

- `--summaries/--no-summaries` is now a parsed CLI surface; summary ingestion is still limited by existing engine behavior, so this flag is mainly parity/discoverability scaffolding.
- README still uses `lf flow ...` examples from existing baseline docs; this branch corrects `docs/lf.md` flow invocation but does not fully normalize every README example.

## What's not included

- No new `lf ops summarize` implementation.
- No changes to summary generation pipeline.
- No Swift/Concerto code changes.
- No broad README overhaul beyond scope of the polish-priority fixes.
