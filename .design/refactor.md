# Refactor

Package reorganization: `cli/` → `lf/`, `lfops.py` → `lfops/` with submodules, move shared types to `lf/models.py`.

## Review

**Verdict:** Needs work

### Uncommitted changes are incomplete

The uncommitted diff shows a partially-complete split of `lfops/commands.py` into submodules (`init.py`, `pr.py`, `land.py`, `commit.py`, `rebase.py`, `summarize.py`). The new `commands.py` imports these modules and calls `register_commands(app)`, but the submodule files don't exist yet:

```python
from loopflow.lfops import init as init_module
from loopflow.lfops import pr as pr_module
# ...
init_module.register_commands(app)
```

Files referenced but missing:
- `src/loopflow/lfops/init.py`
- `src/loopflow/lfops/pr.py`
- `src/loopflow/lfops/land.py`
- `src/loopflow/lfops/commit.py`
- `src/loopflow/lfops/rebase.py`
- `src/loopflow/lfops/summarize.py`

Also references a moved `summarize` module that needs to exist:
- `from loopflow.lfops.summarize import is_stale, load_summary` in `context.py`

The branch is in a broken state: `summarize.py` was deleted but `lfops/summarize.py` doesn't exist yet.

### Work queue removed from context without replacement

The committed changes remove `gather_work_queue()` and the `work_queue` field from `PromptComponents`. This was intentional (fixing a circular import), but the format_prompt logic that output `<lf:work>` is also gone. If work queue context is still wanted, it needs a new home.

### Lazy import in lfops/__init__.py

The uncommitted changes add lazy imports to avoid circular imports:

```python
def main() -> None:
    from loopflow.lfops.commands import main as _main
    _main()
```

This is fine for the entrypoint, but `get_app()` being lazy means tests that import `from loopflow.lfops import get_app` will work differently than those importing `app` directly. The test changes handle this, but it's a subtle API change.

### No _helpers.py visible

The git status shows `src/loopflow/lfops/_helpers.py` as untracked, but it's not in the diff. If this contains shared code (like `_add_commit_push`, `_get_default_branch`, etc.), it should be reviewed to confirm the lfops split is complete.

## Design notes

**Package boundaries:**
- `lf/` — core task execution, shared infrastructure (config, git, files, logging)
- `lf/models.py` — Session/SessionStatus shared between lf and lfd
- `lfops/` — git workflow commands (pr, land, commit, init, etc.)
- `lfd/` — daemon, agents, work queue

**Circular import fix:** Moved Session and fire-and-forget logging from `lfd/client.py` to `lf/models.py`. The lfd modules re-export for backwards compatibility.

**lfops split pattern:** Each submodule has a `register_commands(app)` function that adds its commands to the Typer app. Keeps the split clean without complex imports.
