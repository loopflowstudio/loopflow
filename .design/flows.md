# Flow UX + Choose

## Summary
Flows are Python-defined DAGs in `.lf/flows/*.py`, with `choose` (prompted decision between subflows), `fork` (run N branches), and `join` (synthesize forked outputs back into one changeset). `ship` starts with a choose between adding to roadmap and scoping from roadmap. `add_to_roadmap` runs a fork of three model+voice variants, then joins them into a single changeset on the main worktree; `scope_from_roadmap` continues through `design_from_roadmap` → `implement` → `polish`.

## Recent updates
- Ran ruff lint/format; formatted `src/loopflow/lfd/db.py`.

## Outstanding work
- Update docs/README with choose/fork/join examples and the ship flow fork/join pattern.
- Encourage join to write a summary artifact (e.g., `.design/joins/<flow>.md`) but do not require it for correctness.

## Docs + README updates
- Update docs to show flows live under `.lf/flows/*.py`, including examples of `Flow(...)`, `Choose`, `fork`, and `join` usage.
- Add `ship` example showing a choose between `roadmap` and `design_from_roadmap`, then `implement`/`polish`.
- README: update pipeline/flow examples to reference `.lf/flows/ship.py` and mention that each loop/flow must specify a single pipeline via `--flow`.
- Document choose behavior: `choose` writes decision to `.design/choices/<flow>.md` with `choice` + `reason`; `join` synthesizes fork diffs into one changeset.

## Open questions
- Should `join` support an explicit output artifact (e.g., summary) or rely on commit + logs?
- How should we signal flow requirements in CLI help (flow is required for loops/flows)?
