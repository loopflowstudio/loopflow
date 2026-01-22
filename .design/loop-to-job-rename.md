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
| `Concerto/Views/LoopRow.swift` → `JobRow.swift` | Parameter names |
| `Concerto/Views/LoopLiveOutput.swift` | Update references |
| `Concerto/AppState.swift` | Property names |

### Tests

- `tests/test_lfd.py` - Update imports and model references
- `tests/test_lfd_flows.py` - Update imports and model references

### Docs

- `docs/lfd.md` - Command examples
- `docs/troubleshooting.md` - Examples

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

## Migration Strategy

1. **Database migration first** - Rename tables and columns with ALTER statements
2. **Python changes** - All model/function renames
3. **Swift/TypeScript changes** - Frontend model updates
4. **Test updates** - Fix imports and assertions
5. **Doc updates** - Command examples

Single PR. No backward compatibility shims—this is internal tooling.

## Files Changed

~35-40 files total:
- 8 Python source files
- 1 new migration file
- 2 TypeScript files
- 6 Swift files
- 2 test files
- 3 doc files
