# Code Quality Improvements

Low-effort cleanups from codebase research.

## Items

### Document token trimming strategy

`context.py:450-490` trims components without explaining priority. Add a comment block explaining why diff_files drops before clipboard, and the rationale for the ordering.

**Effort**: Low (comments only)

### Clean up Choose handling

`flow.py:420-450` has `choose_branch()` that calls the runner directly rather than going through the standard step execution path. Route through the same logging/error handling as Steps.

**Effort**: Low (isolated function)

### Remove or document worker.py

`lfd/execution/worker.py` exists but `runner.py` handles execution. Either delete if unused or add docstring explaining its purpose.

**Effort**: Low (investigate then delete or document)

### Add fork/synthesize tests

`tests/test_flows.py` tests Flow parsing but not fork execution. `flow.py:run_fork()` and `run_synthesize()` have no dedicated tests. Fork is a key feature used in roadmap flows.

**Effort**: Medium (need to mock worktree creation and parallel execution)
