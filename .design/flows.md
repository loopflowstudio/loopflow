# Flow UX + Choose

## Summary
We are migrating flows to Python-defined DAGs in `.lf/flows/*.py`, adding `choose_fork` (prompted flow decision) and `choose_result` (multi-variant outputs with judge selection). `ship` now starts with a fork between adding to roadmap and scoping from roadmap, then continues with implement/polish.

## Outstanding work
- Implement/finish runtime support for choose_fork/choose_result in lfd loop execution and flow runner (merge winner, write decision to `.design/choices/`).
- Decide on supported options for choose_result (model, voice, context) and whether to allow additional metadata in options.
- Determine whether parallel/race are supported in lfd loops or explicitly disallowed with clear errors.

## Docs + README updates
- Update docs to show flows live under `.lf/flows/*.py`, including examples of `Flow(...)`, `ChooseFork`, and `ChooseResult` usage.
- Add `ship` example showing a fork between `roadmap` and `design_from_roadmap`, then `implement`/`polish`.
- README: update pipeline/flow examples to reference `.lf/flows/ship.py` and mention that each loop/flow must specify a single pipeline via `--flow`.
- Document choose behavior: fork writes decision to `.design/choices/<flow>.md` with `choice` + `reason`; choose_result runs variants and uses judge selection.

## Open questions
- Should choose_result allow additional voice configurations per option beyond `voice` lists?
- Do we want to expose a default judge step or require explicit judge config?
- How should we signal flow requirements in CLI help (flow is required for loops/flows)?
