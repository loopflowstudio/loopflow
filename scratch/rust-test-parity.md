# Rust test parity: implemented coverage

## Summary
- Added file gathering utilities to filter gitignored files, skip `.lf/`, dedupe file paths, and ignore binary files.
- Parsed step frontmatter for `model`, `interactive`, and `directions`, and combined step + CLI directions in context assembly.
- Added tests for frontmatter parsing, direction merging, file filtering, and agent command variants.

## Notable behavior
- `GatherContextOpts.files` includes explicit files in the diff_files section and is deduped by path.
- Step content excludes frontmatter when present.

## Tests added
- File gathering (gitignore, .lf exclusion, dedupe, binary skip, specific files).
- Step frontmatter parsing (model, interactive, directions) and error messaging.
- Direction merging order (step then CLI).
- Agent command building (model variants, chrome flag).

