# Simplification Opportunities

This branch consolidates goals into voices and simplifies context assembly.

## Summary

**Goals → Voices**: Deleted `goals.py`, `templates/goals/`, and `test_goals.py`. Voices now handle both persona (how to respond) and goals (what to achieve). Voice loading supports repo, global (~/.lf/voices/), and builtin sources.

**Context Config**: Introduced `DiffMode` enum and `ContextConfig` model to replace scattered boolean flags. The `--diff-mode` CLI flag replaces `--diff/--no-diff` + `--diff-files/--no-diff-files` combinations.

## Changes

- Deleted `src/loopflow/lf/goals.py` (275 lines)
- Deleted `src/loopflow/templates/goals/` (4 files)
- Deleted `tests/test_goals.py` (229 lines)
- Updated `voices.py` with global voice support and `format_voice_section()`
- Updated `context.py` with `DiffMode` enum and `ContextConfig` model
- Updated `step.py` to use new context config
- Updated `design.py` to use voice loading from voices.py
- Updated tests to remove goals references

## Future Opportunities (not in this branch)

### Step resolution layers
Steps check 6 locations: external skills, repo steps, claude commands, global steps, global claude commands, builtins. Could simplify to: repo → global → builtins with skills as explicit concept.

### FlowRun/StepRun/Agent
Three execution models where two might suffice. Session (execution) + Agent (config) could replace the three-layer hierarchy.
