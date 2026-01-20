# Implementation Status: agents1 branch

## Design Documents Found

The design docs are in `.docs/loops-*.md` (not `.design/`):

- `.docs/loops-overview.md` - Overview of agent loops concept
- `.docs/loops-db-schema.md` - Database schema for loops and loop_runs tables
- `.docs/loops-lfd-loop.md` - `lfd loop` command design
- `.docs/loops-lfd-status.md` - `lfd status/stop/prs` commands
- `.docs/loops-pr-limit.md` - Outstanding commit counting
- `.docs/loops-lfops-land.md` - Squash-merge to main

## Implementation Status

### ✅ Completed

1. **Database Schema** (`src/loopflow/lfd/db.py`):
   - `loops` table with all fields from design (id, type, goal, repo, personal_main, status, iteration, pr_limit, merge_mode, project_file, pathset, cron, area, pid, created_at)
   - `loop_runs` table with all fields (id, loop_id, iteration, status, started_at, ended_at, worktree, current_step, error, pr_url, pr_number)
   - All CRUD operations implemented (save_loop, get_loop, list_loops, update_loop_status, delete_loop, save_loop_run, get_loop_runs, etc.)

2. **Data Models** (`src/loopflow/lfd/models.py`):
   - `LoopType` enum: LOOP, FLOW, SUBSCRIBE, SCHEDULE
   - `LoopStatus` enum: IDLE, RUNNING, WAITING, ERROR
   - `MergeMode` enum: AUTO, PR, LAND
   - `Loop` dataclass with all fields including `pid`
   - `LoopRun` dataclass with all fields

3. **CLI Commands** (`src/loopflow/lfd/__init__.py`):
   - `lfd loop <goal>` - Start continuous homeostasis loop
   - `lfd flow <goal> --project <file>` - One-off project execution
   - `lfd subscribe <pathset> <goal>` - Subscribe to pathset changes
   - `lfd schedule "<cron>" <goal>` - Schedule loop on cron
   - `lfd status [loop-id]` - Show loop status
   - `lfd stop <loop-id>` - Stop a loop
   - `lfd prs <loop-id>` - Show PRs for a loop
   - `lfd rm <loop-id>` - Remove a loop

4. **Loop Management** (`src/loopflow/lfd/loops.py`):
   - `create_loop()` - Create or get existing loop
   - `start_loop()` - Mark loop as running
   - `stop_loop()` - Stop a running loop
   - Personal-main branch allocation and creation

5. **Goal Loading** (`src/loopflow/lf/goals.py`):
   - `load_goal()` - Parse goal file with frontmatter
   - `list_goals()` - List available goals
   - `goal_exists()` - Check if goal exists

6. **Runner** (`src/loopflow/lfd/runner.py`):
   - Personal-main branch workflow
   - Merge modes (AUTO, PR, SILENT/LAND)
   - PR creation to personal-main

7. **Land Commands** (`src/loopflow/lfops/land.py`):
   - `lfops land --squash` for squash-merging personal-main to main
   - Regular land with auto-rebase

8. **Rebase** (`src/loopflow/lfops/rebase.py`):
   - `lfops rebase` for rebasing onto main

### ⚠️ Incomplete (TODOs in code)

1. **loops.py:112** - `start_loop()` has TODO: "Actually spawn the subprocess to run the loop"
2. **loops.py:124** - `stop_loop()` has TODO: "Actually kill the subprocess if running"

These TODOs indicate that the loop spawning mechanism is stubbed - the CLI commands work, but loops don't actually spawn background processes to run iterations.

### Test Coverage

- All 476 tests pass (up from 411)
- Added tests for:
  - `Loop` and `LoopRun` models
  - Loop database functions (save_loop, get_loop, list_loops, update_loop_status, delete_loop)
  - LoopRun database functions (save_loop_run, get_loop_runs, update_loop_run_status, etc.)
- Still missing tests for:
  - Goal loading (load_goal, list_goals)
  - CLI commands for lfd loop/flow/subscribe/schedule

### Fixes Made This Session

1. **Removed duplicate MergeMode class** - There were two definitions in `models.py`, causing test comparison failures
2. **Added `pid` field to Loop model** - Was missing from the dataclass but present in the database schema

## Summary

The implementation is **substantially complete** for the core infrastructure:
- Database schema matches design ✅
- Models match design ✅
- CLI commands match design ✅
- Personal-main workflow implemented ✅
- Test coverage improved ✅

The main gap is that `start_loop()` and `stop_loop()` are stubs - they update the database status but don't actually spawn/kill background processes. This would be needed for loops to actually run iterations in the background.
