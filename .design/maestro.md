# Maestro

Refactors task execution into a shared helper, consolidates `write_prompt_file` into logging module, fixes API key leakage in Codex subprocess calls, and cleans up remote branches after `lf pr land`.

## Review

**Verdict:** Ready to ship

The changes are clean and well-structured. The refactoring reduces code duplication substantially and the bug fixes are straightforward.

### Changes Summary

1. **`_execute_task` helper** (`cli/run.py`): Extracts ~90 lines of duplicated task execution logic from `run()` and `inline()` into a shared function. Handles session creation, command building, interactive vs auto mode dispatch, and cleanup.

2. **`write_prompt_file` consolidation** (`logging.py`): Moves the temp file helper from `cli/run.py` and `pipeline.py` into `logging.py` as a single source. Both modules now import from there.

3. **Codex API key leak fix** (`launcher.py`): `CodexRunner.launch()` now passes `env=get_model_env()` to subprocess calls, stripping API keys so Codex uses the ChatGPT subscription instead of API credits.

4. **Remote branch cleanup** (`cli/pr.py`): After squash-merging via `lf pr land`, the remote branch is now deleted with `git push origin --delete`. Silently ignores failures (e.g., already deleted by GitHub).

5. **Tests**: Added tests for `write_prompt_file` covering basic functionality and unicode handling.

### Notes

- The `_execute_task` docstring describes the function well; types could be tightened (`components` is `PromptComponents` but annotated loosely).
- The removed inline comments ("No flag: use config or default (auto)") were correct to delete—they restated what the code does.
- The `.gitignore` and `.lf/config.yaml` uncommitted changes are local config, not part of this branch's purpose.
