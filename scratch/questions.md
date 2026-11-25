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

3. **`ui.yaml` drops `pr-publish` before `deliver` and runs `cmd: sync` before
   `implement`.** `deliver`'s `gate` already publishes after `reflect`, and
   syncing before `implement` avoids mid-loop conflicts after a human demo pause.

4. **`ui` with auto on stands in at `demo` using `scratch/qa.html`.** `demo.md`
   gains a two-line headless decider rule (inspect qa screenshots/transcripts,
   iterate only if visibly broken, else note no person tried it and advance) so
   Auto-Ship behaves deterministically rather than clashing with `stand_in_message`.

5. **`fix` merges with no interactive step.** Matches the brief; Desktop's Start
   menu already labels it `merges without asking`, and `lf task auto off` stops
   before `land`.
