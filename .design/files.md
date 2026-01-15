# Deduplicate diff_files and context files loading

## What to build

Prevent files touched by the branch from being loaded twice when they also appear in explicit `-x` context.

## The problem

Currently, `gather_prompt_components()` in `context.py` loads files in two separate calls:

```python
# Line 254-255: Load files touched by branch
diff_file_paths = gather_diff_files(repo_root) if include_diff_files else []
diff_files_content = gather_files(diff_file_paths, repo_root, context_exclude)

# Line 257-259: Load explicit context files
context_files = gather_files(context, repo_root, context_exclude) if context else []
```

Deduplication happens later in `format_prompt()` (lines 324-330), but by then we've already:
1. Read file content twice
2. Gathered parent READMEs twice (via `_gather_docs`)
3. Run binary detection twice
4. Run gitignore checks twice

If a user runs `lf review -x src/foo.py` and `src/foo.py` is in the diff, it gets loaded twice.

## Data structures

No new types needed. The fix is in the loading logic.

## Key functions

```python
def gather_prompt_components(...) -> PromptComponents:
    """Change: merge paths before loading, not after."""
    ...
    # Current: two gather_files calls, dedupe in format_prompt
    # New: combine paths, single gather_files call, store in diff_files
```

## Constraints

- Deduplication must prefer explicit context order when specified (user's `-x` paths should appear in their given order)
- diff_files should still appear first in the final output (they're the "what changed" context)
- Parent README gathering in `gather_files()` already handles deduplication internally via `seen` set

## Implementation

In `gather_prompt_components()`, combine the two file sets before loading:

```python
# Gather file paths (not content yet)
diff_file_paths = gather_diff_files(repo_root) if include_diff_files else []
context_paths = context or []

# Merge: diff files first, then context (gather_files dedupes internally)
all_file_paths = diff_file_paths + [p for p in context_paths if p not in set(diff_file_paths)]
all_files = gather_files(all_file_paths, repo_root, context_exclude) if all_file_paths else []
```

Then update `PromptComponents` usage:
- Put merged files in `diff_files` field
- Set `context_files` to empty list
- Remove dedup logic from `format_prompt()` (no longer needed)

Alternative: keep both fields populated but change semantics:
- `diff_files`: files from diff that weren't in explicit context
- `context_files`: files from explicit context

This is more complex and the UI doesn't distinguish them anyway.

## Done when

```bash
# Create a test file, stage it
echo "test" > /tmp/testfile.py
# The same file in diff and context should only appear once in token breakdown
lf review -x /tmp/testfile.py -c 2>&1 | grep -c "testfile.py"
# Should output: 1
```

More rigorous: add a unit test that mocks `gather_files` and verifies it's called once with merged paths.
