# flows

Flow definitions are now Python files with choose/fork/join support, and lfd loops now track area, goals, and flow with updated docs and tests to match.

## Review

**Verdict:** Needs work

- Fork join diffs can be empty because `_collect_fork_diffs` uses `git diff` even though fork steps are auto-committed; with a clean worktree the join prompt sees no changes and may produce a no-op. Consider diffing against `HEAD~1` or the fork base to capture committed changes. `src/loopflow/lf/flow.py`
- Maestro's LoopService reads `area` and `goals` only; older loop rows that stored goal data in the legacy `goal` column (or non-JSON goals) will render as empty/adaptive in the UI. Consider falling back to `goal` and handling non-JSON goal strings. `Maestro/Maestro/Services/LoopService.swift`

## Design notes

- Keep join summaries optional (`.design/joins/<flow>.md`) so join can succeed without extra artifacts.
- Open questions to carry forward: should join require an explicit output artifact, and how should the CLI surface required `--flow` usage?
