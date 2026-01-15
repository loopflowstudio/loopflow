# Files

Two features: always-visible context chips in Maestro, and deduplicated file loading in the Python context module.

## Review

**Verdict:** Ready to ship

Both features are well-implemented and follow existing patterns. No issues found.

## Design notes

**Context chips replace hidden panel.** The old `contextOptionsSection` with its checkbox toggles and "Context ▼" disclosure button is gone. The new `contextBar` shows four colored `ContextChip` views (Docs/Files/Diff/Clipboard) plus attached file chips, always visible below the mode picker. Token count moved from the input area to the right side of the context bar.

**File loading merges before `gather_files`.** Previously `gather_prompt_components()` called `gather_files()` twice—once for diff files, once for explicit context—then `format_prompt()` deduplicated by path. Now the merge happens before loading:

```python
diff_set = set(diff_file_paths)
all_file_paths = diff_file_paths + [p for p in context_paths if p not in diff_set]
all_files = gather_files(all_file_paths, repo_root, context_exclude)
```

This eliminates duplicate file reads, binary checks, and parent README gathering. The `context_files` field is gone from `PromptComponents`; everything goes in `diff_files`.

**Token analysis simplified.** The `analyze_prompt_tokens()` function no longer takes `context_files` as a separate parameter. All files are reported under the "files" category instead of split between "diff_files" and "context".

**Voice config added to Maestro.** The `LoopflowConfig` now parses `voice:` from YAML (single string or array), and `AppState.openRepo()` initializes `selectedVoices` from config. Token estimation passes all four context flags (`includeDocs`, `includeDiff`, `includeDiffFiles`, `includePaste`) to `lf -c`.
