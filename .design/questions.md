# Implementation Questions

Questions that arose during implementation of the agents/pipelines feature.

## Goal file format

The design doc asks: "Goal file format. Pure markdown prompt, or structured sections (objectives, constraints, out-of-scope)?"

**Implementation choice**: Pure markdown prompt. The goal path is stored in `AgentSpec.goal` and read as context when running tasks. This is the simpler approach and matches how task files work. Structured sections can be added later if needed.

## .research/ structure

The design doc asks: "`.research/` structure. Free-form files, or convention like `{agent-emoji}-notes.md`?"

**Implementation choice**: Not implemented yet. The `.research/` directory concept is documented but not enforced by code. Agents can write to it freely. A convention can be added later if cross-agent communication becomes important.

## Pipeline integration with existing run_pipeline

The new `PipelineDef` and DAG execution in `lfd/pipelines.py` is separate from the existing `pipeline.py` which uses `PipelineConfig` from `config.py`. The design doc specifies pipelines in `.lf/pipelines/*.yaml` while existing pipelines are defined inline in `.lf/config.yaml`.

**Implementation choice**: Both systems coexist. Agents use the new `lfd/pipelines.py` module which loads from YAML files. The existing `run_pipeline()` in `pipeline.py` continues to work for the `lf ship` command using config.yaml pipelines. A future consolidation may be needed.

## Database migration for emoji field

Added `emoji` column to `agent_runs` table. New databases will have it; existing databases will need migration.

**Implementation choice**: The column is added with `DEFAULT ''` so SQLite will handle existing rows gracefully. No explicit migration is needed.

## Type hint for run_step callable

The `execute_pipeline()` function takes a `run_step: callable` parameter. A proper Protocol type would be cleaner but adds complexity.

**Implementation choice**: Left as `callable` for simplicity. Can be tightened if type checking becomes stricter.
