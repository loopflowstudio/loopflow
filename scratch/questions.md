
- 2026-10-20 (LOO-537 follow-up, out of scope): first use of a preview copy spends 284 s of a 336 s instrumented run in `copy::sanitize`, whose `UPDATE processes` fires per-row triggers across 910k rows, plus 49 s in `validate_foreign_keys`. A candidate Task: make the copy cheap by suspending or bypassing those triggers during sanitize.
