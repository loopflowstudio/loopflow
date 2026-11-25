# Open questions (LOO-1104)

Assumptions taken with nobody in the conversation; each is reversible.

1. **Wave memory curation moves to `wave-operate`, but that skill reads,
   judges and acts on Tasks and says the Wave's plan is not its to change.**
   The brief names `wave_operate.md` as the destination, so the section goes
   there, conditional on write access. `wave_session.md` (lines 34–43) already
   owns writing Wave direction into `MEMORY.md` and may be the better home.

2. **`.lf/wave/intelligence/MEMORY.md` (2026-09-28) says "`realign` now owns
   memory curation alongside plan and code reconciliation. Keep that procedure
   in the skill."** This Task reverses that. The file is read-only in this
   checkout. Proposed replacement for that sentence: "Memory curation lives in
   builtin `wave-operate`; `realign` reconciles plan and code and points to it
   (LOO-1104)."

3. **`ui` with auto on lands without a demo.** `demo` headless says the demo
   didn't happen and finishes, which advances the loop. Should `ui` refuse to
   pass its demo step without a person, or is auto allowed to skip it as it
   skips `ship-review` in `code`?

4. **`ui` has `pr-publish` and then `deliver`, whose `gate` publishes again.**
   Kept as the brief wrote it. Dropping `pr-publish` saves a step.

5. **`fix` merges with no interactive step.** Kept as the brief wrote it and
   said in its description. An `interactive: verify` at the end would restore
   one stop.

6. **`verify` fast path compares the final PR head with the merge SHA**, not
   `origin/main` as the brief words it, because `origin/main` keeps moving.
