# Docs

Documentation refresh and code consolidation: added public docs for Maestro, lfd, API reference, and philosophy. Consolidated `maestro` module into `lfd`. Added internal READMEs for lfd and Maestro modules.

## What changed

**Documentation:**
- `docs/maestro.md` - User guide for the macOS app
- `docs/lfd.md` - Daemon installation, session tracking, agents, database schema
- `docs/api.md` - Socket protocol reference with Python, Swift, and shell examples
- `docs/vision.md` - Philosophy distilled from `.research/`
- `src/loopflow/lfd/README.md` - Internal module docs
- `Maestro/README.md` - Internal module docs for Swift app

**Code reorganization:**
- Deleted `src/loopflow/maestro/` module (15 files)
- Added `src/loopflow/lfd/agent_runner.py`, `collector.py`, `runner.py`
- Updated imports from `maestro.db` to `lfd.db`
- Updated Swift files to read from `lfd.db`
- Deleted obsolete tests (`test_agent.py`, `test_daemon.py`, `test_maestro.py`, etc.)

## Status

- All 312 Python tests pass
- Swift build succeeds
- Working tree is clean

## Design notes

**Why both socket and direct DB reads in Maestro**: Direct DB reads let Maestro show history even if lfd crashed. Socket events provide real-time updates. This is documented in `Maestro/README.md`.

**Fire-and-forget session logging**: Session logging uses synchronous socket calls with 0.5s timeout that fail silently. Correct tradeoff—lfd availability shouldn't block task execution.
