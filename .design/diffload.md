# diffload

Add `diff_files` context source (full file content for files touched by branch) and flip `diff` default to off.

## Review

**Verdict:** Ready to ship

Implementation is clean and complete. All touched files follow the existing patterns. The design spec is fully implemented:

- `diff_files` field added to `Config` (default: True), `PromptComponents`, and `AgentFile`
- `gather_diff_files()` correctly filters deleted files and returns paths for `gather_files()` to load
- CLI flags `--diff-files/--no-diff-files` added to `run()`, `inline()`, `cp()`
- Token analysis shows `diff_files` as separate category
- Files merge with `context_files` in `<lf:files>` section, deduplicated by path
- `docs/config.md` updated with new options

No issues found.

## Design notes

**Why two options?** `diff_files` loads complete file content—lets the LLM see surrounding context, imports, full functions. `diff` shows exactly what changed but lacks context. Most tasks work better with files; some may want both or neither.

**Exclude patterns applied at load time.** `gather_diff_files()` returns raw paths; exclusions happen when `gather_files()` loads content. This keeps the function simple and consistent with how `-x` context works.
