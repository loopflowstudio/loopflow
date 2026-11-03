# Open questions and assumptions

## Settings catalogue design review — 2026-11-03

Jack Heart requested a catalogue design while the approved choice-before-fallback
pursue continues. Draft: `settings-catalog.md`. Proposed keystone unifies definition
policy, discovery and explanation while retaining storage/domain owners. Review
whether dedicated-action bindings are sufficient initially for theme, portfolio,
accounts and machine movement. Editor generation, launch snapshots and distribution
remain later outcomes. These are review decisions for the catalogue draft, not
blockers or new instructions for the active pursue.

## Duplicate implement run stopped (2026-11-02 14:25)

A second `code` Graph (process 768383a6, `lf graph code`) was launched in this
worktree by an exploratory shell command from the primary implement agent
(`lf help graph; lf graph code | head -40`). Its implement step found two
implement agents on one checkout, made no code changes, and stopped its own
Graph (PID 83352) so it would not run compress/sync/pr-publish against the
primary run's live edits. The primary run (Graph f6f522e2, `lf task run
LOO-714`) owns this step. `lf help graph <name>` explains a graph without
launching it; `lf graph <name>` runs it.

## Resume fallback (November 3)

Assumption for Jack Heart's choice-before-fallback revision: a CLI/env default_agent
is not an explicit cross-provider import request on resume. Keep native history's
provider, applying the fallback model when it names that same provider. Explicit
agent choices retain the existing switch behavior.
