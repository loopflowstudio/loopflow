# Docs

Documentation refresh and code consolidation: added public docs for Maestro, lfd, API reference, and philosophy. Consolidated `maestro` module into `lfd`. Added live output streaming to Maestro.

## Review

**Verdict:** Ready to ship

Clean consolidation. The old `loopflow.maestro` module (15 files, ~2000 lines) is deleted and functionality moved to `loopflow.lfd`. Tests pass (318 Python, 11 Swift). The output streaming feature completes the documented API by making it useful for observability.

## What changed

**Code reorganization:**
- Deleted `src/loopflow/maestro/` module entirely
- Added `src/loopflow/lfd/agent_runner.py`, `collector.py`, `runner.py`
- Updated imports from `maestro.db` to `lfd.db` across codebase
- Updated Swift files to read from `lfd.db` instead of `maestro.db`
- Deleted obsolete tests (`test_agent.py`, `test_daemon.py`, `test_maestro.py`, etc.)

**Output streaming (new feature):**
- `server.py`: `output.line` handler broadcasts to subscribers
- `collector.py`: `_send_output_line()` streams each formatted line
- `LFDEventService.swift`: Extended to parse `session.*` and `output.line` events
- `AppState.swift`: Added `liveOutputBySession` and `activeSessionIds` state
- `OutputPanel.swift`: New collapsible panel showing live task output
- `ContentView.swift`: Integrated panel below PromptLauncher

**Documentation:**
- `docs/maestro.md` - User guide for the macOS app
- `docs/lfd.md` - Daemon installation, session tracking, agents, database schema
- `docs/api.md` - Socket protocol reference with Python, Swift, and shell examples
- `docs/vision.md` - Philosophy distilled from `.research/`
- `src/loopflow/lfd/README.md` - Internal module docs
- `Maestro/README.md` - Internal module docs for Swift app

## Design notes

**Why both socket and direct DB reads in Maestro**: Direct DB reads let Maestro show history even if lfd crashed. Socket events provide real-time updates. This is documented in `Maestro/README.md`.

**Fire-and-forget session logging**: Session logging uses synchronous socket calls with 0.5s timeout that fail silently. Correct tradeoff—lfd availability shouldn't block task execution.

**Output streaming is fire-and-forget**: Missing a few lines in the UI is acceptable. Log files remain the source of truth for complete output.
