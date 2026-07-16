# Open questions and assumptions — W2-238 kickoff

## Assumptions (proceeding with these; correct in review-design if wrong)

1. **"Explicit override" at the ops layer = the existing `LF_PROJECT_SESSION_ID` arm in task's
   `command_source`.** The TaskCommand/ProjectCommand CLI variants take only `issue`+`message`+
   `--json` (mod.rs:1024+) — no `--wave` is threaded to `command_source`. So there is no
   wave-level `--wave` override at this layer to preserve; the Project-Session arm is the explicit
   override. If a true wave `--wave` override is wanted on `lf task steer`/`lf project steer`, that
   is a separate CLI follow-up, not this task.

2. **`command_source` and `project_command_source` become `async` and take `&SharedStore`.** Every
   call site (task 690/2730/2781; project 292/751/793) is inside an async block with `store` bound,
   so this is feasible without a scratch-thread store.

3. **"Stale name" is a distinct classified result from "foreign registered wave".** The shared
   resolver returns a hand-set name without membership check; `command_source` adds a
   `get_wave_by_name` membership check so stale-name → "not registered" loud error, while
   foreign-registered → "cannot control". Different remediation, so different error text.

4. **Delete `command_source_for_wave` and its test** (`foreign_wave_cannot_be_reclassified_as_a_human_command`,
   task.rs:3688). Its invariant (foreign/stale wave never becomes `Human`) is re-expressed in the
   new matrix test. No second identity model.

## Open questions — resolved (executive decisions, 2026-07-16)

- **Test placement.** Option (a): a lib `#[tokio::test]` over `resolve_child_command_source`
  (`pub(crate)`) + the two wrappers with a temp `open_store` sqlite. Direct, no network, proves the
  shared code and both wrappers classify identically across the 7 ambient contexts. This is the core
  proof the done-when demands. A thin bin-level parity extension (b) is nice-to-have, not required.

- **Helper placement.** `ops::util` — already owns `normalize_wave_name`, 36 lines, no new module for
  one function. Confirmed.

## Status

Kickoff design written to `scratch/resolve-ambient-wave-names-and.md` and verified against the
current code (2026-07-16): every de-risking finding holds — call sites, async context, store
availability, resolver behavior, test deletability. Open questions resolved. Acknowledging directive
v1 and advancing to implementation.
