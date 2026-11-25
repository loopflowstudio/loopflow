# Open questions (LOO-1104)

Reversible choices made for review (see `scratch/concept-review.html` for full context):

1. **Wave memory curation moves to `wave_session.md` instead of `wave_operate.md`.**
   `build.rs` composes `wave-session` from `wave_session.md` + `wave_operate.md`,
   `wave_session.md` already owns writing/publishing `MEMORY.md`, and
   `wave_operate.md:10` says Wave files are not its to change.

2. **`.lf/wave/intelligence/MEMORY.md` (2026-09-28) says "`realign` now owns
   memory curation alongside plan and code reconciliation. Keep that procedure
   in the skill."** Read-only in this Task checkout. Proposed replacement once
   LOO-1104 ships: "Memory curation lives in builtin `wave-session`; `realign`
   reconciles plan and code and points to it (LOO-1104)."

3. **`quick` merges with no interactive step.** Matches the brief and
   ship-review revision (`fix` renamed to `quick`; `ui` removed from builtins);
   Desktop's Start menu labels it `merges without asking`, and `lf task auto off`
   stops before `land`.
