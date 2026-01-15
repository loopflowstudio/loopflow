# diffload: Extract files from diff for context

## What to build

Replace auto-loaded diff with auto-loaded file context for files touched by the branch.

## Problem

"It seems like the LLMs do not realize they have the whole diff in their context."

A diff shows changes but lacks surrounding context. When an LLM sees:

```diff
@@ -121,5 +121,8 @@
-    old_line()
+    new_line()
+    another_line()
```

It knows what changed but not what's around it. Three context lines isn't enough to understand the file's structure, imports, or how functions fit together.

Loading the actual files gives the LLM the full picture—it can see the whole function, the class hierarchy, the imports. The diff becomes redundant because the LLM can read the current state directly.

## Data structures

No new types. The change is in `gather_prompt_components()`.

```python
# Current behavior
diff = gather_diff(repo_root, exclude)
context_files = gather_files(context, repo_root, context_exclude) if context else []

# New behavior
diff_files = gather_diff_files(repo_root, exclude)  # List of paths from diff
context_files = gather_files(context + diff_files, repo_root, context_exclude)
diff = None  # Or keep as option, default off
```

## Key functions

```python
def gather_diff_files(repo_root: Path, exclude: list[str] | None = None) -> list[str]:
    """Return list of file paths touched by this branch vs main.

    Uses git diff --name-only main...HEAD.
    Filters out deleted files (they don't exist to load).
    """
```

Modification to `gather_prompt_components()`:
- Call `gather_diff_files()`
- Merge result with explicit `-x` context
- Pass combined list to `gather_files()`

## Constraints

- **Must filter deleted files.** `git diff --name-only` includes deleted files—can't load those.
- **Must respect exclude patterns.** If `exclude: ["*.test.ts"]` is set, don't load test files even if they're in the diff.
- **Config toggle for backwards compat.** Some users may prefer the diff. Add `diff_as_files: bool = True` (default on) to config. When false, loads raw diff as before.

## Config

```yaml
# .lf/config.yaml
diff_as_files: true   # default: load files instead of diff
```

The existing `diff: bool` controls whether to include diff context at all. The new `diff_as_files` controls the format when diff is enabled.

## Done when

```bash
# On a feature branch with changes to src/loopflow/context.py
lf : "list the files in lf:files" -c 2>&1 | grep "context.py"
```

Should show `src/loopflow/context.py` in the `<lf:files>` section, not in `<lf:diff>`.

Alternative verification:
```bash
lf : "what files are in your context?" -c | pbpaste | grep -A5 "lf:files"
```

## Open questions

None blocking. The smallest useful version is clear: parse diff for filenames, load those files instead of raw diff.
