# Loop → Job Rename

## Problem

`Loop` is misleading. Only continuous manual runs actually "loop". Scheduled and subscribed runs trigger on events, and flows run once. The current naming conflates the entity (a configured unit of work) with one specific execution mode.

## Solution

Rename `Loop` → `Job`. A job is a configured unit of work with:
- **What**: flow definition or prompt
- **Trigger**: manual, cron, or pathset
- **Repetition**: one-shot or continuous

## Scope

### Python (src/loopflow/lfd/)

| File | Changes |
|------|---------|
| `models.py` | `Loop` → `Job`, `LoopRun` → `JobRun`, `LoopType` → `JobType`, `LoopStatus` → `JobStatus`, `loop_main` → `job_main` |
| `db.py` | All `*_loop*` functions → `*_job*`, table names in queries |
| `loops.py` → `jobs.py` | `create_loop` → `create_job`, `start_loop` → `start_job`, `stop_loop` → `stop_job` |
| `loop_runner.py` → `job_runner.py` | `run_loop_iterations` → `run_job_iterations` |
| `schedule.py` | Update imports and parameter names |
| `subscribe.py` | Update imports and parameter names |
| `server.py` | Event names `loop.*` → `job.*`, status response keys |
| `__init__.py` | CLI command `loop` → `job`, all display text |

### Database

New migration to:
- Rename table `loops` → `jobs`
- Rename table `loop_runs` → `job_runs`
- Rename column `loop_main` → `job_main`
- Rename column `loop_id` → `job_id`
- Update indexes

### TypeScript (web/src/)

| File | Changes |
|------|---------|
| `models/loop.ts` → `models/job.ts` | `Loop` → `Job`, `LoopRun` → `JobRun`, enums |
| `services/lfd-client.ts` | Update imports and types |

### Swift (swift/)

| File | Changes |
|------|---------|
| `LoopflowCore/Models/Loop.swift` → `Job.swift` | All types and properties |
| `LoopflowCore/Services/LoopService.swift` → `JobService.swift` | Function names, SQL queries |

Concerto views (`LoopRow.swift`, `LoopLiveOutput.swift`, `AppState.swift`) keep original names and use backward compatibility typealiases (`Loop = Job`, `LoopService = JobService`, etc.).

### Tests

- `tests/test_lfd.py` - Update imports and model references
- `tests/test_lfd_flows.py` - Update imports and model references

### Docs

Docs (`docs/lfd.md`, `docs/agents.md`, `docs/troubleshooting.md`) continue using `lfd loop` since the backwards compatibility alias exists. Future updates may migrate to `lfd job`.

## Decisions

### 1. CLI command name

**Options:**
- `lfd job` (matches the model)
- `lfd loop` (backward compatible, familiar)

**Recommendation:** `lfd job`. Clean break, matches the model.

### 2. Sub-commands

Keep `lfd flow`, `lfd subscribe`, `lfd schedule` as-is. These describe behavior types and create jobs with specific trigger configurations. They're convenience aliases.

### 3. JobType enum values

**Options:**
- Keep `'loop'`, `'flow'`, `'subscribe'`, `'schedule'` (database backward compat)
- Change to `'continuous'`, `'oneshot'`, `'pathset'`, `'cron'`

**Recommendation:** Keep existing values. They're stored in the database and describe the behavior accurately. The enum name changes but values don't need to.

### 4. Branch naming

`job_main` branches like `myarea-foo-bar-main` don't encode "loop" or "job" in the actual branch name, just in the field name. No branch rename needed.

## Backwards Compatibility

The implementation provides comprehensive backwards compatibility:

**Python**: Type aliases (`Loop = Job`, `LoopRun = JobRun`) in all modules. Old files (`loops.py`, `loop_runner.py`) re-export from new files. Hidden CLI command `lfd loop` aliases to `lfd job`.

**Swift**: Public typealiases (`Loop = Job`, `LoopService = JobService`, etc.) allow Concerto views to keep using `Loop` terminology.

**TypeScript**: `loop.ts` re-exports all types from `job.ts` with both old and new names.

**Database**: Migration detects existing table names and only runs if needed. Functions query for actual table names at runtime.

## Implementation Summary

All tests pass (536 Python tests). Swift framework builds successfully.

Changes:
- 10 Python files in `src/loopflow/lfd/`
- 2 Swift files (`Job.swift`, `JobService.swift`)
- 2 TypeScript files (`job.ts`, `loop.ts`)
- 1 migration (`m_2025_01_22_loop_to_job.py`)
- 1 test file (`test_lfd_flows.py`)
