# laozi

Unix socket daemon (`lfd`) for agent orchestration, replacing SQLite polling with real-time IPC.

## Review

**Verdict:** Needs work

### Issues

1. **Inline imports in `__init__.py`** (STYLE violation)

   `src/loopflow/lfd/__init__.py:29`, :52, :84, etc. has inline imports inside command functions:
   ```python
   @app.command()
   def status():
       from loopflow.lfd.launchd import is_running  # inline
   ```
   Move imports to top of file per STYLE.md.

2. **Inline imports in `server.py`** (STYLE violation)

   `src/loopflow/lfd/server.py:95-96`, :111, :132, etc. has inline imports inside handler methods:
   ```python
   async def _handle_status(self) -> Response:
       from loopflow.lfd.agents import list_agents  # inline
   ```
   These should be at file top.

3. **Inline imports in `agents.py`** (STYLE violation)

   `src/loopflow/lfd/agents.py:127-128`, :179, :197-198 has inline imports inside functions.

4. **Missing `lf agent` redirect**

   The design doc says "`lf agent` now shows a redirect message pointing to `lfd`" but `cli/agent.py` was deleted and no redirect was added. Users running `lf agent` will get "unknown command" rather than helpful guidance.

5. **`show` command imports from maestro module**

   `src/loopflow/lfd/__init__.py:162-163`:
   ```python
   from loopflow.maestro.worktree import get_agent_worktree_path
   from loopflow.maestro.markdown import get_agent_file
   ```
   The lfd module shouldn't depend on maestro. Either move these utilities to a shared location or duplicate the minimal code needed.

6. **Unused import in `__init__.py`**

   Line 8: `import subprocess` is only used in `logs` command via `subprocess.run`. Not wrong, but `subprocess` import at module level isn't needed if `logs` could use a different approach.

7. **Test coverage gaps**

   Tests cover serialization and DB operations but not:
   - Agent file parsing (`_parse_agent_file`, `_parse_yaml_frontmatter`)
   - Trigger evaluation (`should_trigger`, `check_main_changed`)
   - Server dispatch (integration test with actual socket)

### Minor

- `db.py:257` has trailing blank lines (3 instead of 1)
- `client.py:91` has trailing blank line

## Design notes

Architecture: `lfd` is a Unix socket daemon (like postgres) that owns agent lifecycle. CLI (`lf`) and Maestro (Swift app) are clients. SQLite persists state across daemon restarts; socket is for real-time IPC.

launchd label: `com.loopflow.lfd`

CLI structure: All agent commands moved to `lfd` binary. Old `lf agent` and `lf ops maestro` commands removed.

Dependencies removed: `fastapi`, `uvicorn` (FastAPI web UI replaced by Maestro app).

Files deleted: `cli/agent.py`, `cli/maestro.py`.
