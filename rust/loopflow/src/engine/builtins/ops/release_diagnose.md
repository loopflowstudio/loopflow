Diagnose and fix a failed release workflow.

Use the provided logs and repo state to identify the root cause, apply a fix, and prepare for re-tagging.

## Requirements

1. Identify the first concrete failure cause from logs.
2. Apply the minimal code/workflow/config fix in this repo.
3. Ensure the fix is committed in a clean state.
4. Summarize what failed, what changed, and why this should pass on re-run.

## Constraints

- Keep fixes scoped to the release failure.
- Do not ask questions; make best assumptions and proceed.
- Prefer deterministic fixes over retries or flaky workarounds.
