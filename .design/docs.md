# Docs

Documentation refresh: added public docs for Maestro, lfd, API reference, and philosophy. Added internal READMEs for lfd and Maestro modules.

## Review

**Verdict:** Needs work

### Issues

**1. Uncommitted code changes mixed with docs**

The uncommitted diff includes significant code changes that go beyond documentation:
- Deletes entire `loopflow/maestro` Python module (~15 files)
- Adds new files to `loopflow/lfd`: `agent_runner.py`, `collector.py`, `runner.py`
- Changes `lfops.py` imports from `maestro.db` to `lfd.db`
- Updates Swift files to read from `lfd.db` instead of `maestro.db`

This is code reorganization, not documentation. The design doc explicitly flags this as "Out of Scope" for follow-up. Either commit these code changes separately or revert them before landing the docs branch.

**2. New Python files not committed**

`git status` shows untracked files:
- `src/loopflow/lfd/agent_runner.py`
- `src/loopflow/lfd/collector.py`
- `src/loopflow/lfd/runner.py`

These are presumably copies/moves from the deleted `maestro` module but they're not staged. The branch won't work without them.

**3. Test file imports from deleted module**

`tests/test_collector.py:6` imports:
```python
from loopflow.lfd.collector import _format_stream_line
```

This will fail if `collector.py` isn't committed. Many other tests are deleted (`test_agent.py`, `test_daemon.py`, `test_maestro.py`, etc.) - verify that remaining tests pass.

### Documentation quality

The documentation itself is well-written:

- **docs/maestro.md** - Clear user guide covering installation, prompt launcher, worktree sidebar, agents panel
- **docs/lfd.md** - Good coverage of daemon installation, session tracking, agents, database schema
- **docs/api.md** - Comprehensive protocol reference with examples in Python, Swift, and shell
- **docs/vision.md** - Effective distillation of `.research/` content into public-facing philosophy
- **src/loopflow/lfd/README.md** and **Maestro/README.md** - Useful internal docs per STYLE.md

The navigation header in `docs/_config.yml` is updated correctly.

### Recommendation

1. Separate code changes from docs - create a new branch for the `maestro` → `lfd` consolidation
2. Commit the untracked lfd files
3. Run tests to verify nothing is broken
4. Land docs-only changes first, then land code reorganization

## Design notes

**Why both socket and direct DB reads in Maestro**: Direct DB reads let Maestro show history even if lfd crashed. Socket events provide real-time updates. This is documented in `Maestro/README.md` and is the correct architecture.

**Fire-and-forget session logging**: Session logging uses synchronous socket calls with 0.5s timeout that fail silently. Correct tradeoff—lfd availability shouldn't block task execution.
